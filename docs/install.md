---
icon: lucide/hammer
---

# 构建与安装

## 安装

### cargo install（推荐）

```bash
# 从 crates.io 安装
cargo install mtrans

# 或从 Git 仓库安装
cargo install --git https://github.com/jetsung/mtrans.git
```

安装后可直接调用 `docker-mtrans`。

### 从源码构建

在仓库根目录执行：

```bash
cargo build --release
```

产物为单文件 CLI：`target/release/docker-mtrans`。

## 安装形态

### 独立运行

直接调用可执行文件即可：

```bash
target/release/docker-mtrans sync <源镜像>
```

也可以把它移到 `PATH` 中（如 `/usr/local/bin/`）后全局使用。

### 作为 docker CLI 插件

把可执行文件放入 docker CLI 插件目录，即可用 `docker mtrans ...` 调用：

| 平台 | 插件目录 |
| --- | --- |
| Linux / macOS | `~/.docker/cli-plugins/` |
| Windows | `%USERPROFILE%\.docker\cli-plugins\` |

```bash
# Linux / macOS 安装示例
mkdir -p ~/.docker/cli-plugins
cp target/release/docker-mtrans ~/.docker/cli-plugins/docker-mtrans
chmod +x ~/.docker/cli-plugins/docker-mtrans

# 验证插件被发现
docker mtrans --help
```

## 系统要求

- Windows / Linux / macOS 三端均可构建运行，平台差异详见[平台支持](advanced/platform.md)。
- 本机需可连接 Docker 守护进程（默认端点自动探测，也可用 `DOCKER_HOST` 指定）。
- 复制由 GitHub Actions 托管执行，本机**不承担**拉取/推送镜像层的复制工作。
