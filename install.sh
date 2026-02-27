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
# If empty or "latest", the script resolves the newest alpha tag automatically.
VERSION="${NGINX_MANAGER_VERSION:-latest}"
TMP_DIR="/tmp/nginx-manager-install"
BINARY_IN_TMP=""

# Optional versions
NGINX_VERSION="${NGINX_VERSION:-}"
CERTBOT_VERSION="${CERTBOT_VERSION:-}"

# Download proxy prefix (enabled by default for CN users)
# Example: https://down.avi.gs/
# Disable: DOWNLOAD_PROXY_PREFIX=""
DOWNLOAD_PROXY_PREFIX="${DOWNLOAD_PROXY_PREFIX:-https://down.avi.gs/}"

if [[ "${EUID}" -ne 0 ]]; then
  echo "Please run as root."
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

normalize_github_repo() {
  local repo="$1"
  repo="${repo#https://github.com/}"
  repo="${repo#http://github.com/}"
  repo="${repo%.git}"
  repo="${repo#/}"
  echo "${repo}"
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64) echo "x86_64" ;;
    aarch64|arm64) echo "aarch64" ;;
    *) echo "" ;;
  esac
}

apply_proxy_url() {
  local url="$1"
  if [[ -z "${DOWNLOAD_PROXY_PREFIX}" ]]; then
    echo "${url}"
  else
    echo "${DOWNLOAD_PROXY_PREFIX}${url}"
  fi
}

build_download_url() {
  local arch="$1"
  local asset_name="${APP_NAME}-linux-${arch}.tar.gz"
  echo "https://github.com/${GITHUB_REPO}/releases/download/${VERSION}/${asset_name}"
}

resolve_latest_alpha_version() {
  local api_url="https://api.github.com/repos/${GITHUB_REPO}/releases?per_page=30"
  local api_proxy_url
  api_proxy_url="$(apply_proxy_url "${api_url}")"
  local tmp_json
  tmp_json="$(mktemp)"

  if ! curl -fsSL -H "Accept: application/vnd.github+json" -H "User-Agent: nginx-manager-installer" "${api_proxy_url}" -o "${tmp_json}" 2>/dev/null; then
    curl -fsSL -H "Accept: application/vnd.github+json" -H "User-Agent: nginx-manager-installer" "${api_url}" -o "${tmp_json}" 2>/dev/null || {
      echo "Failed to query GitHub Releases API."
      rm -f "${tmp_json}"
      exit 1
    }
  fi

  local tag=""
  tag="$(python3 - "${tmp_json}" <<'PY'
import json
import sys
from pathlib import Path

p = Path(sys.argv[1])
try:
    data = json.loads(p.read_text(encoding="utf-8"))
except Exception:
    print("")
    raise SystemExit(0)

if not isinstance(data, list):
    print("")
    raise SystemExit(0)

for rel in data:
    if not isinstance(rel, dict):
        continue
    if rel.get("draft") is True:
        continue
    tag = str(rel.get("tag_name", "")).strip()
    if "alpha" in tag:
        print(tag)
        raise SystemExit(0)

print("")
PY
)"

  rm -f "${tmp_json}"
  if [[ -z "${tag}" ]]; then
    echo "No alpha release tag found in repository ${GITHUB_REPO}."
    echo "Please set NGINX_MANAGER_VERSION manually."
    exit 1
  fi
  VERSION="$(echo "${tag}" | tr -d '[:space:]')"
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
      curl -fsSL "$(apply_proxy_url "https://nginx.org/keys/nginx_signing.key")" \
        | gpg --dearmor -o /etc/apt/keyrings/nginx.gpg
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
    echo "nginx already exists, skipping installation."
    return
  fi

  setup_nginx_official_repo

  case "${PM}" in
    apt)
      if [[ -n "${NGINX_VERSION}" ]]; then
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
  if command -v certbot >/dev/null 2>&1 && [[ "${FORCE_CERTBOT_REINSTALL:-false}" != "true" ]]; then
    echo "certbot already exists, skipping installation."
    return
  fi

  python3 -m pip install --upgrade pip
  if [[ -n "${CERTBOT_VERSION}" ]]; then
    python3 -m pip install "certbot==${CERTBOT_VERSION}" "certbot-nginx==${CERTBOT_VERSION}"
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
    echo "Unsupported architecture: $(uname -m). Supported: x86_64 / aarch64."
    exit 1
  fi

  GITHUB_REPO="$(normalize_github_repo "${GITHUB_REPO}")"
  if [[ "${GITHUB_REPO}" != */* ]]; then
    echo "Invalid GitHub repository format. Use owner/repo or full URL."
    exit 1
  fi

  if [[ -z "${VERSION}" || "${VERSION}" == "latest" ]]; then
    resolve_latest_alpha_version
  fi

  local url
  url="$(build_download_url "${arch}")"
  local download_url
  download_url="$(apply_proxy_url "${url}")"
  local archive="${TMP_DIR}/${APP_NAME}.tar.gz"

  rm -rf "${TMP_DIR}"
  mkdir -p "${TMP_DIR}"

  echo "Downloading prebuilt binary: ${download_url}"
  if ! curl -fL "${download_url}" -o "${archive}"; then
    echo "Download failed."
    echo "Repo: ${GITHUB_REPO}"
    echo "Version: ${VERSION}"
    echo "Proxy prefix: ${DOWNLOAD_PROXY_PREFIX}"
    exit 1
  fi

  tar -xzf "${archive}" -C "${TMP_DIR}"
  BINARY_IN_TMP="$(find "${TMP_DIR}" -type f -name "${APP_NAME}" | head -n 1 || true)"
  if [[ -z "${BINARY_IN_TMP}" ]]; then
    echo "Binary ${APP_NAME} not found in archive."
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

PM="$(detect_pkg_manager)"
if [[ "${PM}" == "unknown" ]]; then
  echo "Unsupported package manager."
  exit 1
fi

echo "[1/12] Installing base dependencies..."
install_base_deps

echo "[2/12] Installing nginx from official repository..."
install_nginx_from_official_repo

echo "[3/12] Installing certbot via pip..."
install_certbot_by_pip

echo "[4/12] Downloading nginx-manager binary..."
download_binary

if ! command -v systemctl >/dev/null 2>&1; then
  echo "systemctl not found. systemd is required."
  exit 1
fi

echo "[5/12] Preparing directories..."
mkdir -p "${INSTALL_DIR}" "${CONFIG_DIR}" "${DATA_DIR}" "${LOG_DIR}" "/etc/nginx-manager/certs"
mkdir -p "/etc/nginx/sites-available" "/etc/nginx/sites-enabled"
mkdir -p "${INSTALL_DIR}/scripts"

echo "[6/12] Installing binaries..."
cp "${BINARY_IN_TMP}" "${BIN_PATH}"
chmod +x "${BIN_PATH}"
cp "./scripts/renew-certs.sh" "${INSTALL_DIR}/scripts/renew-certs.sh"
chmod +x "${INSTALL_DIR}/scripts/renew-certs.sh"

echo "[7/12] Writing environment config..."
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

echo "[8/12] Installing systemd units..."
cp "./deploy/nginx-manager.service" "${SYSTEMD_FILE}"
cp "./deploy/nginx-manager-cert-renew.service" "${RENEW_SERVICE_FILE}"
cp "./deploy/nginx-manager-cert-renew.timer" "${RENEW_TIMER_FILE}"
systemctl daemon-reload
systemctl enable nginx-manager
systemctl restart nginx-manager
systemctl enable nginx-manager-cert-renew.timer
systemctl restart nginx-manager-cert-renew.timer

echo "[9/12] Configuring firewall ports..."
configure_firewall

echo "[10/12] Verifying service..."
systemctl --no-pager --full status nginx-manager >/dev/null

echo "[11/12] Cleaning temp files..."
rm -rf "${TMP_DIR}"

echo "[12/12] Done."
echo "URL: http://<server-ip>:8080"
echo "Default login: admin / admin123!"
echo "Please change password after first login."
echo "GitHub repo: ${GITHUB_REPO}"
echo "Version: ${VERSION}"
echo "Download proxy: ${DOWNLOAD_PROXY_PREFIX}"
echo "Optional: NGINX_VERSION=1.26.3 CERTBOT_VERSION=2.11.0 sudo ./install.sh"
