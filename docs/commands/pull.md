---
icon: lucide/cloud-download
---
# `pull` — 反向拉回

按与 `sync` **相同的组合规则**由源镜像推算出目标镜像，凭本地登录态从目标注册表直接拉取并重命名为源镜像名。

## 用法

```bash
docker mtrans pull <源镜像> [覆盖参数]
```

```bash
docker mtrans pull ghcr.io/jetsung/shortener:latest
```

## 执行流程

1. 按[组合规则](../advanced/naming-rules.md)推算目标镜像（**无需先执行 `sync`**）。
2. 凭本地登录态从目标注册表直接拉取目标镜像。
3. 重命名为源镜像名，并删除本地的目标镜像标签——本地只留源镜像名。

??? example "完整示例（mode=2）"

    ```text
    pull ghcr.io/jetsung/shortener:latest
      → 拉取 registry.cn-guangzhou.aliyuncs.com/jetsung/shortener:latest
      → 重命名为 ghcr.io/jetsung/shortener:latest
      → 删除本地目标标签
    ```

## 前置条件

- **目标注册表已登录**：`~/.docker/config.json` 的 `auths` 下存在该注册表条目，未登录时直接报错。
- 目标注册表中**已存在**该镜像：不存在时不执行拉取，报错并提示先执行 `sync <源镜像>` 同步后再拉取。

!!! note "为什么不需要先 sync"

    组合规则是 `sync` 与 `pull` 共用的纯函数（ADR-0003），本地不维护同步状态，`pull` 可直接从源镜像名推算出目标镜像名。
