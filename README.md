# nginx-manager

基于 Rust 的 Linux Nginx 可视化管理工具。

## 当前功能

- Web 登录与仪表盘
- 系统设置（修改管理员密码）
- 站点管理（新建/编辑/删除、反向代理/静态站点、启用/停用、绑定证书）
- Nginx 服务控制（`start/stop/restart/reload/status`）
- 证书管理（上传已有证书、申请 Let's Encrypt）
- 证书到期提醒（7/15/30 天分级）
- 操作审计日志
- 证书自动续期（systemd timer）
- 安全应用流程（`nginx -t` 通过后才重载，失败自动回滚）

## 本地运行

```bash
cargo run
```

## Linux 一键安装

```bash
chmod +x install.sh
sudo ./install.sh
```

安装脚本会执行：

- 自动识别包管理器（`apt` / `dnf` / `yum`）并安装依赖
- 从 GitHub Release 下载预编译二进制（默认仓库：`justicerains1/nginx-manager`）
- 自动安装 systemd 服务与证书续期定时器
- 自动放行 `80/443/8080` 端口（若检测到 `ufw` 或 `firewalld`）

可选安装参数：

- 指定版本：`NGINX_MANAGER_VERSION=v0.1.0 sudo ./install.sh`
- 指定仓库：`NGINX_MANAGER_GITHUB_REPO=组织名/仓库名 sudo ./install.sh`
- 指定完整地址：`NGINX_MANAGER_GITHUB_REPO=https://github.com/justicerains1/nginx-manager.git sudo ./install.sh`

## 自动发布规则（GitHub Actions）

- 每次推送 `master/main`，自动发布下一个 `v0.1.x-alpha`
- `x` 基于已有标签自动递增
- 也支持手动触发工作流，仍按同样规则自动递增

Release 产物命名要求：

- `nginx-manager-linux-x86_64.tar.gz`
- `nginx-manager-linux-aarch64.tar.gz`
- 压缩包内必须包含可执行文件：`nginx-manager`

默认发布地址：

- `https://github.com/justicerains1/nginx-manager/releases`

## 卸载

```bash
chmod +x uninstall.sh
sudo ./uninstall.sh
```

可选参数：

- 保留数据库数据：`KEEP_DATA=true sudo ./uninstall.sh`

## 安装后关键路径

- 程序：`/opt/nginx-manager/nginx-manager`
- 环境配置：`/etc/nginx-manager/env`
- 数据库：`/var/lib/nginx-manager/manager.db`
- 主服务：`/etc/systemd/system/nginx-manager.service`
- 续期服务：`/etc/systemd/system/nginx-manager-cert-renew.service`
- 续期定时器：`/etc/systemd/system/nginx-manager-cert-renew.timer`
- 续期脚本：`/opt/nginx-manager/scripts/renew-certs.sh`
