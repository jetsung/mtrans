---
icon: lucide/key-round
---
# `secret` — 打印加密凭据

把注册表登录凭据经 `auth_passphrase` 加密后的加密串（即 `sync` 传入流水线的 `target_auth_secret`）打印到 stdout。

## 用法

```bash
# 不跟参数：使用 target_registry（当前选定的目标注册表）
docker mtrans secret

# 指定注册表
docker mtrans secret docker.cnb.cool
```

## 行为说明

- 从 `~/.docker/config.json` 读取该注册表的 `auth`，用 `[setting].auth_passphrase` 加密后打印。
- **须已登录该注册表**；`auth_passphrase` 未设置时报错。
- 每次加密使用随机盐：同一凭据两次输出的密文**不同**，无法以密文做指纹比对。
- 只输出密文，**不还原明文凭据**。

!!! tip "用途"

    需要手工核对或配置流水线时，可把该加密串与仓库 Secret `AUTH_PASSPHRASE` 配对做解密验证；`sync` 触发时自动执行同样流程，一般无需手动调用。
