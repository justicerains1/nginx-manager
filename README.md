# nginx-manager

基于 Rust 的 Linux Nginx 可视化管理工具。

## 私有仓库一条命令安装（推荐）

先准备一个有 `repo` 读取权限的 GitHub Token：

```bash
export GITHUB_TOKEN=你的Token
```

安装命令：

```bash
bash -c "$(curl -fsSL \
  -H "Authorization: Bearer ${GITHUB_TOKEN}" \
  -H "Accept: application/vnd.github.raw" \
  "https://api.github.com/repos/justicerains1/nginx-manager/contents/install.sh?ref=master")"
```

## 可选版本参数（私有仓库）

```bash
NGINX_MANAGER_VERSION=v0.1.0-alpha NGINX_VERSION=1.26.3 CERTBOT_VERSION=2.11.0 \
bash -c "$(curl -fsSL \
  -H "Authorization: Bearer ${GITHUB_TOKEN}" \
  -H "Accept: application/vnd.github.raw" \
  "https://api.github.com/repos/justicerains1/nginx-manager/contents/install.sh?ref=master")"
```

## 公开仓库安装（备用）

若仓库改为公开，可直接使用：

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/justicerains1/nginx-manager/master/install.sh)"
```

## 卸载（私有仓库）

```bash
bash -c "$(curl -fsSL \
  -H "Authorization: Bearer ${GITHUB_TOKEN}" \
  -H "Accept: application/vnd.github.raw" \
  "https://api.github.com/repos/justicerains1/nginx-manager/contents/uninstall.sh?ref=master")"
```

保留数据库数据：

```bash
KEEP_DATA=true bash -c "$(curl -fsSL \
  -H "Authorization: Bearer ${GITHUB_TOKEN}" \
  -H "Accept: application/vnd.github.raw" \
  "https://api.github.com/repos/justicerains1/nginx-manager/contents/uninstall.sh?ref=master")"
```

## CI 触发规则

- 自动触发（push）仅在以下文件变更时触发：
  - `src/**`
  - `Cargo.toml` / `Cargo.lock`
  - `install.sh` / `uninstall.sh`
  - `deploy/**` / `scripts/**`
  - `.github/workflows/build-and-release.yml`
- 仅改 `README` 等文档，不会触发发布流水线
