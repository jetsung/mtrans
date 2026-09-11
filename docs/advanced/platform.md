---
icon: lucide/monitor
---
# 平台支持

Windows、Linux、macOS 三端均可构建运行。跨平台差异**仅在本地连接 Docker 的默认端点与配置文件路径**。

## 默认路径

| 平台 | 默认 Docker 端点 | 应用配置路径（`XDG_CONFIG_HOME` 未设置时） | Docker 配置路径（`DOCKER_CONFIG` 未设置时） |
| --- | --- | --- | --- |
| Linux | `/var/run/docker.sock` | `~/.config/mtrans/config.toml` | `~/.docker/config.json` |
| macOS | `/var/run/docker.sock` | `~/.config/mtrans/config.toml` | `~/.docker/config.json` |
| Windows | `\\.\pipe\docker_engine` | `%APPDATA%\mtrans\config.toml` | `%USERPROFILE%\.docker\config.json` |

## 端点解析

按以下优先级解析本地 Docker 端点：

1. `DOCKER_HOST` 环境变量——支持 `unix://`、`tcp://`、`http://`、`npipe://` 前缀；
2. `DOCKER_CONTEXT` 环境变量；
3. `config.json` 的 `currentContext`；
4. 平台默认端点（上表）。

!!! warning "不支持的端点"

    TLS（`https://`）与 `ssh://` 不支持，需自建隧道转成明文端点。

## 配置路径规则

- **应用配置**：`XDG_CONFIG_HOME` 优先，未设置时回退 `dirs::config_dir()`（各平台标准目录）。
- **Docker 配置与 context 目录**：均尊重 `DOCKER_CONFIG` 覆盖。

## 插件形态路径

`docker mtrans ...` 需把可执行文件放入 docker 插件目录：

| 平台 | 插件目录 |
| --- | --- |
| Linux / macOS | `~/.docker/cli-plugins/` |
| Windows | `%USERPROFILE%\.docker\cli-plugins\` |

## CI 覆盖

仓库自身的 `.github/workflows/ci.yml` 在 ubuntu / macos / windows 三系统运行 clippy、test、release 构建，并校验 docker 插件发现协议——三端兼容性由 CI 持续保证。
