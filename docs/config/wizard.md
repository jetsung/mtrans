---
icon: lucide/list-checks
---
# 交互向导

`docker mtrans config` 进入 TUI 向导，逐步输入并写回 `config.toml`，无需手工编辑文件。

## 运行

```bash
docker mtrans config
```

## 向导步骤

### 1. 目标注册表（`setting.target_registry`）

进入 TUI，从 `~/.docker/config.json` 已登录的注册表中选择填入；同时把已登录注册表同步更新到 `[registries]` 各注册表条目（与 `import` 同规则，已存在的条目不作任何修改）。

### 2. 目标组织 / 仓库（`setting.target_org` / `setting.target_repo`）

手动输入，**留空则不改变值**；提示语展示当前值（`回车保持 <当前值>`）。

### 3. 组合模式（`setting.mode`）

TUI 选择：`1`（单仓库汇聚）、`2`（仓库映射）或 `3`（固定中转），语义见[组合规则](../advanced/naming-rules.md)。

### 4. 流水线参数（`[ci]`）

逐项输入 `repo`、`branch`、`workflow`、`keep_run`，逻辑相同——**留空则不改变原值**。

## 写回行为

- 按**行文本改写**写回：替换键值、**保留原有注释与顺序**（ADR-0004），不使用 TOML 反序列化后重排。
- **任意步骤按 ++escape++ 取消整个向导，不写盘**——已输入内容全部丢弃。

!!! warning "先初始化，再设口令"

    `passphrase` 子命令要求配置文件已存在：文件不存在时报错，先运行 `docker mtrans config` 初始化。
