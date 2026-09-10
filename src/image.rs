//! 镜像引用规则：digest 拒绝、源镜像拆分、目标镜像组合（mode 1/2/3，ADR-0003）。
//!
//! 组合规则是 `sync` 与 `pull` 共用的纯函数，保证可往返对账：
//! `pull` 无需同步状态即可从源镜像名推算出目标镜像。

/// 显式拒绝 `@sha256:` 等 digest 引用（组合会截出垃圾 tag，入口即响亮失败）。
pub fn reject_digest(image: &str) -> Result<(), String> {
    if image.contains('@') {
        Err(crate::i18n::err_digest_unsupported(image))
    } else {
        Ok(())
    }
}

/// 拆分 `repo:tag` 为 (repo, tag)；无 tag 时默认 `latest`。调用前须已拒绝 digest。
pub fn split_ref(image: &str) -> (String, String) {
    let last_seg = image.rsplit('/').next().unwrap_or("");
    match last_seg.split_once(':') {
        Some((name, tag)) if !tag.is_empty() => {
            let repo = &image[..image.len() - last_seg.len() + name.len()];
            (repo.to_string(), tag.to_string())
        }
        _ => (image.to_string(), "latest".to_string()),
    }
}

/// 目标镜像组合（SPEC 第 4 节）：
/// - mode 1（单仓库汇聚）：`{registry}/{target_org}/{target_repo}:{源镜像名}`，源 tag 丢弃；
/// - mode 2（仓库映射）：`{registry}/{target_org}/{源镜像名}:{源 tag}`，源无 tag 视为 latest；
/// - mode 3（固定中转）：`{registry}/{target_org}/{target_repo}:mtrans`，target_repo 缺省时用 `mtrans`。
///
/// 注册表命名空间只有一层，不支持子组织，源组织段不保留（ADR-0003）。
pub fn compose_target_image(
    source_image: &str,
    registry: &str,
    target_org: &str,
    target_repo: &str,
    mode: u8,
) -> Result<String, String> {
    reject_digest(source_image)?;
    let (src_repo, src_tag) = split_ref(source_image);
    let src_name = src_repo.rsplit('/').next().unwrap_or("");
    if src_name.is_empty() {
        return Err(crate::i18n::err_empty_image_name(source_image));
    }
    match mode {
        1 => Ok(format!("{registry}/{target_org}/{target_repo}:{src_name}")),
        2 => Ok(format!("{registry}/{target_org}/{src_name}:{src_tag}")),
        3 => {
            let repo = if target_repo.is_empty() {
                "mtrans"
            } else {
                target_repo
            };
            Ok(format!("{registry}/{target_org}/{repo}:mtrans"))
        }
        _ => Err(crate::i18n::err_unknown_mode(mode)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REG: &str = "registry.cn-guangzhou.aliyuncs.com";

    #[test]
    fn mode1_collapses_into_single_repo() {
        // 源镜像名作 tag，源 tag 丢弃
        assert_eq!(
            compose_target_image("ghcr.io/idev/shortener:dev", REG, "jetsung", "myimage", 1)
                .unwrap(),
            format!("{REG}/jetsung/myimage:shortener")
        );
        assert_eq!(
            compose_target_image("alpine:3.19", REG, "jetsung", "myimage", 1).unwrap(),
            format!("{REG}/jetsung/myimage:alpine")
        );
    }

    #[test]
    fn mode2_maps_repo_and_keeps_source_tag() {
        // 源组织段不保留（ADR-0003），tag 沿用源 tag
        assert_eq!(
            compose_target_image("ghcr.io/idev/shortener:dev", REG, "jetsung", "myimage", 2)
                .unwrap(),
            format!("{REG}/jetsung/shortener:dev")
        );
        assert_eq!(
            compose_target_image("alpine:3.19", REG, "jetsung", "myimage", 2).unwrap(),
            format!("{REG}/jetsung/alpine:3.19")
        );
    }

    #[test]
    fn mode3_fixed_relay_uses_mtrans_tag() {
        // 一律推到 target_org/target_repo:mtrans，忽略源镜像名与 tag
        assert_eq!(
            compose_target_image("ghcr.io/idev/shortener:dev", REG, "jetsung", "myimage", 3)
                .unwrap(),
            format!("{REG}/jetsung/myimage:mtrans")
        );
        assert_eq!(
            compose_target_image("alpine:3.19", REG, "jetsung", "myimage", 3).unwrap(),
            format!("{REG}/jetsung/myimage:mtrans")
        );
    }

    #[test]
    fn mode3_defaults_missing_repo_to_mtrans() {
        // target_repo 缺省时用 mtrans 作为仓库名
        assert_eq!(
            compose_target_image("ghcr.io/idev/shortener:dev", REG, "hello", "", 3).unwrap(),
            format!("{REG}/hello/mtrans:mtrans")
        );
    }

    #[test]
    fn mode2_defaults_missing_tag_to_latest() {
        assert_eq!(
            compose_target_image("ghcr.io/idev/shortener", REG, "jetsung", "myimage", 2).unwrap(),
            format!("{REG}/jetsung/shortener:latest")
        );
        // 端口不是 tag：r:5000/img 无 tag
        assert_eq!(
            compose_target_image("r.example.com:5000/img:1.0", REG, "jetsung", "myimage", 2)
                .unwrap(),
            format!("{REG}/jetsung/img:1.0")
        );
    }

    #[test]
    fn rejects_digest_and_empty_name() {
        assert!(compose_target_image("world@sha256:abc", REG, "a", "b", 1).is_err());
        assert!(compose_target_image("ghcr.io/hello/", REG, "a", "b", 1).is_err());
        assert!(compose_target_image("alpine", REG, "a", "b", 9).is_err());
    }

    #[test]
    fn split_ref_defaults_to_latest() {
        assert_eq!(split_ref("alpine"), ("alpine".into(), "latest".into()));
        assert_eq!(
            split_ref("library/alpine:3.19"),
            ("library/alpine".into(), "3.19".into())
        );
        assert_eq!(
            split_ref("r.example.com:5000/jetsung/img:1.0"),
            ("r.example.com:5000/jetsung/img".into(), "1.0".into())
        );
        // 端口不是 tag
        assert_eq!(
            split_ref("r.example.com:5000/img"),
            ("r.example.com:5000/img".into(), "latest".into())
        );
    }

    #[test]
    fn sync_and_pull_round_trip() {
        // sync 组合出的目标镜像，pull 用同一规则必须推算出同一目标
        let source = "ghcr.io/idev/shortener:dev";
        let a = compose_target_image(source, REG, "jetsung", "myimage", 2).unwrap();
        let b = compose_target_image(source, REG, "jetsung", "myimage", 2).unwrap();
        assert_eq!(a, b);
    }
}
