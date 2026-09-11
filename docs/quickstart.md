---
icon: lucide/rocket
---

# 快速开始

本页带你用最短路径完成第一次镜像复制：**登录目标注册表 → 初始化配置 → 部署工作流 → 同步**。

## 0. 前置条件

- 本机已安装 Docker，并已 `docker login` **目标注册表**（复制结果的推送目的地）。
- 一个用于触发流水线的 GitHub 仓库（`[ci].repo`，默认 `jetsung/docker-build-sync`），且其上已部署工作流（见第 3 步）。
- 具备 `actions: write` 权限的 GitHub Token（配置文件中设置，或回退环境变量 `GITHUB_TOKEN`）。

## 1. 登录目标注册表

```bash
# 例：登录阿里云广州（目标注册表须已登录，sync 才能读取凭据）
docker login registry.cn-guangzhou.aliyuncs.com
```

## 2. 初始化配置

```bash
docker mtrans config        # TUI 向导，逐步完成 setting 与 [ci] 配置
docker mtrans registry      # 从已登录注册表中选择目标注册表
docker mtrans passphrase    # 设置加密口令（保持 / 自动生成 / 手动输入）
```

向导中任意步骤按 ++escape++ 取消整个向导，不写盘。

## 3. 部署工作流

把工作流模板部署到 `[ci].repo` 仓库的 `.github/workflows/docker-mtrans.yml`，并配置仓库 Secret：

```bash
docker mtrans ci docker-mtrans.yml   # 生成工作流 YAML 到本地文件
```

然后在 `[ci].repo` 仓库 **Settings → Secrets and variables → Actions** 中添加 Secret `AUTH_PASSPHRASE`，值与本地 `[setting].auth_passphrase` **保持一致**。

## 4. 执行第一次复制

```bash
# 把源镜像复制到目标注册表（触发远程工作流，本地轮询等待）
docker mtrans sync ghcr.io/jetsung/shortener:latest

# 从目标注册表拉回并重命名为源镜像名
docker mtrans pull ghcr.io/jetsung/shortener:latest

# 或者一步到位：先复制再拉回
docker mtrans spull alpine:latest
```

??? tip "临时换目标，不改配置"

    `sync`/`pull`/`spull` 支持覆盖参数，单次执行生效，**绝不写回 `config.toml`**：

    ```bash
    docker mtrans sync alpine:latest -R new.example.com -o myorg -m 2
    ```

## 下一步

- 了解目标镜像名如何由源镜像派生 → [目标镜像组合规则](advanced/naming-rules.md)
- 查看全部子命令 → [命令总览](commands/index.md)
- 理解凭据加密与日志打掩码 → [凭据加密与打掩码](advanced/credentials.md)
