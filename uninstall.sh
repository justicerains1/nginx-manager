#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "请使用 root 权限运行此脚本。"
  exit 1
fi

APP_NAME="nginx-manager"
INSTALL_DIR="/opt/nginx-manager"
CONFIG_DIR="/etc/nginx-manager"
DATA_DIR="/var/lib/nginx-manager"
LOG_DIR="/var/log/nginx-manager"

SERVICE_FILE="/etc/systemd/system/nginx-manager.service"
RENEW_SERVICE_FILE="/etc/systemd/system/nginx-manager-cert-renew.service"
RENEW_TIMER_FILE="/etc/systemd/system/nginx-manager-cert-renew.timer"

KEEP_DATA="${KEEP_DATA:-false}"

echo "[1/6] 停止并禁用服务..."
systemctl stop nginx-manager 2>/dev/null || true
systemctl disable nginx-manager 2>/dev/null || true
systemctl stop nginx-manager-cert-renew.timer 2>/dev/null || true
systemctl disable nginx-manager-cert-renew.timer 2>/dev/null || true

echo "[2/6] 删除 systemd 单元..."
rm -f "${SERVICE_FILE}" "${RENEW_SERVICE_FILE}" "${RENEW_TIMER_FILE}"
systemctl daemon-reload

echo "[3/6] 删除程序目录..."
rm -rf "${INSTALL_DIR}"

echo "[4/6] 删除配置目录..."
rm -rf "${CONFIG_DIR}"

echo "[5/6] 删除日志目录..."
rm -rf "${LOG_DIR}"

if [[ "${KEEP_DATA}" == "true" ]]; then
  echo "[6/6] 保留数据目录：${DATA_DIR}"
else
  echo "[6/6] 删除数据目录..."
  rm -rf "${DATA_DIR}"
fi

echo "卸载完成。"
if [[ "${KEEP_DATA}" == "true" ]]; then
  echo "你选择了保留数据，如需彻底删除请手动执行：rm -rf ${DATA_DIR}"
fi
