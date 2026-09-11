---
icon: lucide/file-code
---
# `ci` — 输出工作流模板

输出 GitHub Actions 工作流 YAML 模板（`docker-mtrans.yml`），用于部署到 `[ci].repo` 仓库。

## 用法

```bash
# 不跟参数：打印到 stdout
docker mtrans ci

# 给定文件名：保存到该文件，文件已存在时提示是否覆盖
docker mtrans ci docker-mtrans.yml
```

## 工作流做什么

模板实现 `workflow_dispatch` 触发的镜像复制：

1. **解密凭据并登录**：用 Secret `AUTH_PASSPHRASE` 解密 input `target_auth_secret` 得到登录凭据，`::add-mask::` 打掩码后 `docker login` 目标注册表。
2. **复制镜像**：`docker pull` 源镜像 → `docker tag` 为目标镜像 → `docker push` 到目标注册表。

??? abstract "关键安全约束（ADR-0002）"

    - `target_auth_secret` 与 `secrets.AUTH_PASSPHRASE` 绝不在 run 脚本中内联 `${{ }}`——runner 回显脚本时会先替换为实际值，必然明文落日志；而是经 step 级 `env:` 传入（env 值不回显）。
    - 脚本内依次对加密口令、`auth`（base64）、用户名、密码打掩码，再进行 `docker login` 与拉取/推送。
    - 密码经 `--password-stdin` 传入，不进命令行参数。
    - 解密、解码、打掩码、登录收敛为**同一步骤**执行。

## 部署步骤

1. `docker mtrans ci docker-mtrans.yml` 生成模板。
2. 把文件放到 `[ci].repo` 仓库的 `.github/workflows/docker-mtrans.yml`。
3. 在该仓库 **Settings → Secrets and variables → Actions** 添加 Secret `AUTH_PASSPHRASE`，值与本地 `[setting].auth_passphrase` 一致。

完整说明见[部署 GitHub Actions 工作流](../advanced/workflow.md)。
