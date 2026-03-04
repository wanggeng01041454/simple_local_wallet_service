# Local Wallet Service — Design Doc

Date: 2026-03-04

## 1. Overview

A locally-installed wallet service that manages private keys for Solana and EVM-compatible networks (Ethereum, BNB Chain, Arbitrum). Exposes a web-based admin UI on port 9292 and a REST API on port 9293. One wallet per network. Private keys are encrypted at rest and decrypted into memory after the user unlocks the service via the admin UI.

## 2. Architecture

Single Rust binary embedding a React frontend.

```
local-wallet (single binary)
├── Admin Server — 0.0.0.0:9292 (axum)
│     └── Serves embedded React static files
│         + Admin API routes (/api/admin/*)
├── API Server   — 127.0.0.1:9293 (axum)
│     └── /api/wallet/address
│         /api/wallet/balance
│         /api/wallet/sign
└── WalletManager (Arc<RwLock<State>>)
      ├── Locked | Unlocked { keys }
      └── NotificationService (Telegram)
```

**Data directory:**
```
~/.local-wallet/              (Linux/macOS/Termux)
%APPDATA%\local-wallet\       (Windows)
├── config.json               # RPC URLs, app settings (plaintext)
├── secret.enc                # Telegram Token + Chat ID (AES-256-GCM encrypted)
└── wallets/
    ├── solana.enc
    ├── eth.enc
    ├── bnb.enc
    └── arb.enc
```

## 3. Tech Stack

| Layer | Choice |
|-------|--------|
| Language (backend) | Rust (stable) |
| HTTP framework | axum + tokio |
| Solana | solana-sdk + solana-client |
| EVM | alloy |
| Encryption | aes-gcm + pbkdf2 + rand |
| Serialization | serde + serde_json |
| Static file embed | include_dir |
| HTTP client | reqwest (Telegram) |
| Logging | tracing + tracing-subscriber |
| Language (frontend) | TypeScript |
| UI framework | React |
| Build tool | Vite |
| State management | React Context + useState |
| Testing (frontend) | Vitest |

## 4. Project Structure

```
local-wallet/
├── Cargo.toml                # workspace
├── crates/
│   ├── wallet-core/          # encryption, signing, network clients
│   │   ├── src/
│   │   │   ├── crypto.rs     # AES-256-GCM + PBKDF2
│   │   │   ├── wallet.rs     # WalletManager, lock/unlock
│   │   │   ├── solana.rs     # Solana key ops
│   │   │   ├── evm.rs        # EVM key ops (alloy)
│   │   │   └── notification.rs  # Telegram bot client
│   │   └── Cargo.toml
│   ├── wallet-server/        # axum HTTP servers
│   │   ├── src/
│   │   │   ├── admin/        # admin UI routes
│   │   │   ├── api/          # REST API routes
│   │   │   └── state.rs      # shared AppState
│   │   └── Cargo.toml
│   └── wallet-cli/           # binary entry point
│       ├── src/main.rs
│       └── Cargo.toml
├── frontend/
│   ├── src/
│   │   ├── pages/
│   │   │   ├── Setup.tsx     # /setup wizard
│   │   │   ├── Unlock.tsx    # /unlock
│   │   │   ├── Dashboard.tsx
│   │   │   ├── Wallets.tsx
│   │   │   └── Settings.tsx
│   │   ├── context/
│   │   │   └── AppContext.tsx
│   │   └── main.tsx
│   ├── package.json
│   └── vite.config.ts
└── install/
    ├── install-linux.sh
    ├── install-macos.sh
    ├── install-windows.ps1
    ├── install-termux.sh
    └── README.md
```

## 5. Encryption Scheme

**Algorithm:** AES-256-GCM with PBKDF2-SHA256 key derivation.

**Encrypted file format (JSON):**
```json
{
  "version": 1,
  "kdf": "pbkdf2-sha256",
  "iterations": 600000,
  "salt": "<base64 32 bytes>",
  "iv": "<base64 12 bytes>",
  "ciphertext": "<base64>"
}
```

**Password policy:**
- Single password unlocks all network wallets
- Same password encrypts `secret.enc` (Telegram config)
- Password is never stored; derived key lives in memory only while unlocked

## 6. Wallet Lifecycle

### First-time setup (no wallet files detected)
```
GET / → redirect to /setup
/setup Step 1: Set password (confirm twice)
/setup Step 2: For each network — Import private key OR Generate new key
→ Keys encrypted and saved to wallets/*.enc
→ Redirect to /unlock
```

### Normal startup (wallet files exist)
```
GET / → redirect to /unlock
/unlock: User enters password
→ PBKDF2 derive key → AES-256-GCM decrypt each wallets/*.enc
→ Success: WalletManager.state = Unlocked { keys }
         Redirect to /dashboard
→ Failure: Show error, allow retry
```

### Service shutdown
```
WalletManager keys are dropped from memory (Rust ownership)
```

## 7. REST API (port 9293, 127.0.0.1 only)

### GET /api/wallet/address
```
Query: ?network=solana|eth|bnb|arb
Response 200: { "network": "eth", "address": "0x..." }
Response 503: { "error": "wallet_locked", "message": "..." }
  + Telegram notification: "⚠️ Wallet locked — address request rejected"
```

### GET /api/wallet/balance
```
Query: ?network=solana|eth|bnb|arb
Response 200: { "network": "eth", "address": "0x...", "balance": "1.234", "unit": "ETH" }
Response 503: { "error": "wallet_locked", "message": "..." }
  + Telegram notification
```

### POST /api/wallet/sign
```
Body:
{
  "network": "eth",
  "transaction": "<hex-encoded serialized tx>",
  "metadata": {           // optional, for Telegram display
    "to": "0x...",
    "value": "0.1 ETH",
    "description": "transfer to Alice"
  }
}

Response 200: { "network": "eth", "signature": "<hex>", "signed_at": "<ISO8601>" }
Response 503: { "error": "wallet_locked", "message": "..." }
  + Telegram notification: "⚠️ Wallet locked — sign request rejected"
```

**On successful sign:**
Telegram notification:
```
🔏 Transaction Signed
Network: Ethereum
To: 0xAbc...
Value: 0.1 ETH
Description: transfer to Alice
Time: 2026-03-04 10:30:00 UTC
```

## 8. Admin UI Pages

| Route | Description |
|-------|-------------|
| `/setup` | First-time wizard: set password → import/create wallets |
| `/unlock` | Password entry to unlock wallet after restart |
| `/dashboard` | Home after unlock |
| `/wallets` | View addresses, balances, import/create keys, transfer |
| `/settings` | Telegram Bot config (Token + Chat ID), RPC URLs per network |

## 9. Telegram Notifications

**Configuration:** Stored in `secret.enc` (encrypted with wallet password). Set via `/settings` in admin UI.

**Trigger events:**
1. Transaction signed successfully
2. API request received while wallet is locked (address/balance/sign)

**Failure handling:** If Telegram send fails, log the error and continue. Never block the main flow.

**Default RPC endpoints (overridable via /settings):**
```
Solana:   https://api.mainnet-beta.solana.com
Ethereum: https://eth.llamarpc.com
BNB:      https://bsc-dataseed.binance.org
Arbitrum: https://arb1.arbitrum.io/rpc
```

## 10. Installation

### Linux (systemd)
```bash
install-linux.sh
# Copies binary to /usr/local/bin/local-wallet
# Creates /etc/systemd/system/local-wallet.service
# systemctl enable --now local-wallet
```

### macOS (launchd)
```bash
install-macos.sh
# Copies binary to /usr/local/bin/local-wallet
# Creates ~/Library/LaunchAgents/com.local-wallet.plist
# launchctl load -w ~/Library/LaunchAgents/com.local-wallet.plist
```

### Windows (Task Scheduler)
```powershell
install-windows.ps1
# Copies binary to %LOCALAPPDATA%\local-wallet\
# Creates Task Scheduler task: runs at logon, restarts on failure
# No admin privileges required
```

### Android Termux
```bash
install-termux.sh
# Copies binary to $PREFIX/bin/local-wallet
# Adds local-wallet to ~/.bashrc for manual start
# Note: runs in foreground; use tmux/screen for persistence
```

## 11. Testing Strategy

**wallet-core (unit tests):**
- Encrypt/decrypt roundtrip with known test vectors
- Wrong password returns error
- Solana signing: sign known message, verify signature
- EVM signing: sign known transaction, verify signature

**wallet-server (integration tests):**
- `axum::test` for all API endpoints
- Mock `WalletManager` for locked/unlocked states
- Assert 503 + Telegram mock called when locked
- Assert sign response and Telegram mock called on success

**Frontend (Vitest):**
- Setup wizard step transitions
- Unlock form validation
- Settings form save/load

## 12. Key Risks

| Risk | Mitigation |
|------|------------|
| solana-sdk / alloy version conflicts | Pin versions in Cargo.lock, test on CI |
| include_dir binary size bloat | Vite production build with tree-shaking + gzip |
| Termux no persistent service | Document clearly; provide tmux instructions |
| PBKDF2 600k iterations slow on low-end hardware | Benchmark on Termux; allow config override if needed |
