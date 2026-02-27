#!/usr/bin/env bash
set -euo pipefail

log() {
  echo "[证书续期] $*"
}

if ! command -v certbot >/dev/null 2>&1; then
  log "未检测到 certbot，跳过。"
  exit 0
fi

if ! command -v systemctl >/dev/null 2>&1; then
  log "未检测到 systemctl，无法重载 nginx。"
  exit 1
fi

log "开始执行 certbot renew ..."
set +e
output="$(certbot renew --non-interactive 2>&1)"
code=$?
set -e

echo "${output}"
if [[ ${code} -ne 0 ]]; then
  log "续期命令失败，退出码：${code}"
  exit ${code}
fi

if echo "${output}" | grep -Eq "No renewals were attempted|No hooks were run"; then
  log "没有需要续期的证书。"
  exit 0
fi

log "检测到续期动作，重载 nginx ..."
systemctl reload nginx
log "续期任务完成。"
