#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   DOMAIN=swarm.example.com \
#   EMAIL=ops@example.com \
#   SHARD_USER=shard \
#   SHARD_HOME=/opt/shard \
#   SHARD_TCP_PORT=4001 \
#   SHARD_WS_PORT=4101 \
#   SHARD_WEBRTC_PORT=9090 \
#   SHARD_QUIC_PORT=9092 \
#   SHARD_CONTROL_PORT=9091 \
#   SHARD_TELEMETRY_PORT=9093 \
#   ./scripts/ec2-production-bootstrap.sh

DOMAIN="${DOMAIN:-}"
EMAIL="${EMAIL:-}"
SHARD_USER="${SHARD_USER:-shard}"
SHARD_HOME="${SHARD_HOME:-/opt/shard}"
SHARD_TCP_PORT="${SHARD_TCP_PORT:-4001}"
SHARD_WS_PORT="${SHARD_WS_PORT:-}"
SHARD_WEBRTC_PORT="${SHARD_WEBRTC_PORT:-9090}"
SHARD_QUIC_PORT="${SHARD_QUIC_PORT:-9092}"
SHARD_CONTROL_PORT="${SHARD_CONTROL_PORT:-9091}"
SHARD_TELEMETRY_PORT="${SHARD_TELEMETRY_PORT:-9093}"

if [[ -z "$SHARD_WS_PORT" ]]; then
  SHARD_WS_PORT="$((SHARD_TCP_PORT + 100))"
fi

if [[ -z "$DOMAIN" || -z "$EMAIL" ]]; then
  echo "ERROR: DOMAIN and EMAIL must be set." >&2
  exit 1
fi

if [[ $EUID -ne 0 ]]; then
  echo "Run as root (sudo -i)." >&2
  exit 1
fi

apt-get update
apt-get install -y nginx certbot python3-certbot-nginx ufw

id -u "$SHARD_USER" >/dev/null 2>&1 || useradd -r -m -d "$SHARD_HOME" -s /usr/sbin/nologin "$SHARD_USER"
mkdir -p "$SHARD_HOME/bin" "$SHARD_HOME/config"
chown -R "$SHARD_USER:$SHARD_USER" "$SHARD_HOME"

# Firewall rules on instance (also mirror these in EC2 Security Group):
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow "$SHARD_TCP_PORT/tcp"
ufw allow "$SHARD_WS_PORT/tcp"
ufw allow "$SHARD_WEBRTC_PORT/udp"
ufw allow "$SHARD_QUIC_PORT/udp"
ufw --force enable

cp /workspace/Shard/deploy/nginx/shard-wss.conf /etc/nginx/sites-available/shard
sed -i "s/swarm.example.com/${DOMAIN}/g" /etc/nginx/sites-available/shard
sed -i "s/127.0.0.1:9093/127.0.0.1:${SHARD_TELEMETRY_PORT}/g" /etc/nginx/sites-available/shard
sed -i "s/127.0.0.1:9091/127.0.0.1:${SHARD_CONTROL_PORT}/g" /etc/nginx/sites-available/shard
ln -sf /etc/nginx/sites-available/shard /etc/nginx/sites-enabled/shard
rm -f /etc/nginx/sites-enabled/default
nginx -t
systemctl restart nginx

certbot --nginx -d "$DOMAIN" --non-interactive --agree-tos -m "$EMAIL" --redirect

cat >/etc/systemd/system/shard-daemon.service <<UNIT
[Unit]
Description=Shard Rust libp2p sidecar
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${SHARD_USER}
Group=${SHARD_USER}
WorkingDirectory=${SHARD_HOME}
Environment=RUST_LOG=info
Environment=PORT=${SHARD_TELEMETRY_PORT}
ExecStart=${SHARD_HOME}/bin/shard-daemon \
  --control-port ${SHARD_CONTROL_PORT} \
  --tcp-port ${SHARD_TCP_PORT} \
  --webrtc-port ${SHARD_WEBRTC_PORT} \
  --quic-port ${SHARD_QUIC_PORT}
Restart=always
RestartSec=5
LimitNOFILE=65536
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
UNIT

systemctl daemon-reload
systemctl enable shard-daemon
systemctl restart shard-daemon

cat <<DONE
Bootstrap complete.

Next steps:
1) Copy your compiled shard-daemon binary to ${SHARD_HOME}/bin/shard-daemon
2) chown ${SHARD_USER}:${SHARD_USER} ${SHARD_HOME}/bin/shard-daemon && chmod +x ${SHARD_HOME}/bin/shard-daemon
3) systemctl restart shard-daemon
4) systemctl status shard-daemon --no-pager
5) journalctl -u shard-daemon -f

EC2 Security Group must allow:
- 80/tcp, 443/tcp
- ${SHARD_TCP_PORT}/tcp (libp2p tcp)
- ${SHARD_WS_PORT}/tcp (libp2p websocket)
- ${SHARD_WEBRTC_PORT}/udp (webrtc-direct)
- ${SHARD_QUIC_PORT}/udp (quic)
DONE
