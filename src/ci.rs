//! 工作流模板常量：必须与远程仓库实际部署的 `docker-mtrans.yml` 逐字节一致。
//! 凭据以 `target_auth_secret`（本地用加密口令 AES-256-CBC+PBKDF2 加密的 auth）传入，
//! 流水线内用 GitHub Secret `AUTH_PASSPHRASE` 经 openssl 解密；凭据经 step 级 `env:`
//! 传入（runner 回显 run 脚本时会把内联 `${{ }}` 替换为实际值、在掩码前泄漏，
//! 故禁止内联），脚本内 `::add-mask::` 打掩码，绝不得打印到日志（ADR-0002）。

use crate::i18n;

pub const CI_TEMPLATE: &str = r#"name: Mtrans Sync

on:
  workflow_dispatch:
    inputs:
      source_image:
        description: '源镜像'
        required: true
        type: string
      target_image:
        description: '目标镜像（必须包含注册表域名，例如 ghcr.io/user/repo:tag）'
        required: true
        type: string
      target_auth_secret:
        description: '目标注册表登录凭据加密串（本地用加密口令 AES-256-CBC+PBKDF2 加密的 auth）'
        required: true
        type: string

permissions:
  contents: read
  packages: write

jobs:
  copy-image:
    runs-on: ubuntu-24.04
    env:
      SOURCE_IMAGE: ${{ inputs.source_image }}
      TARGET_IMAGE: ${{ inputs.target_image }}
    steps:
      - name: 解密认证信息并登录目标注册表
        env:
          # 加密串与加密口令必须经 env 传递：runner 回显 run 脚本时会把 ${{ }} 表达式
          # 替换为实际值（内联读取会在 add-mask 执行前的脚本回显中泄漏）；env 值不回显，
          # 脚本回显中只有变量名。绝不得在 run 脚本中内联 ${{ inputs.* }} 或 ${{ secrets.* }}。
          TARGET_AUTH_SECRET: ${{ inputs.target_auth_secret }}
          AUTH_PASSPHRASE: ${{ secrets.AUTH_PASSPHRASE }}
        run: |
          set -euo pipefail

          # 1. 首行对加密口令打掩码：后续任何回显均显示为 ***
          echo "::add-mask::${AUTH_PASSPHRASE}"

          # 2. 用密钥解密得到 auth（base64 编码的 username:password），随即打掩码
          AUTH="$(printf '%s' "$TARGET_AUTH_SECRET" | openssl enc -d -aes-256-cbc -pbkdf2 -a -A -pass "pass:$AUTH_PASSPHRASE")"
          echo "::add-mask::${AUTH}"

          # 3. base64 解码得到 username:password，按第一个冒号拆分，各自打掩码
          CRED="$(printf '%s' "$AUTH" | base64 -d)"
          USERNAME="${CRED%%:*}"
          PASSWORD="${CRED#*:}"
          if [ -z "$USERNAME" ] || [ -z "$PASSWORD" ]; then
            echo "::error::凭据解析失败，请确认 target_auth_secret 加密串与 Secret AUTH_PASSPHRASE 配对" >&2
            exit 1
          fi
          echo "::add-mask::${USERNAME}"
          echo "::add-mask::${PASSWORD}"

          # 4. 从目标镜像提取注册表地址（第一个斜杠前的部分，如 ghcr.io/user/repo:tag -> ghcr.io）
          if [[ "$TARGET_IMAGE" != */* ]]; then
            echo "::error::target_image 必须包含注册表地址，例如 ghcr.io/user/repo:tag" >&2
            exit 1
          fi
          REGISTRY="${TARGET_IMAGE%%/*}"

          # 5. 登录目标注册表（密码通过 stdin 传入，不进入命令行参数与日志）
          printf '%s' "$PASSWORD" | docker login "$REGISTRY" --username "$USERNAME" --password-stdin

      - name: 复制镜像
        run: |
          set -euo pipefail
          echo "source-image: $SOURCE_IMAGE"
          echo "target-image: $TARGET_IMAGE"
          docker pull "$SOURCE_IMAGE"
          docker tag "$SOURCE_IMAGE" "$TARGET_IMAGE"
          docker push "$TARGET_IMAGE"
"#;

/// `docker mtrans ci [FILE]`：输出工作流模板。
/// 不跟参数时打印到 stdout；指定文件名时保存，文件已存在时提示是否覆盖。
pub fn generate(file: Option<&str>) -> Result<(), String> {
    let Some(file) = file else {
        print!("{CI_TEMPLATE}");
        return Ok(());
    };
    let output = std::env::current_dir()
        .map_err(|e| i18n::err_current_dir(&e))?
        .join(file);
    if output.exists() && !crate::wizard::confirm(&i18n::confirm_overwrite(&output)) {
        println!("{}", i18n::cancelled_no_write());
        return Ok(());
    }
    std::fs::write(&output, CI_TEMPLATE).map_err(|e| i18n::err_write_failed(&output, &e))?;
    println!("{}", i18n::ok_written(&output));
    Ok(())
}
