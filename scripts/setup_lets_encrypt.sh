#!/usr/bin/env bash
set -euo pipefail

# Config Variables
source ../.env
DOCKER_CONTAINER_NAME="pda-proxy"

echo "👉 Detecting OS and installing Certbot..."

# Detect and install Certbot
if [ -f /etc/os-release ]; then
  source /etc/os-release
  case "$ID" in
    ubuntu|debian)
      sudo apt update
      sudo apt install -y certbot
      ;;
    amzn)
      sudo yum install -y epel-release
      sudo yum install -y certbot
      ;;
    al2023)
      sudo dnf install -y certbot
      ;;
    *)
      echo "❌ Unsupported OS: $ID"
      exit 1
      ;;
  esac
else
  echo "❌ Cannot detect OS"
  exit 1
fi

echo "✅ Certbot installed."

# # Stop service temporarily to free port 80
# echo "⛔ Stopping Docker container: $DOCKER_CONTAINER_NAME (if running)"
# docker stop "$DOCKER_CONTAINER_NAME" || true

echo "📡 Requesting certificate for $TLS_DOMAIN"
sudo certbot certonly --standalone -d "$TLS_DOMAIN" --agree-tos --non-interactive --email "$EMAIL"

CERT_PATH="/etc/letsencrypt/live/$TLS_DOMAIN"
echo "🔐 Certs stored at $CERT_PATH"

# echo "🚀 Starting Docker container again..."
# docker start "$DOCKER_CONTAINER_NAME"

# Set up automatic renewal with a post-hook
RENEW_CMD="certbot renew --quiet --deploy-hook 'docker restart $DOCKER_CONTAINER_NAME'"

# NOTE: Assumes systemd
if command -v systemctl &>/dev/null; then
  echo "🕒 Creating systemd timer for auto-renew..."
  sudo bash -c "cat > /etc/systemd/system/certbot-renew.service" <<EOF
[Unit]
Description=Certbot Renew

[Service]
Type=oneshot
ExecStart=/usr/bin/env bash -c '$RENEW_CMD'
EOF

  sudo bash -c "cat > /etc/systemd/system/certbot-renew.timer" <<EOF
[Unit]
Description=Daily renewal of Let's Encrypt certificates, random time

[Timer]
OnBootSec=10min
OnUnitActiveSec=1d
RandomizedDelaySec=12h
AccuracySec=30s

[Install]
WantedBy=timers.target
EOF

  sudo systemctl daemon-reexec
  sudo systemctl daemon-reload
  sudo systemctl enable --now certbot-renew.timer
  echo "✅ Systemd timer enabled. Certificates will auto-renew  at a random time 12h."

else
  echo "🕒 Falling back to cron job for auto-renew..."
  (sudo crontab -l 2>/dev/null; echo "0 */12 * * * $RENEW_CMD") | sudo crontab -
  echo "✅ Cron job added to renew certificates every 12h."
fi

echo "🎉 Done! Your certificate is ready and auto-renewal is configured."

