#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "==> Installing frontend dependencies..."
(cd "$ROOT_DIR/frontend" && npm install)

echo "==> Building frontend..."
(cd "$ROOT_DIR/frontend" && npm run build)

echo "==> Building Rust binary (release)..."
(cd "$ROOT_DIR" && cargo build --release)

echo ""
echo "Done. Binary: $ROOT_DIR/target/release/local-wallet"
