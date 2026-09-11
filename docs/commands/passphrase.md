---
icon: lucide/shield-check
---
# `passphrase` — 设置加密口令

TUI 交互：设置加密口令（写入 `[setting].auth_passphrase`）。

## 用法

```bash
docker mtrans passphrase
```

## 三个选项

| 选项 | 行为 |
| --- | --- |
| 保持原值 | 不改动现有口令 |
| 自动生成 | 生成随机口令并写入 |
| 手动输入 | 自己输入口令 |

## 行为说明

- 与 `config` 向导第 1 步相同的 TUI。
- **配置文件不存在时报错**——先运行 `docker mtrans config` 初始化。
- 写盘后回显口令值，并提示**同步到 GitHub Secret `AUTH_PASSPHRASE`**：口令必须与目标仓库的 Secret 保持一致，流水线才能解密 `target_auth_secret`。

!!! warning "口令的两处一致性"

    `[setting].auth_passphrase`（本地）与 Secret `AUTH_PASSPHRASE`（`[ci].repo` 仓库）必须一致。轮换密钥时**两处同步更新**，否则 `sync` 的复制会在解密步骤失败。口令生成建议：`openssl rand -base64 24`。

凭据加密与打掩码的完整契约见[凭据加密与打掩码](../advanced/credentials.md)。
