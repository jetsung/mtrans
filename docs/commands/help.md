---
icon: lucide/circle-help
---
# `help` / `man` — 查看帮助

两个内置帮助命令，文案按系统语言切换（`LANG`/`LC_ALL`/`LC_MESSAGES`，`zh*` 判中文，**默认英文**）。

| 命令 | 内容 |
| --- | --- |
| `docker mtrans help` | 子命令简洁说明（一句话一条） |
| `docker mtrans man` | 完整帮助，各子命令详细用法逐段展开 |
| `docker mtrans <子命令> --help` | 该子命令的帮助（clap 生成，与 `help`/`man` 同语言规则） |

```bash
docker mtrans help
docker mtrans man
docker mtrans sync --help
```

帮助信息采用「一行摘要 + 空行 + 缩进字段表」结构，字段名对齐，便于快速定位。
