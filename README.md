# nginx-manager

基于 Rust 的 Linux Nginx 可视化管理工具。

## 关键说明

- `install.sh` 不再依赖系统默认仓库安装 `nginx/certbot`
- `nginx` 改为官方仓库安装，可指定版本
- `certbot` 改为 `pip` 安装，可指定版本
- 程序本体仍从 GitHub Release 下载预编译二进制

## Linux 一键安装

```bash
chmod +x install.sh
sudo ./install.sh
```

### 可选版本参数

- 指定 nginx 版本：
```bash
NGINX_VERSION=1.26.3 sudo ./install.sh
```

- 指定 certbot 版本：
```bash
CERTBOT_VERSION=2.11.0 sudo ./install.sh
```

- 指定程序版本（GitHub Release Tag）：
```bash
NGINX_MANAGER_VERSION=v0.1.0-alpha sudo ./install.sh
```

- 指定 GitHub 仓库：
```bash
NGINX_MANAGER_GITHUB_REPO=https://github.com/justicerains1/nginx-manager.git sudo ./install.sh
```

## 卸载

```bash
chmod +x uninstall.sh
sudo ./uninstall.sh
```

- 保留数据库数据：
```bash
KEEP_DATA=true sudo ./uninstall.sh
```
