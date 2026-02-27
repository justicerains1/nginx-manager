# nginx-manager

基于 Rust 的 Linux Nginx 可视化管理工具。

## 一条命令安装（推荐）

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/install.sh)"
```

## 一条命令安装（可选版本）

```bash
NGINX_MANAGER_VERSION=v0.1.0-alpha NGINX_VERSION=1.26.3 CERTBOT_VERSION=2.11.0 \
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/install.sh)"
```

## 可选参数说明

- `NGINX_MANAGER_VERSION`：程序版本（GitHub Release Tag）
- `NGINX_VERSION`：Nginx 版本（走 nginx 官方仓库）
- `CERTBOT_VERSION`：Certbot 版本（走 pip 安装）
- `NGINX_MANAGER_GITHUB_REPO`：程序仓库地址（支持 `owner/repo` 或完整 URL）

## 卸载

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/uninstall.sh)"
```

保留数据库数据：

```bash
KEEP_DATA=true bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/uninstall.sh)"
```
