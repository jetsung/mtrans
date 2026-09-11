# Docker Mtrans

Docker image replication tool: copies source images to a **target registry**, and can also pull them back locally. Written in Rust, single-file CLI, supports **Windows / Linux / macOS**; runs standalone (`docker-mtrans`) or as a docker CLI plugin (`docker mtrans ...`).

## Features

- **Remote replication**: All copies are hosted via GitHub Actions workflows (ADR-0001); the local machine never directly replicates or holds image layers.
- **Encrypted credential transport**: Uses `~/.docker/config.json` login state as the source; during `sync`, the `auth` field in `auths` is encrypted with the passphrase `[setting].auth_passphrase` into `target_auth_secret` and passed to the workflow. The workflow decrypts it via GitHub Secret `AUTH_PASSPHRASE` (openssl-compatible AES-256-CBC+PBKDF2) and masks it with `::add-mask::` — never written to config files, never printed to logs (ADR-0002).
- **Three target combination modes**: mode 1 single-repo aggregation, mode 2 repo mapping, mode 3 fixed transit (ADR-0003). `sync`/`pull` share the same combination rules, ensuring round-trip reconciliation.
- **Override parameters**: `sync`/`pull`/`spull` support `--registry/-R`, `--org/-o`, `--repo/-r`, `--mode/-m` for one-time config overrides — temporarily switch targets without modifying `config.toml`.
- **Cross-platform**: Endpoint resolution auto-detects Windows named pipes and Linux/macOS default sockets; config paths respect `DOCKER_CONFIG`/`XDG_CONFIG_HOME`.
- **Chinese/English interface**: `help`/`man`/`--help` and runtime output (progress/success/error messages) switch between Chinese and English based on system language (`LANG`/`LC_ALL`/`LC_MESSAGES`), defaulting to English.

## Installation

```bash
# One-line install script
curl -fsSL https://mtrans.gcli.cn/install.sh | bash

# From crates.io
cargo install mtrans

# Or from the Git repository
cargo install --git https://github.com/jetsung/mtrans.git
```

Or build from source: `cargo build --release` (binary at `target/release/docker-mtrans`).

## Quick Start

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

Before first use, configure `config.toml` with the `config` wizard and select the target registry with `registry`:

```bash
docker mtrans config     # TUI wizard for step-by-step configuration
docker mtrans registry   # Select target registry
docker mtrans ci          # Output workflow YAML template (deploy to [ci].repo repository)
```

## Commands

| Subcommand | Description |
| --- | --- |
| `sync <source-image>` | Trigger remote workflow to replicate source image to target registry |
| `pull <source-image>` | Derive target from source image, pull back from target registry and rename to source image name |
| `spull <source-image>` | Sync then pull (sync + pull) |
| `secret [registry]` | Print encrypted login credential (`auth_passphrase` encrypted, i.e. `target_auth_secret`) to stdout |
| `import` | Sync logged-in registries into `[registries]` entries (idempotent) |
| `registry` | TUI to select target registry and sync `[registries]` |
| `passphrase` | TUI to set encryption passphrase (`auth_passphrase`): keep / auto-generate / manual input |
| `config` | TUI wizard for step-by-step `config.toml` configuration |
| `ci [file]` | Output workflow YAML template (prints to stdout without arguments) |
| `help` / `man` | View help: `help` concise summary, `man` full manual (Chinese/English per system language) |

Common override parameters for `sync`/`pull`/`spull` (optional, effective for single execution, not written back to config): `--registry/-R` (target registry), `--org/-o` (target organization), `--repo/-r` (target repository), `--mode/-m` (combination mode 1/2/3). Priority: command line > `[registries]` entry > `[setting]` > built-in defaults.

## Documentation

| Document | Content |
| --- | --- |
| [docs/SPEC.md](docs/SPEC.md) | Config schema, target image combination rules, and credential transport contract (specification) |
| [docs/CONTEXT.md](docs/CONTEXT.md) | Glossary (single source of truth for terminology) |
| [docs/adr](docs/adr/) | Architecture Decision Records (ADR) |
| [config.example.toml](config.example.toml) | Authoritative config sample (redacted placeholders, with comments) |

## License

Apache-2.0
