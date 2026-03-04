#!/usr/bin/env bash
set -euo pipefail

BINARY_PATH="./local-wallet"
INSTALL_PATH="/usr/local/bin/local-wallet"
PLIST_PATH="$HOME/Library/LaunchAgents/com.local-wallet.plist"
LOG_DIR="$HOME/.local-wallet"

if [ ! -f "$BINARY_PATH" ]; then
  echo "Error: $BINARY_PATH not found. Build first with: cargo build --release"
  exit 1
fi

echo "Installing local-wallet to $INSTALL_PATH..."
cp "$BINARY_PATH" "$INSTALL_PATH"
chmod +x "$INSTALL_PATH"

mkdir -p "$LOG_DIR"
mkdir -p "$HOME/Library/LaunchAgents"

echo "Creating launchd plist..."
cat > "$PLIST_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.local-wallet</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_PATH</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$LOG_DIR/local-wallet.log</string>
    <key>StandardErrorPath</key>
    <string>$LOG_DIR/local-wallet.log</string>
</dict>
</plist>
EOF

launchctl load -w "$PLIST_PATH"

echo ""
echo "Done. local-wallet is running."
echo "  Admin UI: http://localhost:9292"
echo "  REST API: http://localhost:9293 (localhost only)"
echo "  Logs:     $LOG_DIR/local-wallet.log"
