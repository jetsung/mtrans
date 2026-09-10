# mtrans

把容器镜像复制到指定注册表、并可反向拉回本地的 CLI 工具。本文件是项目唯一的术语表，工程与文档中的措辞以这里为准。

## 术语

**目标注册表（target registry）**:
当前选定的、存放复制结果的注册表域名（`setting.target_registry`）。

**注册表条目（registry entry）**:
`[registries]` 表中以注册表域名为键的条目，对其中的 `target_org`/`target_repo` 逐键覆盖全局值。

**源镜像（source image）**:
命令行给出的待复制镜像完整引用（含注册表域名与 tag），是唯一的命令行输入。

**目标镜像（target image）**:
由目标注册表与目标路径按组合模式从源镜像派生出的复制结果引用，`sync` 与 `pull` 共用同一派生规则。

**目标路径（target path）**:
目标镜像去掉注册表域名与 tag 后的路径，由 `target_org`/`target_repo` 与源镜像段组成。

**单仓库汇聚（mode 1）**:
组合模式之一：所有源镜像汇入固定的 `target_org/target_repo`，源镜像名作 tag，源 tag 丢弃。

**仓库映射（mode 2）**:
组合模式之二：`target_org` 下按源镜像名建仓库，tag 沿用源 tag；不保留源组织。

**登录凭据（auth）**:
`~/.docker/config.json` 中 `auths."<注册表域名>".auth` 的值，即 base64 编码的 `用户名:密码`；`sync` 触发时用加密口令（`setting.auth_passphrase`）加密成 workflow input `target_auth_secret` 传入，流水线内解密并打掩码，不落任何配置文件。

**加密口令（auth_passphrase）**:
加密登录凭据所用的对称密钥（`setting.auth_passphrase`），与目标仓库的 GitHub Secret `AUTH_PASSPHRASE` 保持一致；流水线内用它解密 `target_auth_secret`，绝不得打印到日志。
