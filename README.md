# nginx-manager

基于 Rust 的 Linux Nginx 可视化管理工具。

## 一条命令安装（公共仓库）

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/install.sh)"
```

说明：
- 默认会自动解析并安装最新 `alpha` 版本（不再依赖 `latest`）
- 下载默认使用加速前缀：`https://down.avi.gs/`

## 指定版本安装

```bash
NGINX_MANAGER_VERSION=v0.1.5-alpha \
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/install.sh)"
```

查看可用版本（Release Tag）：

```bash
curl -fsSL https://api.github.com/repos/justicerains1/nginx-manager/releases | grep tag_name
```

## 可选参数

- `NGINX_MANAGER_VERSION`：程序版本（如 `v0.1.5-alpha`）
- `NGINX_VERSION`：Nginx 版本（官方仓库安装）
- `CERTBOT_VERSION`：Certbot 版本（pip 安装）
- `NGINX_MANAGER_GITHUB_REPO`：程序仓库（默认 `justicerains1/nginx-manager`）
- `DOWNLOAD_PROXY_PREFIX`：下载加速前缀（默认 `https://down.avi.gs/`）
- `NGINX_MANAGER_INSTALL_REF`：安装附属文件来源分支/标签（默认 `master`）

关闭下载加速：

```bash
DOWNLOAD_PROXY_PREFIX="" \
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/install.sh)"
```

## 一条命令卸载

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/uninstall.sh)"
```

保留数据库数据：

```bash
KEEP_DATA=true \
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/uninstall.sh)"
```
