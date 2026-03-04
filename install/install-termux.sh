#!/usr/bin/env bash
set -euo pipefail

BINARY_PATH="./local-wallet"
INSTALL_PATH="$PREFIX/bin/local-wallet"

if [ ! -f "$BINARY_PATH" ]; then
  echo "Error: $BINARY_PATH not found."
  echo "Cross-compile with: cargo build --release --target aarch64-linux-android"
  exit 1
fi

echo "Installing local-wallet to $INSTALL_PATH..."
cp "$BINARY_PATH" "$INSTALL_PATH"
chmod +x "$INSTALL_PATH"

# Optionally add convenience alias to .bashrc
if ! grep -q "local-wallet" "$HOME/.bashrc" 2>/dev/null; then
  cat >> "$HOME/.bashrc" <<'EOF'

# local-wallet: start in background with tmux
alias wallet-start='tmux new-session -d -s wallet "local-wallet" 2>/dev/null || echo "wallet already running"'
alias wallet-stop='tmux kill-session -t wallet 2>/dev/null || echo "wallet not running"'
EOF
fi

echo ""
echo "Done. To start local-wallet:"
echo "  local-wallet          # foreground"
echo "  wallet-start          # background via tmux (after reloading shell)"
echo ""
echo "  Admin UI: http://localhost:9292"
echo "  REST API: http://localhost:9293 (localhost only)"
echo ""
echo "Note: Termux has no persistent service manager."
echo "      Use tmux or run manually each session."
