# 仅通过 GitHub Actions 远程触发复制

复制一律由本地触发 GitHub Actions `workflow_dispatch`，由托管 Runner 完成拉取与推送。本地只做 `pull` 侧的拉取与改名，不直连目标注册表做复制。托管 Runner 免去本机直连目标注册表的配置与暴露面，复制日志可在流水线侧审计；代价是强依赖 GitHub 仓库与具备 `actions: write` 的 Token。凭据以本机 `~/.docker/config.json` 登录态为源，`sync` 时用加密口令加密成 `target_auth_secret` 传入，流水线内解密并打掩码（ADR-0002）。
