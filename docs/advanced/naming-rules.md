---
icon: lucide/git-branch
---
# 目标镜像组合规则

`sync` 与 `pull` 共用同一规则派生目标镜像，保证**可往返对账**。规则是纯函数，本地不维护同步状态。

## 派生步骤

1. **源镜像**须为 `name:tag` 形式；含 `@sha256:` 等 digest 引用一律报错，不支持。
2. **目标注册表 `R`** = `setting.target_registry`（当前选定）。
3. **生效组织/仓库逐键取值**：`[registries."R"]` 条目中已设置（非空）的 `target_org`/`target_repo` 优先，条目未设置的键以 `[setting]` 同名键为回退值。
4. 按 `mode` 组合（`ORG`/`REPO` 为第 3 步的生效值）：

| mode | 名称 | 公式 | tag 来源 |
| :-: | --- | --- | --- |
| `1` | 单仓库汇聚 | `{R}/{ORG}/{REPO}:{SRC_NAME}` | `SRC_NAME` = 源镜像最后一段路径名；源自带 tag 一律丢弃 |
| `2` | 仓库映射 | `{R}/{ORG}/{SRC_REPO}:{SRC_TAG}` | `SRC_REPO` = 源镜像最后一段路径名；源无 tag 视为 `latest` |
| `3` | 固定中转 | `{R}/{ORG}/{REPO}:mtrans` | 固定 tag `mtrans`，忽略源镜像名与 tag；`REPO` 缺省时用 `mtrans` |

## 示例

以目标注册表与全局默认目标路径为例（`registry.cn-guangzhou.aliyuncs.com` / `jetsung` / `myimage`）：

| 源镜像 | mode=1 结果 | mode=2 结果 | mode=3 结果 |
| --- | --- | --- | --- |
| `ghcr.io/idev/shortener:dev` | `…/jetsung/myimage:shortener` | `…/jetsung/shortener:dev` | `…/jetsung/myimage:mtrans` |
| `alpine:3.19` | `…/jetsung/myimage:alpine` | `…/jetsung/alpine:3.19` | `…/jetsung/myimage:mtrans` |

## 设计决策

??? note "mode 2 为什么不保留源组织（ADR-0003）"

    目标注册表（阿里云、腾讯云等镜像仓库）的命名空间只有一层，不支持 org 下再建子组织；拼入源组织会产生非法仓库名。因此 mode 2 只取源镜像最后一段路径名作为目标仓库名。

## 与 `pull` 的关系

拉回与复制共用同一规则：由源镜像直接推算出目标镜像名，**无需先执行 `sync`**。详见 [`pull`](../commands/pull.md)。
