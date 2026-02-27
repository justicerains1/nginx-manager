# nginx-manager

基于 Rust 的 Linux Nginx 可视化管理工具。

## 当前 MVP 功能

- Web 登录与仪表盘
- 系统设置（修改管理员密码）
- 站点管理（新建/编辑/删除、反向代理/静态站点、启用/停用、绑定证书）
- Nginx 服务控制（`start/stop/restart/reload/status`）
- 证书管理
  - 上传已有证书与私钥
  - 通过 certbot 申请 Let's Encrypt 证书（HTTP-01）
- 证书到期提醒（仪表盘 + 证书列表元数据刷新）
- 操作审计日志页面
- 证书自动续期（systemd timer 每天执行）
- 安全应用流程（`nginx -t` 通过后才重载，失败自动回滚）

## 本地运行

```bash
cargo run
```

可用环境变量：

- `NGINX_MANAGER_BIND`（默认：`0.0.0.0:8080`）
- `NGINX_MANAGER_DB`（默认：`sqlite:///var/lib/nginx-manager/manager.db`）
- `NGINX_MANAGER_ADMIN_USER`（默认：`admin`）
- `NGINX_MANAGER_ADMIN_PASS`（默认：`admin123!`）
- `NGINX_SITES_AVAILABLE`（默认：`/etc/nginx/sites-available`）
- `NGINX_SITES_ENABLED`（默认：`/etc/nginx/sites-enabled`）
- `NGINX_MANAGER_CERT_DIR`（默认：`/etc/nginx-manager/certs`）

## Linux 一键安装

```bash
chmod +x install.sh
sudo ./install.sh
```

安装脚本会执行以下动作：

- 自动识别包管理器（`apt` / `dnf` / `yum`）并安装依赖（`nginx`、`certbot`、`openssl`、`tar`）
- 从 GitHub Release 下载预编译二进制（默认仓库：`justicerains1/nginx-manager`）
- 若存在 `ufw` 或 `firewalld`，自动放行 `80`、`443`、`8080` 端口
- 以 `systemd` 方式安装并启动服务
- 自动安装并启用证书续期定时任务（`nginx-manager-cert-renew.timer`）

可选安装参数：

- 指定版本：`NGINX_MANAGER_VERSION=v0.1.0 sudo ./install.sh`
- 指定 GitHub 仓库：`NGINX_MANAGER_GITHUB_REPO=组织名/仓库名 sudo ./install.sh`
- 也支持完整仓库地址：`NGINX_MANAGER_GITHUB_REPO=https://github.com/justicerains1/nginx-manager.git sudo ./install.sh`

自动发布说明（GitHub Actions）：

- 手动触发工作流时，输入基础版本号（如 `v0.1.1`），系统会自动生成并发布 `v0.1.1-alpha.<运行号>`
- 若你输入已包含 `alpha` 的版本号，则按你输入的版本号发布

Release 产物命名要求：

- `nginx-manager-linux-x86_64.tar.gz`
- `nginx-manager-linux-aarch64.tar.gz`
- 压缩包内需包含可执行文件：`nginx-manager`

默认发布地址：

- `https://github.com/justicerains1/nginx-manager/releases`

安装后关键路径：

- 二进制：`/opt/nginx-manager/nginx-manager`
- 环境配置：`/etc/nginx-manager/env`
- 数据库：`/var/lib/nginx-manager/manager.db`
- 服务文件：`/etc/systemd/system/nginx-manager.service`
- 续期服务：`/etc/systemd/system/nginx-manager-cert-renew.service`
- 续期定时器：`/etc/systemd/system/nginx-manager-cert-renew.timer`
- 续期脚本：`/opt/nginx-manager/scripts/renew-certs.sh`
