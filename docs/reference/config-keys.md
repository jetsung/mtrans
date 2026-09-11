---
icon: lucide/table
---
# 键名速查

`config.toml` 全部键名一页速查，详细语义见[配置总览](../config/index.md)。

```toml
[setting]
github_token     # 可选，回退环境变量 GITHUB_TOKEN
auth_passphrase  # 加密口令：加密登录凭据，须与 Secret AUTH_PASSPHRASE 一致
target_registry  # 目标注册表域名（当前使用）
target_org       # 目标组织
target_repo      # 目标仓库
mode             # 1 | 2 | 3，目标镜像组合模式

[ci]
repo             # 远程流水线仓库（复制一律经其工作流触发）
branch           # 分支
workflow         # 工作流文件名
keep_run         # bool，成功后保留流水线记录

[registries."<注册表域名>"]
target_org       # 可选，覆盖全局
target_repo      # 可选，覆盖全局
```

## 环境变量

| 变量 | 作用 |
| --- | --- |
| `GITHUB_TOKEN` | `[setting].github_token` 未设置时的回退 |
| `XDG_CONFIG_HOME` | 应用配置目录覆盖（未设置回退平台标准目录） |
| `DOCKER_CONFIG` | Docker 配置目录覆盖 |
| `DOCKER_HOST` | 本地 Docker 端点覆盖（支持 `unix://`/`tcp://`/`http://`/`npipe://`） |
| `DOCKER_CONTEXT` | Docker context 覆盖 |
| `LANG` / `LC_ALL` / `LC_MESSAGES` | 界面语言（`zh*` 判中文，默认英文） |
