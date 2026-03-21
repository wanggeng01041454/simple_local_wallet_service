---
# 文档类型：开发计划
# 对应项目：Local Wallet Service
# 版本：v1.0
# 状态：已确认
# 替代旧文档：无
# 被新文档替代：无
# 生效时间：2026-03-04
---

# Local Wallet Service Implementation Plan

**Goal:** Build a single Rust binary that manages encrypted Solana and EVM private keys, serves a React admin UI on port 9292, and exposes a REST API on port 9293.

**Architecture:** Cargo workspace with three crates (`wallet-core`, `wallet-server`, `wallet-cli`). The frontend is built with Vite and embedded into the binary via `include_dir`. Wallet state lives in a shared `Arc<WalletManager>` protected by `tokio::sync::RwLock`.

**Tech Stack:** Rust (axum 0.7, tokio 1, solana-sdk 2, alloy 0.9, aes-gcm 0.10, pbkdf2 0.12, rand 0.8, serde_json 1, include_dir 0.7, reqwest 0.12, tracing 0.1) · TypeScript (React 18, Vite 5, Vitest 2, React Router 6)

---

## Task 1: Cargo Workspace + Crate Stubs

> **TDD mode:** N/A — scaffolding only

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/wallet-core/Cargo.toml`
- Create: `crates/wallet-core/src/lib.rs`
- Create: `crates/wallet-server/Cargo.toml`
- Create: `crates/wallet-server/src/lib.rs`
- Create: `crates/wallet-cli/Cargo.toml`
- Create: `crates/wallet-cli/src/main.rs`

*(Full task details preserved from original document — Tasks 1 through 16)*

---

## Task 2: wallet-core — Crypto Module

> **TDD mode:** Simple Logic — pure encrypt/decrypt functions, input → output

**Files:**
- Create: `crates/wallet-core/src/crypto.rs`

---

## Task 3: wallet-core — Network Types + WalletManager Skeleton

> **TDD mode:** Simple Logic — enum exhaustiveness and state transitions

**Files:**
- Create: `crates/wallet-core/src/network.rs`
- Create: `crates/wallet-core/src/wallet.rs`

---

## Task 4: wallet-core — Solana Key Operations

> **TDD mode:** Simple Logic — generate/import/sign/address functions

**Files:**
- Create: `crates/wallet-core/src/solana_wallet.rs`

---

## Task 5: wallet-core — EVM Key Operations

> **TDD mode:** Simple Logic — generate/import/sign using alloy

**Files:**
- Create: `crates/wallet-core/src/evm_wallet.rs`

---

## Task 6: wallet-core — Telegram Notification

> **TDD mode:** Simple Logic — HTTP POST to Telegram API, silent failure

**Files:**
- Create: `crates/wallet-core/src/notification.rs`

---

## Task 7: wallet-core — Config & Storage

> **TDD mode:** Simple Logic — read/write config files

**Files:**
- Create: `crates/wallet-core/src/config.rs`

---

## Task 8: wallet-server — AppState + Server Setup

> **TDD mode:** Simple Logic — state construction and server binding

**Files:**
- Create: `crates/wallet-server/src/state.rs`
- Create: `crates/wallet-server/src/lib.rs` (update)
- Create: `crates/wallet-server/src/server.rs`

---

## Task 9: wallet-server — REST API Routes (port 9293)

> **TDD mode:** Simple Logic — HTTP handlers, test via axum-test

**Files:**
- Create: `crates/wallet-server/src/api/mod.rs`
- Create: `crates/wallet-server/src/api/handlers.rs`

---

## Task 10: wallet-server — Admin API Routes (port 9292)

> **TDD mode:** Simple Logic — admin HTTP handlers for setup, unlock, wallet management, settings

**Files:**
- Create: `crates/wallet-server/src/admin/mod.rs`

---

## Task 11: wallet-cli — Main Entry Point

> **TDD mode:** N/A — binary entry point wiring

**Files:**
- Modify: `crates/wallet-cli/src/main.rs`

---

## Task 12: Frontend Scaffold

> **TDD mode:** N/A — scaffolding

---

## Task 13: Frontend — Setup Wizard

> **TDD mode:** Simple Logic — component state transitions tested with Vitest + Testing Library

---

## Task 14: Frontend — Unlock, Dashboard, Wallets, Settings Pages

> **TDD mode:** Simple Logic — form validation and API calls tested with Vitest

---

## Task 15: Build Integration — Embed Frontend in Binary

> **TDD mode:** N/A — build pipeline integration

---

## Task 16: Installation Scripts

> **TDD mode:** N/A — shell/PowerShell scripts

---

## Summary

| Task | Description | Files |
|------|-------------|-------|
| 1 | Cargo workspace scaffold | Cargo.toml, 3 crate stubs |
| 2 | Crypto: AES-256-GCM + PBKDF2 | crypto.rs |
| 3 | Network enum + WalletManager skeleton | network.rs, wallet.rs |
| 4 | Solana key ops | solana_wallet.rs |
| 5 | EVM key ops | evm_wallet.rs |
| 6 | Telegram notification | notification.rs |
| 7 | Config + encrypted secret storage | config.rs |
| 8 | AppState + dual-server setup | state.rs, server.rs |
| 9 | REST API routes (port 9293) | api/mod.rs |
| 10 | Admin API routes (port 9292) | admin/mod.rs |
| 11 | CLI entry point | main.rs |
| 12 | Frontend scaffold (Vite + React) | frontend/ |
| 13 | Setup wizard page | Setup.tsx |
| 14 | Unlock, Dashboard, Wallets, Settings | 4 page files |
| 15 | Embed frontend in binary | static_files.rs, include_dir |
| 16 | Installation scripts | install/ |
