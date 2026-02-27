# nginx-manager

基于 Rust 的 Linux Nginx 可视化管理工具。

## 一条命令安装（公共仓库）

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/install.sh)"
```

## 一条命令安装（可选版本）

```bash
NGINX_MANAGER_VERSION=v0.1.0-alpha NGINX_VERSION=1.26.3 CERTBOT_VERSION=2.11.0 \
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/install.sh)"
```

参数说明：

- `NGINX_MANAGER_VERSION`：程序版本（GitHub Release Tag）
- `NGINX_VERSION`：Nginx 版本（官方仓库安装）
- `CERTBOT_VERSION`：Certbot 版本（pip 安装）
- `NGINX_MANAGER_GITHUB_REPO`：程序仓库（默认 `justicerains1/nginx-manager`）
- `DOWNLOAD_PROXY_PREFIX`：下载加速前缀（默认 `https://down.avi.gs/`）

默认下载行为：

- 安装脚本下载构建产物时会自动走：
  - `https://down.avi.gs/https://github.com/...`
- 如需关闭代理加速：
```bash
DOWNLOAD_PROXY_PREFIX="" bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/install.sh)"
```

## 一条命令卸载

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/uninstall.sh)"
```

保留数据库数据：

```bash
KEEP_DATA=true bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/uninstall.sh)"
```

## CI 触发规则

- push 自动触发仅在以下文件变更时：
  - `src/**`
  - `Cargo.toml` / `Cargo.lock`
  - `install.sh` / `uninstall.sh`
  - `deploy/**` / `scripts/**`
  - `.github/workflows/build-and-release.yml`
- 仅文档（如 README）变更不会触发发布流水线。
