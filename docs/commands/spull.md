---
icon: lucide/refresh-cw
---
# `spull` — 复制并拉回

连续执行 `sync` + `pull`：先触发远程复制，再从目标注册表拉回并重命名。

## 用法

```bash
docker mtrans spull <源镜像> [覆盖参数]
```

```bash
# 一条命令完成"复制到目标注册表 + 拉回本地"
docker mtrans spull alpine:latest

# 覆盖参数对两步同时生效
docker mtrans spull alpine:latest -R new.example.com -o myorg -m 2
```

## 行为说明

- 两步**共用同一份覆盖后的生效配置**：覆盖参数一次指定，`sync` 与 `pull` 同时生效。
- 前置条件为两者的并集：

| 条件 | 适用 |
| --- | --- |
| 目标注册表已登录 | `sync` 与 `pull` |
| `auth_passphrase` 已设置 | `sync` |
| 目标注册表中已存在镜像 | `pull`（复制成功后自然满足） |

!!! tip "适用场景"

    本机拿不到源镜像（网络受限）但需要它时：让托管 Runner 复制到目标注册表，再由本地从目标注册表拉回。
