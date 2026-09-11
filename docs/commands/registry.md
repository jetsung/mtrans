---
icon: lucide/list
---
# `registry` — 选择目标注册表

TUI 交互：从 `~/.docker/config.json` 已登录的注册表中选择，填入 `setting.target_registry`。

## 用法

```bash
docker mtrans registry
```

## 行为说明

1. 列出 `~/.docker/config.json` 中已登录的注册表，TUI 选择。
2. 选中项写入 `setting.target_registry`（当前选定的目标注册表）。
3. 成功后同步各已登录注册表到 `[registries]` 条目——与 `import` 同规则，**已存在的条目跳过**。

!!! note "前置条件"

    需要先 `docker login` 目标注册表，列表中才会出现它。未登录任何注册表时列表为空。
