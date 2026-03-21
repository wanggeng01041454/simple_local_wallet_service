---
# 文档类型：系统设计
# 对应项目：Local Wallet Service
# 版本：v1.1
# 状态：已确认
# 替代旧文档：System-Design-v1.0.md
# 被新文档替代：无
# 生效时间：2026-03-21
---

# Local Wallet Service — Design Doc

Date: 2026-03-04 | Updated: 2026-03-21

## 1. 需求概述

实现一个本地钱包服务，安装后作为系统独立服务运行，同时提供网页管理界面和 REST API 接口。

### 功能需求

1. 支持 Solana 网络
2. 支持 EVM 兼容网络（Eth / BNB / Arb / Polygon）
3. 每个网络只维护 1 个钱包
4. 钱包私钥加密后存放在文件中
5. 每次服务重启后，需要用户通过管理界面输入密码解密钱包私钥
6. 私钥解密后常驻内存
7. 外部应用通过 REST API 查询地址和使用私钥对交易进行签名
8. 每次签名时通过 Telegram Bot 发送通知到管理员账号

### 管理界面（端口 9292）

1. 设置钱包密码
2. 导入钱包私钥
3. 创建钱包私钥（每个网络仅 1 个）
4. 查看钱包地址
5. 查看钱包余额
6. 转账

### REST API（端口 9293）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/wallet/address` | 获取钱包地址 |
| GET | `/api/wallet/balance` | 获取钱包余额 |
| POST | `/api/wallet/sign/solana` | Solana 交易签名 |
| POST | `/api/wallet/sign/evm/transaction` | EVM 交易签名（Legacy/EIP-2930/EIP-1559） |
| POST | `/api/wallet/sign/evm/typed-data` | EIP-712 结构化数据签名 |
| POST | `/api/wallet/sign/solana/message` | Solana 任意消息签名（Ed25519） |
| POST | `/api/wallet/sign/evm/message` | EVM 任意消息签名（EIP-191） |

> **Breaking Change (v2.0):** `POST /api/wallet/sign` 已移除，请迁移到上述专用签名接口。

所有签名接口均需提供 `request_id`（必填），Solana 签名接口除外均需提供 `network` 字段，且要求钱包处于已解锁状态。

### 技术选型

- 后端：Rust
- 前端：React + TypeScript
- 支持平台：Windows、Linux (systemd)、macOS、Android Termux

---

## 2. Overview

A locally-installed wallet service that manages private keys for Solana and EVM-compatible networks (Ethereum, BNB Chain, Arbitrum, Polygon). Exposes a web-based admin UI on port 9292 and a REST API on port 9293. One wallet per network. Private keys are encrypted at rest and decrypted into memory after the user unlocks the service via the admin UI.

## 3. Architecture

Single Rust binary embedding a React frontend.

```
local-wallet (single binary)
├── Admin Server — 0.0.0.0:9292 (axum)
│     └── Serves embedded React static files
│         + Admin API routes (/api/admin/*)
├── API Server   — 127.0.0.1:9293 (axum)
│     └── /api/wallet/address
│         /api/wallet/balance
│         /api/wallet/sign/solana
│         /api/wallet/sign/evm/transaction
│         /api/wallet/sign/evm/typed-data
│         /api/wallet/sign/solana/message
│         /api/wallet/sign/evm/message
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
    ├── arb.enc
    └── polygon.enc
```

## 4. Tech Stack

| Layer | Choice |
|-------|--------|
| Language (backend) | Rust (stable) |
| HTTP framework | axum + tokio |
| Solana | solana-sdk + solana-client |
| EVM | alloy (RLP encode/decode, signing, EIP-712) |
| EVM dynamic typing | alloy-dyn-abi (EIP-712 runtime type resolution) |
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

## 5. Project Structure

```
local-wallet/
├── Cargo.toml                # workspace
├── crates/
│   ├── wallet-core/          # encryption, signing, network clients
│   │   ├── src/
│   │   │   ├── crypto.rs     # AES-256-GCM + PBKDF2
│   │   │   ├── wallet.rs     # WalletManager, lock/unlock
│   │   │   ├── solana_wallet.rs   # Solana key ops, transaction/message signing
│   │   │   ├── evm_wallet.rs      # EVM key ops, tx/EIP-712/EIP-191 signing
│   │   │   ├── network.rs    # Network enum with chainId mapping
│   │   │   ├── balance.rs    # On-chain balance queries via JSON-RPC
│   │   │   ├── config.rs     # AppConfig (RPC URLs, encrypted Telegram settings)
│   │   │   └── notification.rs  # SignEvent enum, Telegram client
│   │   └── Cargo.toml
│   ├── wallet-server/        # axum HTTP servers
│   │   ├── src/
│   │   │   ├── admin/        # admin UI routes
│   │   │   ├── api/          # REST API routes + OpenAPI (utoipa)
│   │   │   ├── state.rs      # shared AppState
│   │   │   ├── extractor.rs  # ValidatedJson custom extractor
│   │   │   └── static_files.rs  # embedded frontend assets
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

## 6. Encryption Scheme

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

## 7. Wallet Lifecycle

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

## 8. Network 与 Chain ID 映射

| network 标识 | 链名称 | Chain ID |
|-------------|--------|----------|
| `solana` | Solana | N/A |
| `eth` | Ethereum | 1 |
| `bnb` | BNB Chain | 56 |
| `arb` | Arbitrum One | 42161 |
| `polygon` | Polygon | 137 |

## 9. REST API (port 9293, 127.0.0.1 only)

### 全局规则

**request_id 机制（所有签名接口）：**
- 所有签名接口通过 JSON Body 传递 `request_id`，服务原样透传至响应（含错误响应）
- 缺少 `request_id` 时返回 400 `missing_request_id`
- `request_id` 不提供幂等保护

**请求解析错误（进入 handler 前，无 request_id）：**

| 场景 | HTTP 状态码 | error 字段 |
|------|------------|-----------|
| Content-Type 不是 application/json | 415 | `unsupported_media_type` |
| Body 为空 | 400 | `empty_body` |
| JSON 语法错误 | 400 | `malformed_json` |
| JSON 字段类型不匹配 | 422 | `invalid_body` |

通过自定义 `ValidatedJson` 提取器（包装 axum `Json<T>`）统一格式化。

**校验优先级（从高到低）：**
1. Content-Type / JSON 解析（axum 层，无 request_id）
2. `request_id` 字段存在性
3. `network` / `encoding` 等枚举字段合法性
4. 消息解码合法性
5. 钱包锁定状态
6. 业务逻辑校验

### GET /api/wallet/address
```
Query: ?network=solana|eth|bnb|arb|polygon
Response 200: { "network": "eth", "address": "0x..." }
Response 503: { "error": "wallet_locked" }
  + Telegram notification
```

### GET /api/wallet/balance
```
Query: ?network=solana|eth|bnb|arb|polygon
Response 200: { "network": "eth", "address": "0x...", "balance": "1.234", "unit": "ETH" }
Response 503: { "error": "wallet_locked" }
  + Telegram notification
```

### POST /api/wallet/sign/solana — 交易签名

详见 `Feature-Signing-API-Design-v2.0.md` 接口 1。

- 输入：`request_id` + `encoding`(base64/base58) + `transaction`
- 输出：`request_id` + `transaction`(base58)
- 支持 Legacy Transaction 和 VersionedTransaction（先尝试 Versioned）
- 支持多签场景（只填充本服务持有密钥对应的 signer 位置）

### POST /api/wallet/sign/evm/transaction — EVM 交易签名

详见 `Feature-Signing-API-Design-v2.0.md` 接口 2。

- 输入：`request_id` + `network` + `transaction`(0x hex RLP)
- 输出：`request_id` + `network` + `transaction`(0x hex RLP, signed)
- 支持 Type 0 (Legacy) / Type 1 (EIP-2930) / Type 2 (EIP-1559)，不支持 Type 3
- chainId 校验：Type 0 自动注入，Type 1/2 不一致时返回 `chain_id_mismatch`

### POST /api/wallet/sign/evm/typed-data — EIP-712 签名

详见 `Feature-Signing-API-Design-v2.0.md` 接口 3。

- 输入：`request_id` + `network` + `typed_data`(EIP-712 JSON, 对齐 eth_signTypedData_v4)
- 输出：`request_id` + `network` + `signature` + `r` + `s` + `v`
- `domain.chainId` 缺失时自动注入，不一致时返回 `chain_id_mismatch`
- `EIP712Domain` 客户端可传可不传，服务端自行构造

### POST /api/wallet/sign/solana/message — Solana 消息签名

详见 `Feature-Message-Signing-Design-v2.1.md` 接口 4。

- 输入：`request_id` + `encoding`(utf8/base58/base64/hex) + `message`
- 输出：`request_id` + `signature`(base58)
- 无 `network` 字段，始终使用 Solana 密钥
- Ed25519 直接签名原始字节，无前缀

### POST /api/wallet/sign/evm/message — EVM 消息签名（EIP-191）

详见 `Feature-Message-Signing-Design-v2.1.md` 接口 5。

- 输入：`request_id` + `network` + `encoding`(utf8/hex) + `message`
- 输出：`request_id` + `network` + `message_hash` + `signature` + `r` + `s` + `v`
- 遵循 EIP-191：追加 `"\x19Ethereum Signed Message:\n{len}"` 前缀后 keccak256 哈希再签名

### 错误码汇总

| HTTP | error | request_id | 触发场景 |
|------|-------|:----------:|---------|
| 415 | `unsupported_media_type` | 否 | Content-Type 不是 application/json |
| 400 | `empty_body` | 否 | Body 为空 |
| 400 | `malformed_json` | 否 | JSON 语法错误 |
| 422 | `invalid_body` | 否 | JSON 字段类型不匹配 |
| 400 | `missing_request_id` | 否 | 未传 request_id |
| 400 | `invalid_network` | 是 | network 不在支持列表 |
| 400 | `invalid_encoding` | 是 | encoding 不在支持列表 |
| 400 | `invalid_transaction` | 是 | 交易反序列化失败或结构损坏 |
| 400 | `invalid_message` | 是 | message 解码失败 |
| 400 | `invalid_typed_data` | 是 | EIP-712 数据格式或字段不匹配 |
| 400 | `already_signed` | 是 | 交易已包含有效签名 |
| 400 | `chain_id_mismatch` | 是 | chainId 与 network 不一致 |
| 400 | `unsupported_tx_type` | 是 | 不支持的交易类型（如 Type 3） |
| 400 | `signer_not_required` | 是 | 本服务 pubkey 不在 Solana 交易的 required signers 中 |
| 503 | `wallet_locked` | 是 | 钱包未解锁 |
| 404 | `wallet_not_found` | 是 | 该网络无对应钱包 |
| 500 | `sign_failed` | 是 | 签名过程内部错误 |

## 10. Admin UI Pages

| Route | Description |
|-------|-------------|
| `/setup` | First-time wizard: set password → import/create wallets |
| `/unlock` | Password entry to unlock wallet after restart |
| `/dashboard` | Home after unlock |
| `/wallets` | View addresses, balances, import/create keys, transfer |
| `/settings` | Telegram Bot config (Token + Chat ID), RPC URLs per network |

## 11. Telegram Notifications

**Configuration:** Stored in `secret.enc` (encrypted with wallet password). Set via `/settings` in admin UI.

**SignEvent 枚举（每种签名类型独立建模）：**

| 变体 | 通知内容 |
|------|---------|
| `Solana` | TxID、交易（前 60 字符截断）、时间 |
| `EvmTransaction` | 网络、From、To、Value、时间 |
| `EvmTypedData` | 网络、typed_data JSON（前 500 字符截断）、时间 |
| `SolanaMessage` | 消息预览（前 60 字符截断）、时间 |
| `EvmMessage` | 网络、message_hash、消息预览（前 60 字符截断）、时间 |

**锁定状态通知：** API request received while wallet is locked → Telegram notification。

**安全处理：**
- 用户数据中的 Markdown 特殊字符发送前转义
- 消息超长时按接口类型截断（详见各接口通知模板）
- 发送失败仅记日志，不阻塞主流程，不重试

**Default RPC endpoints (overridable via /settings):**
```
Solana:   https://api.mainnet-beta.solana.com
Ethereum: https://eth.llamarpc.com
BNB:      https://bsc-dataseed.binance.org
Arbitrum: https://arb1.arbitrum.io/rpc
Polygon:  https://polygon-rpc.com
```

## 12. Installation

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

## 13. Testing Strategy

**wallet-core (unit tests):**
- Encrypt/decrypt roundtrip with known test vectors
- Wrong password returns error
- Solana signing: transaction signing (Legacy + Versioned), message signing
- EVM signing: transaction signing (Type 0/1/2), EIP-712 typed data, EIP-191 message
- Notification: SignEvent format for all 5 variants, Markdown escaping, truncation

**wallet-server (integration tests):**
- `axum-test` with `TestServer` and `tempfile::TempDir` for isolated state
- All 7 API endpoints: success path + error paths
- Parse-phase errors (415/400/422) without request_id
- Business errors with request_id passthrough
- Wallet locked → 503 + Telegram notification
- OpenAPI schema validation

**Frontend (Vitest):**
- Setup wizard step transitions
- Unlock form validation
- Settings form save/load

## 14. Key Risks

| Risk | Mitigation |
|------|------------|
| solana-sdk / alloy version conflicts | Pin versions in Cargo.lock, test on CI |
| include_dir binary size bloat | Vite production build with tree-shaking + gzip |
| Termux no persistent service | Document clearly; provide tmux instructions |
| PBKDF2 600k iterations slow on low-end hardware | Benchmark on Termux; allow config override if needed |
| EIP-712 dynamic type complexity | Use alloy-dyn-abi for runtime resolution; comprehensive test coverage |
