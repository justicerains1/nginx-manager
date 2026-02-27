#!/usr/bin/env bash
set -euo pipefail

APP_NAME="nginx-manager"
INSTALL_DIR="/opt/nginx-manager"
CONFIG_DIR="/etc/nginx-manager"
DATA_DIR="/var/lib/nginx-manager"
LOG_DIR="/var/log/nginx-manager"
SYSTEMD_FILE="/etc/systemd/system/nginx-manager.service"
RENEW_SERVICE_FILE="/etc/systemd/system/nginx-manager-cert-renew.service"
RENEW_TIMER_FILE="/etc/systemd/system/nginx-manager-cert-renew.timer"
BIN_PATH="${INSTALL_DIR}/nginx-manager"
GITHUB_REPO="${NGINX_MANAGER_GITHUB_REPO:-justicerains1/nginx-manager}"
VERSION="${NGINX_MANAGER_VERSION:-latest}"
TMP_DIR="/tmp/nginx-manager-install"
BINARY_IN_TMP=""

# 可选版本（不填则安装最新）
NGINX_VERSION="${NGINX_VERSION:-}"
CERTBOT_VERSION="${CERTBOT_VERSION:-}"

if [[ "${EUID}" -ne 0 ]]; then
  echo "请使用 root 权限运行此脚本。"
  exit 1
fi

detect_pkg_manager() {
  if command -v apt-get >/dev/null 2>&1; then
    echo "apt"
    return
  fi
  if command -v dnf >/dev/null 2>&1; then
    echo "dnf"
    return
  fi
  if command -v yum >/dev/null 2>&1; then
    echo "yum"
    return
  fi
  echo "unknown"
}

PM="$(detect_pkg_manager)"
if [[ "${PM}" == "unknown" ]]; then
  echo "不支持的包管理器。"
  exit 1
fi

normalize_github_repo() {
  local repo="$1"
  repo="${repo#https://github.com/}"
  repo="${repo#http://github.com/}"
  repo="${repo%.git}"
  repo="${repo#/}"
  echo "${repo}"
}

detect_arch() {
  local arch
  arch="$(uname -m)"
  case "${arch}" in
    x86_64|amd64) echo "x86_64" ;;
    aarch64|arm64) echo "aarch64" ;;
    *) echo "" ;;
  esac
}

build_download_url() {
  local arch="$1"
  local asset_name="${APP_NAME}-linux-${arch}.tar.gz"
  if [[ "${VERSION}" == "latest" ]]; then
    echo "https://github.com/${GITHUB_REPO}/releases/latest/download/${asset_name}"
  else
    echo "https://github.com/${GITHUB_REPO}/releases/download/${VERSION}/${asset_name}"
  fi
}

install_base_deps() {
  case "${PM}" in
    apt)
      apt-get update
      DEBIAN_FRONTEND=noninteractive apt-get install -y \
        curl ca-certificates tar openssl gnupg2 lsb-release python3 python3-pip
      ;;
    dnf)
      dnf install -y \
        curl ca-certificates tar openssl dnf-plugins-core python3 python3-pip
      ;;
    yum)
      yum install -y epel-release || true
      yum install -y \
        curl ca-certificates tar openssl yum-utils python3 python3-pip
      ;;
  esac
}

setup_nginx_official_repo() {
  case "${PM}" in
    apt)
      install -m 0755 -d /etc/apt/keyrings
      curl -fsSL https://nginx.org/keys/nginx_signing.key | gpg --dearmor -o /etc/apt/keyrings/nginx.gpg
      cat >/etc/apt/sources.list.d/nginx.list <<EOF
deb [signed-by=/etc/apt/keyrings/nginx.gpg] http://nginx.org/packages/mainline/ubuntu $(. /etc/os-release && echo "${VERSION_CODENAME}") nginx
EOF
      apt-get update
      ;;
    dnf|yum)
      cat >/etc/yum.repos.d/nginx-official.repo <<'EOF'
[nginx-stable]
name=nginx stable repo
baseurl=https://nginx.org/packages/centos/$releasever/$basearch/
gpgcheck=1
enabled=1
gpgkey=https://nginx.org/keys/nginx_signing.key
module_hotfixes=true
EOF
      if [[ "${PM}" == "dnf" ]]; then
        dnf makecache
      else
        yum makecache
      fi
      ;;
  esac
}

install_nginx_from_official_repo() {
  if command -v nginx >/dev/null 2>&1 && [[ "${FORCE_NGINX_REINSTALL:-false}" != "true" ]]; then
    echo "已检测到 nginx，跳过安装。"
    return
  fi

  setup_nginx_official_repo

  case "${PM}" in
    apt)
      if [[ -n "${NGINX_VERSION}" ]]; then
        # 例如 NGINX_VERSION=1.26.3
        apt-get install -y "nginx=${NGINX_VERSION}*"
      else
        apt-get install -y nginx
      fi
      ;;
    dnf)
      if [[ -n "${NGINX_VERSION}" ]]; then
        dnf install -y "nginx-${NGINX_VERSION}*"
      else
        dnf install -y nginx
      fi
      ;;
    yum)
      if [[ -n "${NGINX_VERSION}" ]]; then
        yum install -y "nginx-${NGINX_VERSION}*"
      else
        yum install -y nginx
      fi
      ;;
  esac
}

install_certbot_by_pip() {
  # 不使用系统 certbot 包，改用 pip 安装，支持版本控制
  if command -v certbot >/dev/null 2>&1 && [[ "${FORCE_CERTBOT_REINSTALL:-false}" != "true" ]]; then
    echo "已检测到 certbot，跳过安装。"
    return
  fi

  python3 -m pip install --upgrade pip
  if [[ -n "${CERTBOT_VERSION}" ]]; then
    python3 -m pip install \
      "certbot==${CERTBOT_VERSION}" \
      "certbot-nginx==${CERTBOT_VERSION}"
  else
    python3 -m pip install certbot certbot-nginx
  fi

  if ! command -v certbot >/dev/null 2>&1; then
    ln -sf /usr/local/bin/certbot /usr/bin/certbot || true
  fi
}

download_binary() {
  local arch
  arch="$(detect_arch)"
  if [[ -z "${arch}" ]]; then
    echo "不支持的 CPU 架构：$(uname -m)，当前仅支持 x86_64 / aarch64。"
    exit 1
  fi

  GITHUB_REPO="$(normalize_github_repo "${GITHUB_REPO}")"
  if [[ "${GITHUB_REPO}" != */* ]]; then
    echo "GitHub 仓库格式无效，请使用 owner/repo 或完整 URL。"
    exit 1
  fi

  local url
  url="$(build_download_url "${arch}")"
  local archive="${TMP_DIR}/${APP_NAME}.tar.gz"

  rm -rf "${TMP_DIR}"
  mkdir -p "${TMP_DIR}"

  echo "正在下载预编译程序：${url}"
  if ! curl -fL "${url}" -o "${archive}"; then
    echo "下载失败，请确认 GitHub 仓库或版本是否正确。"
    echo "当前仓库：${GITHUB_REPO}"
    echo "当前版本：${VERSION}"
    exit 1
  fi

  tar -xzf "${archive}" -C "${TMP_DIR}"
  BINARY_IN_TMP="$(find "${TMP_DIR}" -type f -name "${APP_NAME}" | head -n 1 || true)"
  if [[ -z "${BINARY_IN_TMP}" ]]; then
    echo "压缩包中未找到 ${APP_NAME} 可执行文件。"
    exit 1
  fi
}

configure_firewall() {
  if command -v ufw >/dev/null 2>&1; then
    ufw allow 80/tcp || true
    ufw allow 443/tcp || true
    ufw allow 8080/tcp || true
  fi

  if command -v firewall-cmd >/dev/null 2>&1; then
    if systemctl is-active --quiet firewalld; then
      firewall-cmd --permanent --add-service=http || true
      firewall-cmd --permanent --add-service=https || true
      firewall-cmd --permanent --add-port=8080/tcp || true
      firewall-cmd --reload || true
    fi
  fi
}

echo "[1/12] 安装基础依赖（不含 nginx/certbot）..."
install_base_deps

echo "[2/12] 通过 nginx 官方仓库安装 nginx（支持可选版本）..."
install_nginx_from_official_repo

echo "[3/12] 通过 pip 安装 certbot（支持可选版本）..."
install_certbot_by_pip

echo "[4/12] 下载预编译二进制..."
download_binary

if ! command -v systemctl >/dev/null 2>&1; then
  echo "未检测到 systemctl，本安装器需要 systemd 环境。"
  exit 1
fi

echo "[5/12] 创建目录..."
mkdir -p "${INSTALL_DIR}" "${CONFIG_DIR}" "${DATA_DIR}" "${LOG_DIR}" "/etc/nginx-manager/certs"
mkdir -p "/etc/nginx/sites-available" "/etc/nginx/sites-enabled"
mkdir -p "${INSTALL_DIR}/scripts"

echo "[6/12] 安装二进制文件..."
cp "${BINARY_IN_TMP}" "${BIN_PATH}"
chmod +x "${BIN_PATH}"
cp "./scripts/renew-certs.sh" "${INSTALL_DIR}/scripts/renew-certs.sh"
chmod +x "${INSTALL_DIR}/scripts/renew-certs.sh"

echo "[7/12] 写入环境配置..."
cat > "${CONFIG_DIR}/env" <<'EOF'
NGINX_MANAGER_BIND=0.0.0.0:8080
NGINX_MANAGER_DB=sqlite:///var/lib/nginx-manager/manager.db
NGINX_MANAGER_ADMIN_USER=admin
NGINX_MANAGER_ADMIN_PASS=admin123!
NGINX_SITES_AVAILABLE=/etc/nginx/sites-available
NGINX_SITES_ENABLED=/etc/nginx/sites-enabled
NGINX_MANAGER_CERT_DIR=/etc/nginx-manager/certs
NGINX_BIN=nginx
SYSTEMCTL_BIN=systemctl
CERTBOT_BIN=certbot
RUST_LOG=info
EOF

echo "[8/12] 安装 systemd 服务..."
cp "./deploy/nginx-manager.service" "${SYSTEMD_FILE}"
cp "./deploy/nginx-manager-cert-renew.service" "${RENEW_SERVICE_FILE}"
cp "./deploy/nginx-manager-cert-renew.timer" "${RENEW_TIMER_FILE}"
systemctl daemon-reload
systemctl enable nginx-manager
systemctl restart nginx-manager
systemctl enable nginx-manager-cert-renew.timer
systemctl restart nginx-manager-cert-renew.timer

echo "[9/12] 配置防火墙端口（80/443/8080）..."
configure_firewall

echo "[10/12] 校验服务状态..."
systemctl --no-pager --full status nginx-manager >/dev/null

echo "[11/12] 清理临时文件..."
rm -rf "${TMP_DIR}"

echo "[12/12] 安装完成"
echo "访问地址：http://<服务器IP>:8080"
echo "默认账号：admin / admin123!"
echo "请在首次登录后立即修改密码。"
echo "当前下载源：${GITHUB_REPO}"
echo "当前版本：${VERSION}"
echo "可选 nginx 版本：NGINX_VERSION=1.26.3 sudo ./install.sh"
echo "可选 certbot 版本：CERTBOT_VERSION=2.11.0 sudo ./install.sh"
