---
icon: lucide/cloud-upload
---
# `sync` — 远程复制

触发远程流水线，把**源镜像**复制到**目标注册表**。

## 用法

```bash
docker mtrans sync <源镜像> [覆盖参数]
```

```bash
# 典型用法
docker mtrans sync ghcr.io/jetsung/shortener:latest

# 临时覆盖目标（单次生效，不写回配置）
docker mtrans sync alpine:latest -R new.example.com -o myorg -m 2
```

## 执行流程

1. 校验源镜像为 `name:tag` 形式（含 `@sha256:` digest 引用一律报错）。
2. 按[组合规则](../advanced/naming-rules.md)由源镜像派生**目标镜像**。
3. 从 `~/.docker/config.json` 读取目标注册表的登录凭据 `auth`，用 `auth_passphrase` 加密成 `target_auth_secret`。
4. 以 `github_token`（或回退 `GITHUB_TOKEN`）触发 `[ci].repo@branch` 的 `workflow` 工作流，inputs 携带源镜像、目标镜像与 `target_auth_secret`。
5. 本地轮询等待运行结束；成功后默认删除该次运行（`keep_run = true` 保留）。

## 前置条件

| 条件 | 未满足时 |
| --- | --- |
| 目标注册表已 `docker login`（`auths` 下存在该注册表条目） | 直接报错 |
| `[setting].auth_passphrase` 已设置 | 直接报错 |
| GitHub Token 可用（配置或环境变量 `GITHUB_TOKEN`） | 报错 |
| `[ci].repo` 上已部署工作流并配置 Secret `AUTH_PASSPHRASE` | 工作流运行失败 |

!!! tip "部署工作流"

    尚未部署工作流？先运行 `docker mtrans ci` 生成模板，见[部署工作流](../advanced/workflow.md)。

## 覆盖参数

支持 `--registry/-R`、`--org/-o`、`--repo/-r`、`--mode/-m`，见[命令总览](index.md#overrides)。优先级：**命令行 > `[registries]` 条目 > `[setting]` > 内置默认值**。

## 目标镜像示例

以默认目标 `registry.cn-guangzhou.aliyuncs.com/jetsung/myimage` 为例：

| 源镜像 | mode=1 | mode=2 | mode=3 |
| --- | --- | --- | --- |
| `ghcr.io/idev/shortener:dev` | `…/jetsung/myimage:shortener` | `…/jetsung/shortener:dev` | `…/jetsung/myimage:mtrans` |
| `alpine:3.19` | `…/jetsung/myimage:alpine` | `…/jetsung/alpine:3.19` | `…/jetsung/myimage:mtrans` |
