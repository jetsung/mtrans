# 登录凭据加密传输与流水线日志打掩码

凭据（`~/.docker/config.json` 中 `auths."<注册表域名>".auth`，base64 编码的 `用户名:密码`）不写入任何配置文件。`sync` 触发时本地读取该 `auth`，用 `[setting].auth_passphrase`（加密口令）以 `openssl enc -aes-256-cbc -pbkdf2 -a -A` 兼容格式（`Salted__` magic + 8 字节随机盐 + AES-256-CBC/PKCS7，PBKDF2-HMAC-SHA256 10000 轮派生 key 32 字节 + IV 16 字节，输出单行 base64）加密成加密串，作为 workflow input `target_auth_secret` 传入；流水线内用仓库 Secret `AUTH_PASSPHRASE`（与 `auth_passphrase` 一致）经 openssl 解密。

## 动机

- 传输通道只暴露密文，拿到 run payload 也无法还原凭据；
- 加密口令只存在于两处：本地 `config.toml` 与目标仓库 Secret，均不在事件 payload 中；
- 每次加密使用随机盐，同一凭据两次触发的密文不同，无法以密文做指纹比对；
- 配置文件只存加密口令，凭据本体始终只在 `~/.docker/config.json`。

## 流水线日志打掩码

GitHub Actions 提供 `::add-mask::` 命令：在日志输出中把匹配的字符串替换为 `***`。但掩码**仅对在命令执行之后的日志生效**，且 runner 执行 run 脚本前会先回显整个脚本——`${{ }}` 表达式在回显时**已被替换为实际值**。因此 `target_auth_secret` 与 `secrets.AUTH_PASSPHRASE` **绝不得在 run 脚本中内联 `${{ }}`**，而是经 step 级 `env:` 传入（env 值不回显，脚本回显中只有变量名）；脚本内用 `::add-mask::` 依次对 `AUTH_PASSPHRASE`、解密出的 `auth`（base64）、解码出的 `用户名`、`密码` 分别打掩码后再进行 `docker login` 与拉取/推送。

解密、解码、打掩码、`docker login` 收敛为**同一步骤**执行（见 `docker mtrans ci` 生成的 YAML）。密码经 stdin 传入 `docker login ... --password-stdin`，不进命令行参数。

## 代价

- `sync`/`pull` 前必须已 `docker login` 目标注册表（未登录在入口报错）；
- `auth_passphrase` 须与目标仓库 Secret `AUTH_PASSPHRASE` 保持一致：轮换密钥时两处同步更新；
- `auth_passphrase` 未设置时 `sync` 报错（生成建议：`openssl rand -base64 24`）；
- 本地须实现与 openssl 逐字节配对的加解密（`src/crypto.rs`，以 openssl 参考向量 + 真实互解测试锚定）；
- 工作流内不保存凭据；运行记录（含日志）在成功且 `keep_run = false` 时由本地自动删除，进一步降低泄露面；
- `secret [REGISTRY]` 子命令按需在本地 stdout 打印 `auth` 经 `auth_passphrase` 加密后的加密串（即 `target_auth_secret`，每次随机盐、密文不同），不还原明文凭据。
