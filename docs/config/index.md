---
icon: lucide/settings
---
# 配置总览

配置文件为 `config.toml`，取值口径：**配置文件中的值非空则生效，否则回退内置默认值**。仓库根目录的 [`config.example.toml`](https://github.com/jetsung/mtrans/blob/main/config.example.toml) 是唯一权威样例。

## 文件路径

`$XDG_CONFIG_HOME/mtrans/config.toml`；`XDG_CONFIG_HOME` 未设置时回退平台标准配置目录：

| 平台 | 路径 |
| --- | --- |
| Linux / macOS | `~/.config/mtrans/config.toml` |
| Windows | `%APPDATA%\mtrans\config.toml` |

!!! note "容错行为"

    文件缺失或 TOML 解析失败时，打印告警并整体按内置默认值继续，**不中断**。

## `[setting]` 段

全局设置：

| 键 | 类型 | 内置默认值 | 说明 |
| --- | :-: | --- | --- |
| `github_token` | string | 无 | 触发远程流水线用；未设置时回退环境变量 `GITHUB_TOKEN` |
| `auth_passphrase` | string | 无 | 加密口令；未设置时 `sync` 报错（生成：`openssl rand -base64 24`） |
| `target_registry` | string | `registry.cn-guangzhou.aliyuncs.com` | 目标注册表域名 |
| `target_org` | string | `jetsung` | 目标组织；`[registries]` 条目同名键未设置时作回退值 |
| `target_repo` | string | `myimage` | 目标仓库；`[registries]` 条目同名键未设置时作回退值 |
| `mode` | int | `1` | 目标镜像组合模式：`1`/`2`/`3`，见[组合规则](../advanced/naming-rules.md) |

## `[ci]` 段

远程流水线所在仓库与工作流：

| 键 | 类型 | 内置默认值 | 说明 |
| --- | :-: | --- | --- |
| `repo` | string | `jetsung/docker-build-sync` | 流水线 GitHub 仓库（`owner/name`），复制一律经其工作流触发 |
| `branch` | string | `main` | 工作流分支 |
| `workflow` | string | `docker-mtrans.yml` | 工作流文件名 |
| `keep_run` | bool | `false` | 复制成功后是否保留该次运行记录（连同日志）；默认成功即删除 |

## `[registries."<注册表域名>"]` 段 { #registries }

注册表条目为**可选覆盖**：仅当该注册表是本次同步的目标注册表（等于 `target_registry`）时生效，并**逐键覆盖**——条目中已设置（非空）的 `target_org`/`target_repo` 覆盖 `[setting]` 同名键，未设置的键逐项回退到 `[setting]`。

```toml
[registries]

[registries."registry.cn-guangzhou.aliyuncs.com"]
target_org = "myorg"
target_repo = "myrepo"
```

- 表键即注册表域名：小写、不带 scheme、不带路径；Docker Hub 的键归一化为 `docker.io`。
- 条目由 `import` / `registry` / `config` 向导同步写入，或手动编辑添加。

## 完整样例

```toml title="config.toml"
[setting]
github_token = "ghp_xxx"          # 或回退环境变量 GITHUB_TOKEN
auth_passphrase = "<openssl rand -base64 24 生成>"
target_registry = "registry.example.com"
target_org = "your-org"
target_repo = "your-repo"
mode = 1

[ci]
repo = "your-account/docker-build-sync"
branch = "main"
workflow = "docker-mtrans.yml"
keep_run = false

[registries."registry.example.com"]
target_org = "your-org"
target_repo = "your-repo"
```

!!! tip "不想手写配置？"

    运行 `docker mtrans config` 进入 TUI 向导，见[交互向导](wizard.md)。
