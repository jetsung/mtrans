//! GitHub Actions API 客户端与远程触发流程。
//!
//! 统一：`Authorization: Bearer`、`Accept: application/vnd.github+json`、
//! `X-GitHub-Api-Version: 2022-11-28`、插件 User-Agent、30s 超时；
//! 网络错误以 `(0, reason)` 口径返回而非抛异常。

use std::time::Duration;

use serde_json::Value;

use crate::ui;

pub const API_VERSION: &str = "2022-11-28";
pub const USER_AGENT: &str = concat!("docker-mtrans/", env!("CARGO_PKG_VERSION"));
pub const GITHUB_BASE: &str = "https://api.github.com";

pub struct GithubClient {
    http: reqwest::Client,
    pub base: String,
    pub token: String,
    /// 新 run 出现轮询间隔 / 超时（基准 3s / 120s）
    pub new_run_interval: Duration,
    pub new_run_timeout: Duration,
    /// 运行完成轮询间隔 / 超时（基准 5s / 600s）
    pub complete_interval: Duration,
    pub complete_timeout: Duration,
}

/// `(status, json, raw)`；status=0 表示网络错误，raw 为原因。
pub struct ApiResponse {
    pub status: u16,
    pub json: Option<Value>,
    pub raw: String,
}

impl GithubClient {
    pub fn new(base: &str, token: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(USER_AGENT)
            .build()
            .expect("构建 GitHub HTTP 客户端失败");
        Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            token: token.to_string(),
            new_run_interval: Duration::from_secs(3),
            new_run_timeout: Duration::from_secs(120),
            complete_interval: Duration::from_secs(5),
            complete_timeout: Duration::from_secs(600),
        }
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        payload: Option<&Value>,
    ) -> ApiResponse {
        let mut req = self
            .http
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION);
        if let Some(p) = payload {
            req = req.json(p);
        }
        match req.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let raw = resp.text().await.unwrap_or_default();
                ApiResponse {
                    status,
                    json: serde_json::from_str::<Value>(&raw).ok(),
                    raw,
                }
            }
            Err(e) => ApiResponse {
                status: 0,
                json: None,
                raw: e.to_string(),
            },
        }
    }

    /// 触发前该工作流 `event=workflow_dispatch` 的最新 run_number（无则 0）。
    pub async fn latest_run_number(&self, repo: &str, workflow: &str) -> u64 {
        let path = format!(
            "/repos/{repo}/actions/workflows/{workflow}/runs?event=workflow_dispatch&per_page=1"
        );
        let resp = self.request(reqwest::Method::GET, &path, None).await;
        if resp.status != 200 {
            return 0;
        }
        resp.json
            .as_ref()
            .and_then(|v| v.get("workflow_runs"))
            .and_then(|r| r.get(0))
            .and_then(|r| r.get("run_number"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    }

    /// `POST .../dispatches`，成功判定 HTTP 204。
    /// `target_auth_secret` 为加密口令加密后的登录凭据加密串（流水线内解密并打掩码）。
    pub async fn dispatch(
        &self,
        repo: &str,
        workflow: &str,
        branch: &str,
        source_image: &str,
        target_image: &str,
        target_auth_secret: &str,
    ) -> ApiResponse {
        let path = format!("/repos/{repo}/actions/workflows/{workflow}/dispatches");
        let payload = serde_json::json!({
            "ref": branch,
            "inputs": {
                "source_image": source_image,
                "target_image": target_image,
                "target_auth_secret": target_auth_secret,
            }
        });
        self.request(reqwest::Method::POST, &path, Some(&payload))
            .await
    }

    /// 轮询等新 run 出现（run_number > 触发前编号），返回 run_id。
    pub async fn wait_new_run(&self, repo: &str, workflow: &str, before: u64) -> Option<u64> {
        let path = format!(
            "/repos/{repo}/actions/workflows/{workflow}/runs?event=workflow_dispatch&per_page=10"
        );
        let deadline = tokio::time::Instant::now() + self.new_run_timeout;
        while tokio::time::Instant::now() < deadline {
            let resp = self.request(reqwest::Method::GET, &path, None).await;
            if resp.status == 200
                && let Some(runs) = resp.json.as_ref().and_then(|v| v.get("workflow_runs"))
            {
                for run in runs.as_array().into_iter().flatten() {
                    let num = run.get("run_number").and_then(Value::as_u64).unwrap_or(0);
                    if num > before {
                        let id = run.get("id").and_then(Value::as_u64).unwrap_or(0);
                        if id != 0 {
                            return Some(id);
                        }
                    }
                }
            }
            tokio::time::sleep(self.new_run_interval).await;
        }
        None
    }

    /// 轮询等 `status=completed`，返回 conclusion；超时返回 None。
    pub async fn wait_run_complete(&self, repo: &str, run_id: u64) -> Option<String> {
        let path = format!("/repos/{repo}/actions/runs/{run_id}");
        let deadline = tokio::time::Instant::now() + self.complete_timeout;
        while tokio::time::Instant::now() < deadline {
            let resp = self.request(reqwest::Method::GET, &path, None).await;
            if resp.status == 200
                && resp.json.as_ref().and_then(|v| v.get("status"))
                    == Some(&Value::String("completed".into()))
            {
                return Some(
                    resp.json
                        .as_ref()
                        .and_then(|v| v.get("conclusion"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                );
            }
            tokio::time::sleep(self.complete_interval).await;
        }
        None
    }

    /// 删除整条运行记录（连同日志），期望 204。
    pub async fn delete_run(&self, repo: &str, run_id: u64) -> bool {
        let path = format!("/repos/{repo}/actions/runs/{run_id}");
        self.request(reqwest::Method::DELETE, &path, None)
            .await
            .status
            == 204
    }
}

/// 远程触发全流程：记录编号 -> dispatch -> 等新 run -> 等完成 ->
/// 仅 conclusion=success 且 keep_run=false 时删除记录。
#[allow(clippy::too_many_arguments)]
pub async fn trigger_workflow(
    gh: &GithubClient,
    repo: &str,
    branch: &str,
    workflow: &str,
    source_image: &str,
    target_image: &str,
    target_auth_secret: &str,
    keep_run: bool,
) -> Result<(), String> {
    if repo.split('/').count() != 2 || repo.starts_with('/') || repo.ends_with('/') {
        return Err(crate::i18n::err_repo_format(repo));
    }
    if target_auth_secret.is_empty() {
        return Err(crate::i18n::err_missing_target_auth().to_string());
    }
    if gh.token.is_empty() {
        return Err(crate::i18n::err_missing_github_token().to_string());
    }

    println!("{}", ui::step(crate::i18n::trigger_params_label()));
    println!("  {} {}", ui::dim("repo:         "), repo);
    println!("  {} {}", ui::dim("branch:       "), branch);
    println!("  {} {}", ui::dim("workflow:     "), workflow);
    println!("  {} {}", ui::dim("source-image: "), ui::bold(source_image));
    println!("  {} {}", ui::dim("target-image: "), ui::bold(target_image));
    println!();

    let before = gh.latest_run_number(repo, workflow).await;

    let resp = gh
        .dispatch(
            repo,
            workflow,
            branch,
            source_image,
            target_image,
            target_auth_secret,
        )
        .await;
    if resp.status == 0 {
        return Err(crate::i18n::err_github_api_unreachable(&resp.raw));
    }
    if resp.status != 204 {
        let status = resp.status;
        let hint = match status {
            401 | 403 | 404 | 422 => crate::i18n::trigger_hint(status).to_string(),
            s => return Err(crate::i18n::err_trigger_failed_http(s, &resp.raw)),
        };
        return Err(crate::i18n::err_trigger_failed_hint(&hint, &resp.raw));
    }

    println!("{}", ui::ok(crate::i18n::ok_workflow_triggered()));
    println!(
        "{} {}",
        ui::step(crate::i18n::run_status_page_label()),
        ui::bold(&format!(
            "https://github.com/{repo}/actions/workflows/{workflow}"
        ))
    );
    if keep_run {
        return Ok(());
    }

    println!("{}", ui::warn(crate::i18n::wait_run_complete_hint()));
    let run_id = gh
        .wait_new_run(repo, workflow, before)
        .await
        .ok_or_else(|| crate::i18n::err_wait_new_run_timeout().to_string())?;
    let conclusion = gh
        .wait_run_complete(repo, run_id)
        .await
        .ok_or_else(|| crate::i18n::err_wait_run_complete_timeout().to_string())?;
    if conclusion != "success" {
        return Err(crate::i18n::err_run_unsuccessful(&conclusion));
    }
    if !keep_run {
        if gh.delete_run(repo, run_id).await {
            println!(
                "{} {} {}",
                ui::ok(crate::i18n::ok_copy_deleted()),
                ui::dim("(run_id:"),
                ui::bold(&run_id.to_string()) + &ui::dim(")")
            );
        } else {
            return Err(crate::i18n::err_delete_run_failed().to_string());
        }
    } else {
        println!(
            "{} {} {}",
            ui::ok(crate::i18n::ok_copy_done()),
            ui::dim("(run_id:"),
            ui::bold(&run_id.to_string()) + &ui::dim(")")
        );
    }
    Ok(())
}
