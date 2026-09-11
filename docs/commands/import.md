---
icon: lucide/download
---
# `import` — 同步登录注册表

把 `~/.docker/config.json` 中已登录的注册表同步为 `[registries."<注册表域名>"]` 条目。

## 用法

```bash
docker mtrans import
```

## 行为说明

- 已登录注册表逐一生成条目，例如：

  ```toml
  [registries."registry.cn-guangzhou.aliyuncs.com"]
  ```

- **幂等**：`config.toml` 中已存在的注册表条目跳过、不作任何修改。
- 凭据留在 `~/.docker/config.json` 原地，**不写入**条目——条目只有 `target_org`/`target_repo` 两个可选覆盖键。

??? note "条目何时生效"

    仅当该注册表是本次同步的目标注册表（等于 `target_registry`）时，条目中的 `target_org`/`target_repo` 才逐键覆盖 `[setting]` 同名键，见[配置总览](../config/index.md#registries)。

`registry` 与 `config` 向导第 1 步也会以同样规则同步条目，功能上互为补充。
