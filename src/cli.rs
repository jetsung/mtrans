//! CLI 参数定义（clap）。帮助整体自渲染，`docker mtrans --help` 的输出即本模块的输出。
//!
//! 参数口径：**只有源镜像（位置参数）走命令行**，其余取值全部来自 config.toml（SPEC 第 5 节）。

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "docker-mtrans",
    bin_name = "docker mtrans",
    about = "Docker 镜像复制工具：触发远程 GitHub Actions 流水线把源镜像复制到目标注册表，也可凭本地登录态从目标注册表拉回",
    disable_help_subcommand = true,
    disable_version_flag = true,
    help_template = r#"{about}

用法: {usage}

子命令:

  镜像复制:
    sync <源镜像>       触发远程流水线复制到目标注册表
    pull <源镜像>       从目标注册表拉回并重命名为源镜像名
    spull <源镜像>      sync + pull 两步一气呵成

  凭据:
    secret [注册表]     打印登录凭据加密串（auth_passphrase 加密）
    passphrase          TUI 设置加密口令 auth_passphrase
    import              同步已登录注册表到 [registries]

  配置:
    registry            TUI 选择目标注册表
    config              TUI 向导：口令/注册表/org/repo/mode/[ci]

  工作流模板:
    ci [文件]           输出 docker-mtrans.yml 工作流 YAML 模板

  帮助:
    version             查看版本信息
    help                查看子命令简洁说明
    man                 查看完整帮助（详细用法）

选项:
  -h, --help     显示帮助信息
"#
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// 复制镜像：触发远程 GitHub Actions 流水线复制到目标注册表
    #[command(about = "复制镜像：触发远程 GitHub Actions 流水线复制到目标注册表\n
        源镜像      位置参数，如 ghcr.io/jetsung/shortener:latest（不支持 @sha256: digest）
        前置条件    目标注册表已登录（~/.docker/config.json 的 auths 下存在条目）
        凭据        触发时用 auth_passphrase 把登录凭据加密成 target_auth_secret 传入
        其余取值    全部来自 config.toml；-R/-o/-r/-m 可单次覆盖，不写回配置文件")]
    Sync {
        /// 源镜像，如 ghcr.io/jetsung/shortener:latest（不支持 @sha256: digest）
        source_image: String,

        /// 单次覆盖 [setting] 的 target_registry（不写回配置文件）
        #[arg(short = 'R', long, value_name = "REGISTRY")]
        registry: Option<String>,

        /// 单次覆盖 [setting] 的 target_org（优先级高于 [registries] 条目）
        #[arg(short = 'o', long, value_name = "ORG")]
        org: Option<String>,

        /// 单次覆盖 [setting] 的 target_repo（优先级高于 [registries] 条目）
        #[arg(short = 'r', long, value_name = "REPO")]
        repo: Option<String>,

        /// 单次覆盖 [setting] 的 mode（1/2/3）
        #[arg(
            short = 'm',
            long,
            value_name = "MODE",
            value_parser = clap::value_parser!(u8).range(1..=3)
        )]
        mode: Option<u8>,
    },

    /// 拉回镜像：从目标注册表拉取并重命名为源镜像名
    #[command(about = "拉回镜像：从目标注册表拉取并重命名为源镜像名\n
        目标推算    按组合规则由源镜像推算（无需先 sync）
        流程        拉取目标镜像 → 重命名为源镜像名 → 删除本地目标标签
        目标不存在  报错并提示先 sync <源镜像>
        前置条件    目标注册表已登录；其余同 sync（-R/-o/-r/-m 单次覆盖）")]
    Pull {
        /// 源镜像，如 ghcr.io/jetsung/shortener:latest
        source_image: String,

        /// 单次覆盖 [setting] 的 target_registry（不写回配置文件）
        #[arg(short = 'R', long, value_name = "REGISTRY")]
        registry: Option<String>,

        /// 单次覆盖 [setting] 的 target_org（优先级高于 [registries] 条目）
        #[arg(short = 'o', long, value_name = "ORG")]
        org: Option<String>,

        /// 单次覆盖 [setting] 的 target_repo（优先级高于 [registries] 条目）
        #[arg(short = 'r', long, value_name = "REPO")]
        repo: Option<String>,

        /// 单次覆盖 [setting] 的 mode（1/2/3）
        #[arg(
            short = 'm',
            long,
            value_name = "MODE",
            value_parser = clap::value_parser!(u8).range(1..=3)
        )]
        mode: Option<u8>,
    },

    /// 先同步再拉回：sync + pull 两步一气呵成
    #[command(
        name = "spull",
        about = "先同步再拉回：sync + pull 两步一气呵成\n
        流程        sync（远程复制）成功后立即 pull（拉回并重命名）
        覆盖参数    -R/-o/-r/-m 两步共用同一份覆盖后的生效配置"
    )]
    SyncPull {
        /// 源镜像，如 ghcr.io/jetsung/shortener:latest
        source_image: String,

        /// 单次覆盖 [setting] 的 target_registry（不写回配置文件）
        #[arg(short = 'R', long, value_name = "REGISTRY")]
        registry: Option<String>,

        /// 单次覆盖 [setting] 的 target_org（优先级高于 [registries] 条目）
        #[arg(short = 'o', long, value_name = "ORG")]
        org: Option<String>,

        /// 单次覆盖 [setting] 的 target_repo（优先级高于 [registries] 条目）
        #[arg(short = 'r', long, value_name = "REPO")]
        repo: Option<String>,

        /// 单次覆盖 [setting] 的 mode（1/2/3）
        #[arg(
            short = 'm',
            long,
            value_name = "MODE",
            value_parser = clap::value_parser!(u8).range(1..=3)
        )]
        mode: Option<u8>,
    },

    /// 打印登录凭据加密串（auth_passphrase 加密，即 target_auth_secret）
    #[command(
        about = "打印登录凭据加密串（auth_passphrase 加密，即 target_auth_secret）\n
        注册表      不跟参数时用 target_registry；指定注册表（如 docker.cnb.cool）则用指定值
        输出        auth 经 auth_passphrase 加密后的加密串（每次随机盐，密文不同）
        前置条件    已登录该注册表；config.toml 已设置 auth_passphrase（未设置时报错）"
    )]
    Secret {
        /// 注册表域名，如 docker.cnb.cool；缺省用 target_registry
        registry: Option<String>,
    },

    /// 同步已登录注册表到 [registries] 条目（幂等）
    #[command(about = "同步已登录注册表到 [registries] 条目（幂等）\n
        来源        ~/.docker/config.json 的 auths（登录即有）
        规则        已存在的注册表跳过、不作任何修改；凭据留在原地，不写入条目")]
    Import,

    /// TUI 选择目标注册表（写入 target_registry）
    #[command(about = "TUI 选择目标注册表（写入 target_registry）\n
        候选        ~/.docker/config.json 已登录的注册表
        联动        成功后同步各已登录注册表到 [registries] 条目（已存在的跳过）")]
    Registry,

    /// TUI 设置加密口令（写入 [setting].auth_passphrase）
    #[command(about = "TUI 设置加密口令（写入 [setting].auth_passphrase）\n
        选项        保持原值 / 自动生成随机口令 / 手动输入（与 config 向导第 1 步相同）
        联动        写盘后回显口令值，须同步到 GitHub Secret AUTH_PASSPHRASE
        前置条件    config.toml 已存在（否则先运行 config 初始化）")]
    Passphrase,

    /// TUI 向导逐步配置 config.toml
    #[command(about = "TUI 向导逐步配置 config.toml\n
        步骤        1 加密口令 → 2 目标注册表（同步 registries）→ 3 org/repo → 4 mode → 5 [ci]
        交互        逐步输入，回车保持原值或默认值；任意步骤按 ESC 取消（不写盘）")]
    Config,

    /// 输出 docker-mtrans.yml 工作流 YAML 模板
    #[command(about = "输出 docker-mtrans.yml 工作流 YAML 模板\n
        文件        不跟参数时打印到 stdout；指定文件名（如 ci test.yaml）时保存到该文件
        覆盖        文件已存在时提示是否覆盖
        部署        保存后提交到 [ci].repo 仓库（与远程部署的工作流须逐字节一致）")]
    Ci {
        /// 输出文件名；缺省打印到 stdout
        file: Option<String>,
    },

    /// 显示版本信息
    #[command(about = "显示版本信息")]
    Version,

    /// 显示完整的帮助信息
    #[command(about = "显示完整的帮助信息")]
    Help,

    /// 查看完整帮助（详细用法）
    #[command(about = "查看完整帮助（详细用法）")]
    Man,
}
