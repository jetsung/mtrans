# Docker Mtrans

Docker 镜像复制工具：把源镜像复制到**目标注册表**，也可反向拉回本地。Rust 编写，单文件 CLI，支持 **Windows / Linux / macOS** 三端；可独立运行（`docker-mtrans`），也可作为 docker CLI 插件（`docker mtrans ...`）。

## 特性

- **远程复制**：复制一律经 GitHub Actions 工作流托管（ADR-0001），本地不直连复制、不持有镜像层。
- **凭据加密传输**：以 `~/.docker/config.json` 登录态为源，`sync` 时把 `auths` 的 `auth` 用加密口令 `[setting].auth_passphrase` 加密成 `target_auth_secret` 传入工作流，流水线内用 GitHub Secret `AUTH_PASSPHRASE` 解密（openssl 兼容 AES-256-CBC+PBKDF2），并 `::add-mask::` 打掩码，不落配置文件、不打印到日志（ADR-0002）。
- **三种目标组合模式**：mode 1 单仓库汇聚、mode 2 仓库映射、mode 3 固定中转（ADR-0003），`sync`/`pull` 共用同一组合规则，保证往返对账。
- **覆盖参数**：`sync`/`pull`/`spull` 支持 `--registry/-R`、`--org/-o`、`--repo/-r`、`--mode/-m` 单次覆盖配置，临时换目标无需改 `config.toml`。
- **跨平台**：端点解析自动识别 Windows 命名管道与 Linux/macOS 默认 socket；配置路径尊重 `DOCKER_CONFIG`/`XDG_CONFIG_HOME`。
- **中文/英文界面**：`help`/`man`/`--help` 与运行时输出（进度/成功/错误提示）均按系统语言（`LANG`/`LC_ALL`/`LC_MESSAGES`）中英切换，默认英文。

## 安装

```bash
# 一键安装脚本
curl -fsSL https://mtrans.gcli.cn/install.sh | bash

# 从 crates.io 安装
cargo install mtrans

# 或从 Git 仓库安装
cargo install --git https://github.com/jetsung/mtrans.git
```

或从源码构建：`cargo build --release`（产物在 `target/release/docker-mtrans`）。

## 快速开始

```bash
# 复制镜像到目标注册表（触发远程流水线，须已登录目标注册表）
docker-mtrans sync ghcr.io/jetsung/shortener:latest
# 从目标注册表拉回并重命名
docker-mtrans pull ghcr.io/jetsung/shortener:latest
# 覆盖参数：临时换目标，不写回 config.toml
docker-mtrans sync alpine:latest -R new.example.com -o myorg -m 2
# 也可作为 docker 插件
docker mtrans sync ghcr.io/jetsung/shortener:latest
```

首次使用前用 `config` 向导配置 `config.toml`，用 `registry` 选择目标注册表：

```bash
docker mtrans config     # TUI 向导逐步配置
docker mtrans registry   # 选择目标注册表
docker mtrans ci          # 输出工作流 YAML 模板（部署到 [ci].repo 仓库）
```

## 命令

| 子命令 | 说明 |
| --- | --- |
| `sync <源镜像>` | 触发远程流水线复制源镜像到目标注册表 |
| `pull <源镜像>` | 由源镜像推算目标，从目标注册表拉回并重命名为源镜像名 |
| `spull <源镜像>` | 先同步再拉取（sync + pull） |
| `secret [注册表]` | 打印登录凭据加密串（`auth_passphrase` 加密，即 `target_auth_secret`）到 stdout |
| `import` | 把已登录注册表同步为 `[registries]` 条目（幂等） |
| `registry` | TUI 选择目标注册表并同步 `[registries]` |
| `passphrase` | TUI 设置加密口令（`auth_passphrase`）：保持 / 自动生成 / 手输 |
| `config` | TUI 向导逐步配置 `config.toml` |
| `ci [文件]` | 输出工作流 YAML 模板（无参数打印 stdout） |
| `help` / `man` | 查看帮助：`help` 简洁摘要，`man` 完整手册（中英随系统语言） |

`sync`/`pull`/`spull` 通用覆盖参数（可选，单次执行生效、不写回配置）：`--registry/-R`（目标注册表）、`--org/-o`（目标组织）、`--repo/-r`（目标仓库）、`--mode/-m`（组合模式 1/2/3）。优先级：命令行 > `[registries]` 条目 > `[setting]` > 内置默认。

## 文档

| 文档 | 内容 |
| --- | --- |
| [docs/SPEC.md](docs/SPEC.md) | 配置 schema、目标镜像组合规则与凭据传递契约（规范） |
| [docs/CONTEXT.md](docs/CONTEXT.md) | 术语表（唯一措辞来源） |
| [docs/adr](docs/adr/) | 架构决策记录（ADR） |
| [config.example.toml](config.example.toml) | 权威配置样例（脱敏占位，含注释） |

## 许可

Apache-2.0
