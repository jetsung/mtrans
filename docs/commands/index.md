---
icon: lucide/terminal
---
# 命令总览

mtrans 以子命令组织功能，可独立运行（`docker-mtrans <子命令>`）或作为 docker 插件（`docker mtrans <子命令>`）。

## 子命令一览

| 子命令 | 形式 | 说明 |
| --- | --- | --- |
| [`sync`](sync.md) | `<源镜像>` + 覆盖参数 | 触发远程流水线，把源镜像复制到目标注册表 |
| [`pull`](pull.md) | `<源镜像>` + 覆盖参数 | 由源镜像推算目标，从目标注册表拉回并重命名 |
| [`spull`](spull.md) | `<源镜像>` + 覆盖参数 | `sync` + `pull` 连续执行 |
| [`secret`](secret.md) | `[REGISTRY]` | 打印登录凭据经加密口令加密后的加密串 |
| [`import`](import.md) | 无 | 把已登录注册表同步为 `[registries]` 条目（幂等） |
| [`registry`](registry.md) | TUI | 选择目标注册表，并同步 `[registries]` |
| [`passphrase`](passphrase.md) | TUI | 设置加密口令 `auth_passphrase` |
| [`config`](../config/wizard.md) | TUI 向导 | 逐步配置 `config.toml` |
| [`ci`](ci.md) | `[FILE]` | 输出工作流 YAML 模板 |
| [`help`](help.md) | 无 | 子命令简洁说明 |
| [`man`](help.md) | 无 | 完整帮助（逐段展开） |

## 覆盖参数（`sync`/`pull`/`spull` 通用） { #overrides }

全部可选，**单次执行生效，绝不写回 `config.toml`**：

| 参数 | 覆盖目标 | 说明 |
| --- | --- | --- |
| `--registry <域名>` / `-R` | `setting.target_registry` | 换目标注册表；须已登录 |
| `--org <组织>` / `-o` | `setting.target_org` | 优先级高于条目与全局值 |
| `--repo <仓库>` / `-r` | `setting.target_repo` | 优先级高于条目与全局值 |
| `--mode <1\|2\|3>` / `-m` | `setting.mode` | 其他取值报错退出，不发起远程请求 |

```bash
# 例：临时换目标注册表、组织与模式
docker mtrans sync alpine:latest -R new.example.com -o myorg -m 2
```

## 取值优先级

**命令行覆盖参数 > `[registries."<注册表>"]` 条目逐键 > `[setting]` 全局值 > 内置默认值**。全部省略时行为与不传参数完全一致。

## 帮助与语言

`--help` / `help` / `man` 与运行时输出（进度/成功/错误提示）按系统语言（`LANG`/`LC_ALL`/`LC_MESSAGES`，`zh*` 判中文）中英切换，**默认英文**。
