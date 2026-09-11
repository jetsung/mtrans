# mtrans SPEC（规范）

本规范以仓库根目录 [`config.example.toml`](../config.example.toml) 为唯一权威样例：键名、段落结构与取值口径均以它为准。
当代码实现与本规范不一致时，以本规范为准修正代码；本规范修订时须同步更新 `config.example.toml` 样例。

术语表以 [`CONTEXT.md`](CONTEXT.md) 为唯一来源；本文不再重复定义。

## 1. 概述

mtrans 把容器镜像从**源镜像**所在的注册表复制到**目标注册表**（当前选定的注册表，见 [CONTEXT.md](CONTEXT.md)），也可反向从目标注册表拉回本地并还原为源镜像名。复制**一律由 GitHub Actions 远程触发**完成：本地触发工作流，由托管 Runner 复制，本地轮询等待结束；不支持本地直连复制。

取值口径：源镜像走命令行（位置参数，如 `sync ghcr.io/jetsung/shortener:latest`），其余取值以 `config.toml` 为默认来源；`sync`/`pull`/`spull` 另支持可选覆盖参数单次替换 `setting` 下的 `target_registry`/`target_org`/`target_repo`/`mode`（见第 5 节，不写回配置文件）；环境变量中仅保留 `GITHUB_TOKEN` 作为 `[setting].github_token` 未设置时的回退。运行时输出（进度/成功/错误提示）与帮助文案一致，按系统语言（`LANG`/`LC_ALL`/`LC_MESSAGES`，`zh*` 判中文）中/英切换，默认英文。

平台：Windows、Linux、macOS 三端均可运行，详见第 8 节。

## 2. 构建与安装

- **一键脚本**：`curl -fsSL https://mtrans.gcli.cn/install.sh | bash`。
- **cargo install（推荐）**：`cargo install mtrans`（crates.io）或 `cargo install --git https://github.com/jetsung/mtrans.git`（Git 仓库），安装后直接调用 `docker-mtrans`。
- **构建**：仓库根目录执行 `cargo build --release`，产物为单文件 CLI（`target/release/docker-mtrans`）。
- **独立运行**：直接调用可执行文件，如 `docker-mtrans sync <源镜像>`。
- **docker 插件形态**：把可执行文件放入 docker CLI 插件目录后以 `docker mtrans ...` 调用——Unix/macOS `~/.docker/cli-plugins/`，Windows `%USERPROFILE%\.docker\cli-plugins\`；插件发现协议校验见第 8 节。

## 3. 配置文件

### 3.1 路径与加载口径

- 路径：`$XDG_CONFIG_HOME/mtrans/config.toml`；`XDG_CONFIG_HOME` 未设置时回退到平台标准配置目录（Linux/macOS `~/.config`，Windows `%APPDATA%`，由 `dirs::config_dir()` 解析）。
- 取值口径：配置文件中的值非空则生效，否则回退内置默认值。
- 容错：文件缺失或 TOML 解析失败时，打印告警并整体按内置默认值继续（不中断）。
- 类型约定：`keep_run` 为 TOML 布尔，`mode` 为整数，其余为字符串。

### 3.2 `[setting]`

| 键 | 类型 | 必填 | 内置默认值 | 说明 |
| --- | --- | :-: | --- | --- |
| `github_token` | string | 否 | 无 | GitHub Token，触发远程流水线用；未设置时回退环境变量 `GITHUB_TOKEN` |
| `auth_passphrase` | string | 否 | 无 | 加密口令：`sync` 时加密登录凭据成 `target_auth_secret`，须与目标仓库 GitHub Secret `AUTH_PASSPHRASE` 一致；未设置时 `sync` 报错（可用 `openssl rand -base64 24` 生成） |
| `target_registry` | string | 否 | `registry.cn-guangzhou.aliyuncs.com` | 目标注册表域名：当前使用的注册表，目标镜像推送的目的地 |
| `target_org` | string | 否 | `jetsung` | 目标组织；`[registries]` 条目同名键未设置时作为回退值 |
| `target_repo` | string | 否 | `myimage` | 目标仓库；`[registries]` 条目同名键未设置时作为回退值 |
| `mode` | int | 否 | `1` | 目标镜像组合模式，取值 `1`、`2` 或 `3`，语义见第 4 节 |

### 3.3 `[ci]`

| 键 | 类型 | 必填 | 内置默认值 | 说明 |
| --- | --- | :-: | --- | --- |
| `repo` | string | 否 | `jetsung/docker-build-sync` | 远程流水线 GitHub 仓库（`owner/name`）；复制一律经此仓库的工作流触发 |
| `branch` | string | 否 | `main` | 工作流分支 |
| `workflow` | string | 否 | `docker-mtrans.yml` | 工作流文件名 |
| `keep_run` | bool | 否 | `false` | 复制成功后是否保留该次流水线运行记录（连同日志）；默认成功即删除 |

### 3.4 `[registries."<注册表域名>"]`

- 表键即注册表域名：小写、不带 scheme、不带路径；Docker Hub 的键归一化为 `docker.io`。
- 条目为可选覆盖：仅当该注册表是本次同步的目标注册表（即等于 `target_registry`）时生效，并**逐键覆盖**——条目中已设置（非空）的 `target_org`/`target_repo` 覆盖 `[setting]` 同名键；条目未设置的键逐项回退到 `[setting]` 同名值。
- 条目键：

| 键 | 类型 | 必填 | 说明 |
| --- | --- | :-: | --- |
| `target_org` | string | 否 | 覆盖 `[setting].target_org`；未设置则回退 |
| `target_repo` | string | 否 | 覆盖 `[setting].target_repo`；未设置则回退 |

- 条目由 `import` / `registry` / `config` 向导同步写入，或用编辑器手动添加。

### 3.5 交互向导

`config` 进入 TUI 向导，逐步输入并写回 `config.toml`（按行文本改写，保留注释与顺序）；**任意步骤按 ESC 取消整个向导，不写盘**（已输入内容全部丢弃）：

1. `setting.target_registry`：进入 TUI，从 `~/.docker/config.json` 已登录的注册表中选择填入；同时把已登录注册表同步更新到 `[registries]` 各注册表条目（与 `import` 同规则，已存在的条目不作任何修改）。
2. `setting.target_org` / `setting.target_repo`：手动输入，留空则不改变值；提示语展示当前值（`回车保持 <当前值>`）。
3. `setting.mode`：TUI 选择——`1`（单仓库汇聚）、`2`（仓库映射）或 `3`（固定中转），语义见第 4 节。
4. `[ci]` 各参数（`repo`、`branch`、`workflow`、`keep_run`）：逐项输入，逻辑相同——留空则不改变原值。

## 4. 目标镜像组合规则

`sync` 与 `pull` 共用同一规则，保证可往返对账：

1. 源镜像须为 `name:tag` 形式；含 `@sha256:` 等 digest 引用一律报错，不支持。
2. 目标注册表 `R` = `[setting].target_registry`（当前选定）。
3. 生效目标组织/仓库**逐键取值**：`[registries."R"]` 条目中已设置（非空）的 `target_org`/`target_repo` 优先；条目未设置的键以 `[setting]` 同名键为回退值。
4. 按 `mode` 组合（`ORG`/`REPO` 为第 3 步的生效值）：

| mode | 公式 | tag 来源 |
| :-: | --- | --- |
| `1` | `{R}/{ORG}/{REPO}:{SRC_NAME}` | `SRC_NAME` = 源镜像最后一段路径名；源自带 tag 一律丢弃 |
| `2` | `{R}/{ORG}/{SRC_REPO}:{SRC_TAG}` | `SRC_REPO` = 源镜像最后一段路径名；源无 tag 时视为 `latest`。目标注册表不支持子组织，源组织段不保留 |
| `3` | `{R}/{ORG}/{REPO}:mtrans` | 固定中转：一律推到 `{REPO}:mtrans`，`REPO` 缺省时用 `mtrans`；忽略源镜像名与 tag |

5. 示例（目标注册表与全局默认目标路径）：

| 源镜像 | mode=1 结果 | mode=2 结果 | mode=3 结果 |
| --- | --- | --- | --- |
| `ghcr.io/idev/shortener:dev` | `registry.cn-guangzhou.aliyuncs.com/jetsung/myimage:shortener` | `registry.cn-guangzhou.aliyuncs.com/jetsung/shortener:dev` | `registry.cn-guangzhou.aliyuncs.com/jetsung/myimage:mtrans` |
| `alpine:3.19` | `registry.cn-guangzhou.aliyuncs.com/jetsung/myimage:alpine` | `registry.cn-guangzhou.aliyuncs.com/jetsung/alpine:3.19` | `registry.cn-guangzhou.aliyuncs.com/jetsung/myimage:mtrans` |

6. 拉回（`pull`）与复制共用同一组合规则：由源镜像直接推算出目标镜像名，**无需先执行 `sync`**。前置条件同 `sync`：目标注册表（`target_registry`）必须已登录，即 `~/.docker/config.json` 的 `auths` 下存在该注册表条目，未登录时直接报错。流程为：推算目标镜像 → 凭本地登录态从目标注册表直接拉取 → 重命名为源镜像名 → 删除本地的目标镜像标签，本地只留源镜像名。目标注册表中不存在该镜像时，不执行拉取，报错并提示先执行 `sync <源镜像>` 同步后再拉取。
   例（mode=2）：`pull ghcr.io/jetsung/shortener:latest` → 拉取 `registry.cn-guangzhou.aliyuncs.com/jetsung/shortener:latest` → 重命名为 `ghcr.io/jetsung/shortener:latest` 并删除本地该目标标签。

## 5. 命令面

| 子命令 | 参数 | 说明 |
| --- | --- | --- |
| `sync` | `<源镜像>` + 覆盖参数（可选） | 触发远程流水线把源镜像复制到目标注册表（须已登录该注册表），如 `sync ghcr.io/jetsung/shortener:latest` |
| `pull` | `<源镜像>` + 覆盖参数（可选） | 按第 4 节规则由源镜像推算目标，凭本地登录态从目标注册表直接拉取并重命名为源镜像名（须已登录该注册表，无需先 `sync`；目标不存在时提示先 `sync`），如 `pull ghcr.io/jetsung/shortener:latest` |
| `spull` | `<源镜像>` + 覆盖参数（可选） | 连续执行 `sync` + `pull`，两步共用同一份覆盖后的生效配置 |
| `secret` | `[REGISTRY]`（可选） | 打印注册表登录凭据经 `auth_passphrase` 加密后的加密串（即 `sync` 传入流水线的 `target_auth_secret`）到 stdout：不跟参数时用 `target_registry`；指定注册表（如 `secret docker.cnb.cool`）则用指定值。均从 `~/.docker/config.json` 读取该注册表的 `auth` 并加密后打印（须已登录该注册表；`auth_passphrase` 未设置时报错） |
| `import` | 无 | 把 `~/.docker/config.json` 中已登录的注册表同步为 `[registries."<注册表域名>"]` 条目（如 `[registries."registry.cn-guangzhou.aliyuncs.com"]`）；`config.toml` 中已存在的注册表跳过、不作任何修改（幂等）；凭据留在原地，不写入条目 |
| `registry` | 无（TUI 交互） | 选择目标注册表：从 `~/.docker/config.json` 已登录的注册表中选择，填入 `setting.target_registry`；成功后同步各已登录注册表到 `[registries]` 条目（已存在的跳过） |
| `passphrase` | 无（TUI 交互） | 设置加密口令（写入 `[setting].auth_passphrase`）：保持原值 / 自动生成随机口令 / 手动输入，与 `config` 向导第 1 步相同；配置文件不存在时报错（先运行 `config` 初始化）。写盘后回显口令值并提示同步到 GitHub Secret `AUTH_PASSPHRASE` |
| `config` | 无（TUI 向导） | 逐步配置 `config.toml`，见 3.5 |
| `ci` | `[FILE]`（可选） | 输出工作流 YAML 模板：不跟参数时打印到 stdout；给定文件名（如 `ci test.yaml`）时保存到该文件，文件已存在时提示是否覆盖 |
| `help` | 无 | 查看子命令简洁说明；文案按系统语言（`LANG`/`LC_ALL`/`LC_MESSAGES`，`zh*` 判中文）中/英切换，默认英文 |
| `man` | 无 | 查看完整帮助（各子命令详细用法逐段展开）；文案同 `help` 按系统语言切换 |

覆盖参数（`sync`/`pull`/`spull` 通用，全部可选；单次执行生效，**绝不写回 `config.toml`**）：

| 参数 | 覆盖目标 | 说明 |
| --- | --- | --- |
| `--registry <域名>` / `-R` | `setting.target_registry` | 换目标注册表；条目查找随之使用新域名，且该注册表须已登录 |
| `--org <组织>` / `-o` | `setting.target_org` | 优先级高于 `[registries]` 条目与 `[setting]` |
| `--repo <仓库>` / `-r` | `setting.target_repo` | 优先级高于 `[registries]` 条目与 `[setting]` |
| `--mode <1\|2\|3>` / `-m` | `setting.mode` | 其他取值报错退出，不发起远程请求 |

取值优先级：**命令行覆盖参数 > `[registries."<注册表>"]` 条目逐键 > `[setting]` 全局值 > 内置默认值**。全部省略时行为与不传参数完全一致。

执行方式（仅此一种）：

- **远程触发**：以 `[setting].github_token`（或回退 `GITHUB_TOKEN`）触发 `repo@branch` 的 `workflow` 工作流，inputs 携带源镜像、目标镜像与 `target_auth_secret`（登录凭据经加密口令加密的加密串）；本地轮询等待运行结束；成功后默认删除该次运行（`keep_run = true` 保留）。
- **前置条件**：目标注册表（`target_registry`）必须已登录，即 `~/.docker/config.json` 的 `auths` 下已存在该注册表条目；未登录时 `sync` 直接报错。触发时本地读取该条目的 `auth` 字段（`auths."<target_registry>".auth`），用 `[setting].auth_passphrase` 加密成 `target_auth_secret` 传入，流水线内解密并打掩码（见第 6 节）。`auth_passphrase` 未设置时 `sync` 报错。

## 6. 登录凭据传递与打掩码契约

- `target_auth_secret` 为登录凭据 `auth`（`~/.docker/config.json` 中 `auths."<注册表域名>".auth` 的原值，base64 编码的 `用户名:密码`）经加密口令 `[setting].auth_passphrase` 加密后的加密串；本地不落任何配置文件，触发时加密后作为 workflow input 传入。
- **加密算法与流水线解密命令逐字节配对**：`openssl enc -aes-256-cbc -pbkdf2 -a -A -pass "pass:<auth_passphrase>"`（`Salted__` magic + 8 字节随机盐 + AES-256-CBC/PKCS7，PBKDF2-HMAC-SHA256 10000 轮派生 key 32 字节 + IV 16 字节，输出单行 base64）；流水线内用 GitHub Secret `AUTH_PASSPHRASE`（值须与 `auth_passphrase` 一致）以 `openssl enc -d ...` 解密。
- **凭据与加密口令是密钥，绝不得打印到流水线日志**。runner 回显 run 脚本时会把内联 `${{ }}` 表达式替换为实际值（先于 `::add-mask::` 执行，必然明文落日志），因此 `target_auth_secret` 与 `secrets.AUTH_PASSPHRASE` **绝不得在 run 脚本中内联**，而是经 step 级 `env:` 传入登录步骤（env 值不回显）；脚本内用 `::add-mask::` 依次对 `AUTH_PASSPHRASE`、解密出的 `auth`（base64）、解码出的 `用户名`、`密码` 分别打掩码（ADR-0002）；打掩码后再进行 `docker login` 与拉取/推送。
- 由于 `::add-mask::` 仅对命令执行后的日志生效，模板把 env 传入、解密、解码、打掩码、`docker login` 收敛为**同一步骤**执行（见 `docker mtrans ci` 生成的 YAML）。
- 工作流内不保存凭据；运行记录（含日志）在成功且 `keep_run = false` 时由本地自动删除，进一步降低泄露面。

## 7. 键名速查表

```toml
[setting]
github_token     # 可选，回退环境变量 GITHUB_TOKEN
auth_passphrase         # 加密口令：加密登录凭据，须与 Secret AUTH_PASSPHRASE 一致
target_registry  # 目标注册表域名（当前使用）
target_org       # 目标组织
target_repo      # 目标仓库
mode             # 1 | 2 | 3，目标镜像组合模式

[ci]
repo            # 远程流水线仓库（复制一律经其工作流触发）
branch          # 分支
workflow        # 工作流文件名
keep_run        # bool，成功后保留流水线记录

[registries."<注册表域名>"]
target_org         # 可选，覆盖全局
target_repo        # 可选，覆盖全局
```

## 8. 平台支持

Windows、Linux、macOS 三端均可构建运行，跨平台差异仅在本地连接 Docker 的默认端点与配置文件路径：

| 平台 | 默认 Docker 端点 | 应用配置文件路径（`XDG_CONFIG_HOME` 未设置时） | Docker 配置路径（`DOCKER_CONFIG` 未设置时） |
| --- | --- | --- | --- |
| Linux | `/var/run/docker.sock` | `~/.config/mtrans/config.toml` | `~/.docker/config.json` |
| macOS | `/var/run/docker.sock` | `~/.config/mtrans/config.toml` | `~/.docker/config.json` |
| Windows | `\\.\pipe\docker_engine` | `%APPDATA%\mtrans\config.toml` | `%USERPROFILE%\.docker\config.json` |

- **端点解析**：`DOCKER_HOST` > `DOCKER_CONTEXT` > `config.json` 的 `currentContext` > 平台默认端点；`DOCKER_HOST` 支持 `unix://`、`tcp://`、`http://`、`npipe://` 前缀，TLS（`https://`）与 `ssh://` 不支持，需自建隧道转成明文端点。
- **配置路径**：应用配置以 `XDG_CONFIG_HOME` 优先，未设置时回退 `dirs::config_dir()`（各平台标准目录）；Docker 配置与 context 目录均尊重 `DOCKER_CONFIG` 覆盖。
- **插件形态**：`docker mtrans ...` 需把可执行文件放入 docker 插件目录——Windows `%USERPROFILE%\.docker\cli-plugins\`，Unix/macOS `~/.docker/cli-plugins\`。
- **CI**：`.github/workflows/ci.yml` 在 ubuntu/macos/windows 三系统跑 clippy、test、release 构建，并校验插件发现协议。
