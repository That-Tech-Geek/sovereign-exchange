#!/usr/bin/env bash
set -euo pipefail

INSTALL_ROOT="${INSTALL_ROOT:-/opt/sovereign-exchange}"
STATE_ROOT="${STATE_ROOT:-/var/lib/sovereign-exchange}"
BINARY="${BINARY:-}"
SERVICE_USER="${SERVICE_USER:-exchange}"

if [[ "${EUID}" -ne 0 ]]; then
  echo "run as root" >&2
  exit 1
fi

if [[ -z "${BINARY}" ]]; then
  if [[ -x "./exchange_core/target/release/exchange_core" ]]; then
    BINARY="./exchange_core/target/release/exchange_core"
  elif [[ -x "./target/release/exchange_core" ]]; then
    BINARY="./target/release/exchange_core"
  else
    echo "No release binary found. Build first with:" >&2
    echo "  cd exchange_core && cargo build --release" >&2
    exit 1
  fi
fi

id -u "${SERVICE_USER}" >/dev/null 2>&1 || useradd --system --home-dir /var/lib/sovereign-exchange --shell /usr/sbin/nologin "${SERVICE_USER}"

install -d -o "${SERVICE_USER}" -g "${SERVICE_USER}" "${STATE_ROOT}" "${STATE_ROOT}/snapshots"
install -d -o root -g root "${INSTALL_ROOT}/bin"
install -m 0755 "${BINARY}" "${INSTALL_ROOT}/bin/exchange_core"

install -d -o root -g root /etc/sovereign-exchange
cat >/etc/sovereign-exchange/exchange.env <<EOF
SE_JOURNAL_PATH=${STATE_ROOT}/commands.journal
SE_SNAPSHOT_DIR=${STATE_ROOT}/snapshots
EOF
chown root:root /etc/sovereign-exchange/exchange.env
chmod 0644 /etc/sovereign-exchange/exchange.env

install -m 0644 deploy/systemd/sovereign-exchange.service /etc/systemd/system/sovereign-exchange.service
systemctl daemon-reload
systemctl enable sovereign-exchange.service
systemctl restart sovereign-exchange.service

echo "installed: ${INSTALL_ROOT}/bin/exchange_core"
echo "state:     ${STATE_ROOT}"
echo "status:    systemctl status sovereign-exchange"
echo "logs:      journalctl -u sovereign-exchange -f"
