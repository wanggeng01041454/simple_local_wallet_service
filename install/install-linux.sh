#!/usr/bin/env bash
set -euo pipefail

BINARY_PATH="./local-wallet"
INSTALL_PATH="/usr/local/bin/local-wallet"
SERVICE_FILE="/etc/systemd/system/local-wallet.service"

if [ ! -f "$BINARY_PATH" ]; then
  echo "Error: $BINARY_PATH not found. Build first with: cargo build --release"
  exit 1
fi

echo "Installing local-wallet to $INSTALL_PATH..."
sudo cp "$BINARY_PATH" "$INSTALL_PATH"
sudo chmod +x "$INSTALL_PATH"

echo "Creating systemd service..."
sudo tee "$SERVICE_FILE" > /dev/null <<EOF
[Unit]
Description=Local Wallet Service
After=network.target

[Service]
Type=simple
ExecStart=$INSTALL_PATH
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable local-wallet
sudo systemctl start local-wallet

echo ""
echo "Done. local-wallet is running."
echo "  Admin UI: http://localhost:9292"
echo "  REST API: http://localhost:9293 (localhost only)"
