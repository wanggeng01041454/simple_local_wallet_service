# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A local multi-chain crypto wallet service (Solana + EVM chains: ETH, BNB, ARB, Polygon). It runs two HTTP servers locally:
- **Admin UI** on port 9292 — wallet setup, unlock/lock, settings (React SPA served by the Rust backend)
- **REST API** on port 9293 — address query, balance query, transaction signing, message signing

Wallets are encrypted at rest with AES-GCM (PBKDF2 key derivation) and must be unlocked via password before any signing operation.

## Build & Run

```bash
# Full build (frontend + Rust release binary)
./build.sh

# Rust only (dev)
cargo build

# Run
cargo run              # or ./target/release/local-wallet
```

## Testing

```bash
# All Rust tests
cargo test

# Single crate
cargo test -p wallet-server
cargo test -p wallet-core

# Single test
cargo test -p wallet-server sign_solana_legacy_success

# Frontend
cd frontend && npm test
```

## Workspace Architecture

Cargo workspace with 3 crates:

- **`wallet-core`** — cryptographic operations, wallet management, config, balance queries, notifications
  - `wallet.rs` — `WalletManager` (encrypt/decrypt/lock/unlock wallets, password verification)
  - `solana_wallet.rs` — Solana keypair generation, import, transaction signing, message signing (Ed25519)
  - `evm_wallet.rs` — EVM keypair generation, import, transaction signing (Legacy/EIP-2930/EIP-1559), EIP-712 typed data signing, EIP-191 message signing
  - `crypto.rs` — AES-GCM encryption/decryption with PBKDF2
  - `network.rs` — `Network` enum (Solana, Eth, Bnb, Arb, Polygon) with chain IDs
  - `balance.rs` — on-chain balance queries via JSON-RPC
  - `config.rs` — `AppConfig` (RPC URLs, encrypted Telegram settings)
  - `notification.rs` — Telegram notifications for signing events

- **`wallet-server`** — HTTP layer (Axum)
  - `server.rs` — binds both servers (9292 admin, 9293 API), sets up CORS
  - `api/mod.rs` — REST API routes and handlers with OpenAPI (utoipa) annotations
  - `admin/mod.rs` — admin routes (health, status, unlock, lock, setup, settings)
  - `state.rs` — `AppState` (shared wallet manager, config, Telegram client)
  - `extractor.rs` — `ValidatedJson` custom extractor for differentiated error responses (415/400/422)
  - `static_files.rs` — embedded frontend assets via `include_dir`

- **`wallet-cli`** — binary entry point (`local-wallet`), sets up tracing and data directory

## Frontend

React + TypeScript + Vite app in `frontend/`. Built assets are embedded into the Rust binary at compile time.

```bash
cd frontend
npm install
npm run dev    # dev server on :5173 (proxied to backend)
npm run build  # production build -> frontend/dist/
```

## Key Patterns

- **Wallet state**: Wallets start locked. All signing endpoints return 503 `wallet_locked` when locked. Unlock via `POST /api/admin/unlock`.
- **request_id**: All signing endpoints require a `request_id` field (mandatory). Parse-phase errors (415, 422) intentionally omit `request_id` in the response.
- **Signing endpoints** are chain-specific: `/api/wallet/sign/solana`, `/api/wallet/sign/evm/transaction`, `/api/wallet/sign/evm/typed-data`, `/api/wallet/sign/solana/message`, `/api/wallet/sign/evm/message`.
- **Tests** use `axum-test` with `TestServer` and `tempfile::TempDir` for isolated wallet state. Test helpers like `unlocked_server_with_solana()` create pre-configured test servers.
- **Data directory**: `~/.local-wallet` on Unix, `%APPDATA%/local-wallet` on Windows.

## Documentation Rules (from MEMORY.md)

- Design docs go in `docs/design/`, plans in `docs/plan/`
- Always update `docs/INDEX.md` when adding/renaming/changing document status
- Documents must include a YAML front-matter header with type, version, status fields
- Version format: `vX.0` for new designs, `vX.Y` (Y>0) for compatible changes
