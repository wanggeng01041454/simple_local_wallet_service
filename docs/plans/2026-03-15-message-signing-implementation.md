# 任意消息签名接口实现计划

日期：2026-03-15

接口规范见：`docs/plans/2026-03-15-message-signing-redesign.md`

---

## 执行顺序

```
Task 0 → Task 1 → Task 2 → Task 3 → cargo test → commit
```

Task 0 和 Task 1 互相独立，均为 Task 2 的前置依赖。Task 3（文档同步）在 Task 2 完成后执行。

---

## Task 0 — wallet-core: 新增 `EvmMessageSignature` 和 `sign_message`

**文件**：`crates/wallet-core/src/evm_wallet.rs`

### 新增结构体

```rust
#[derive(Debug)]
pub struct EvmMessageSignature {
    pub message_hash: String,  // "0x" + 32-byte keccak256 hex (66 chars)
    pub signature: String,     // "0x" + 65-byte (r||s||v) hex (132 chars)
    pub r: String,             // "0x" + 32-byte hex (66 chars)
    pub s: String,             // "0x" + 32-byte hex (66 chars)
    pub v: String,             // "0x1b" or "0x1c"
}
```

### 新增函数

```rust
pub fn sign_message(private_key_bytes: &[u8], msg_bytes: &[u8]) -> Result<EvmMessageSignature, WalletError>
```

实现要点：
- 新增导入：`use alloy::primitives::eip191_hash_message;`（加入现有 `use alloy::primitives::{B256, TxKind, U256};` 行）
- `let hash = eip191_hash_message(msg_bytes);` 计算 EIP-191 哈希，返回 `B256`
- `PrivateKeySigner::sign_hash_sync(&hash)` 签名（与 `sign_typed_data` 路径相同）
- v 值处理与 `sign_typed_data` 一致：`sig.as_bytes()[64]` 已是 27/28，直接 `format!("0x{:02x}", sig_bytes[64])`

### 单元测试（TDD，先写测试再实现）

| 测试名 | 断言目标 |
|--------|---------|
| `sign_message_signature_lengths_correct` | `signature` 132 字符；`r`/`s` 各 66；`v` 为 `"0x1b"` 或 `"0x1c"` |
| `sign_message_hash_matches_eip191` | 用 `alloy::primitives::eip191_hash_message` 独立计算，断言 `message_hash` 一致 |
| `sign_message_signer_recoverable` | 从 `message_hash` + `signature` ecrecover 出地址等于本服务地址 |
| `sign_message_empty_bytes_succeeds` | 空字节也能成功签名，返回 Ok |
| `sign_message_invalid_key_returns_error` | 非 32 字节 key 返回 `WalletError::InvalidTransaction` |

---

## Task 1 — wallet-core: 扩展 `SignEvent` 枚举

**文件**：`crates/wallet-core/src/notification.rs`

### 改动

1. `SignEvent` 新增两个变体：

```rust
SolanaMessage {
    /// request 中 message 字段原文（解码前的原始字符串）
    message_preview: String,
    signed_at: String,
},
EvmMessage {
    network: String,
    message_hash: String,
    /// request 中 message 字段原文（解码前的原始字符串）
    message_preview: String,
    signed_at: String,
},
```

2. `format_sign_message` 新增两个 match 分支：
   - 对 `message_preview` 先调用 `escape_markdown`，再调用 `truncate_str(..., 60)` 追加 `"..."`（与现有 Solana 交易分支处理方式一致）
   - 对 `message_hash` 也调用 `escape_markdown`

### 单元测试

| 测试名 | 断言目标 |
|--------|---------|
| `format_solana_message_contains_preview` | 消息预览出现在通知文本中 |
| `format_solana_message_truncates_long_message` | 超过 60 字符时出现 `"..."` |
| `format_evm_message_contains_hash_and_preview` | `message_hash` 和消息预览均出现，含网络名 |

---

## Task 2 — wallet-server: 新增两个 handler 和路由

**文件**：`crates/wallet-server/src/api/mod.rs`

### request_id 实现约束（关键，勿省略）

所有新 Request struct 的 `request_id` 字段必须遵循以下固定模式：

```rust
/// 请求唯一标识，原样透传至响应
#[serde(default)]
#[schema(required = true, value_type = String)]
request_id: Option<String>,
```

- `#[serde(default)]` 保证缺字段时反序列化为 `None`（而非 422）
- `#[schema(required = true, value_type = String)]` 保证 OpenAPI 标注为必填
- Handler 内第一步提取 `rid`，**必须同时处理 `None` 和空字符串**，与现有 handler 一致：

```rust
let rid = match req.request_id.as_deref() {
    Some(id) if !id.is_empty() => id.to_string(),
    _ => return (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": "missing_request_id"})),
    ),
};
let rid = rid.as_str();
```

### encoding 字段实现约束（关键，勿省略）

`encoding` 字段 Rust 类型为 `String`（与 `network` 字段处理方式相同），**不使用 serde 枚举**。
在 handler 内 `request_id` 校验之后 match `req.encoding.as_str()`，非法值返回 400 `invalid_encoding`（含 `request_id`）：

```rust
// 示意，两个 handler 均按此模式
let encoding = req.encoding.as_str();
match encoding {
    "utf8" | "base58" | "base64" | "hex" => { /* 继续 */ }
    _ => return (StatusCode::BAD_REQUEST, Json(json!({
        "error": "invalid_encoding",
        "request_id": rid
    }))).into_response(),
}
```

### Handler 完整执行步骤（两个 handler 均按此顺序，勿调换）

校验优先级对齐 `2026-03-11-signing-api-redesign.md`：字段合法性 → 钱包锁定 → 业务逻辑。

**`sign_solana_message` 步骤：**

```
步骤 1: 提取 rid（None 或空字符串 → 400 missing_request_id，无 request_id 字段）
步骤 2: encoding 校验 → 不在 ["utf8","base58","base64","hex"] → 400 invalid_encoding（含 rid）
步骤 3: message 解码 → 失败 → 400 invalid_message（含 rid）
步骤 4: is_unlocked() 检查 → false → send_telegram(format_locked_message(...)) → 503 wallet_locked（含 rid）
步骤 5: get_private_key_bytes(&Network::Solana) → NotFound → 404 / Locked → 503 / 其他 → 500
步骤 6: solana_wallet::sign_message → Err → 500 sign_failed（含 rid）
步骤 7: 构造 SignEvent::SolanaMessage，调用 send_telegram（失败只记日志）
步骤 8: 返回 200
```

**`sign_evm_message` 步骤：**

```
步骤 1: 提取 rid（None 或空字符串 → 400 missing_request_id，无 request_id 字段）
步骤 2: encoding 校验 → 不在 ["utf8","hex"] → 400 invalid_encoding（含 rid）
步骤 3: network 校验 → parse_network(&req.network) 失败 → 400 invalid_network（含 rid）
步骤 4: message 解码 → 失败 → 400 invalid_message（含 rid）
步骤 5: is_unlocked() 检查 → false → send_telegram(format_locked_message(...)) → 503 wallet_locked（含 rid）
步骤 6: get_private_key_bytes(&network) → NotFound → 404 / Locked → 503 / 其他 → 500
步骤 7: evm_wallet::sign_message → Err → 500 sign_failed（含 rid）
步骤 8: 构造 SignEvent::EvmMessage，调用 send_telegram（失败只记日志）
步骤 9: 返回 200
```

**注意**：`sign_solana_message` 无 `network` 字段，步骤 5 固定传 `wallet_core::Network::Solana`。

### message 解码 API（在 handler 内实现，在步骤 4 使用）

```rust
// utf8
let msg_bytes = req.message.as_bytes().to_vec();  // 不会失败

// base64（使用 base64::engine::general_purpose::STANDARD，不是 base64::decode()）
let msg_bytes = base64::engine::general_purpose::STANDARD
    .decode(&req.message)
    .map_err(|_| /* 400 invalid_message */)?;

// base58
let msg_bytes = bs58::decode(&req.message)
    .into_vec()
    .map_err(|_| /* 400 invalid_message */)?;

// hex（必须先校验 0x 前缀，再解码）
if !req.message.starts_with("0x") {
    return /* 400 invalid_message */;
}
let msg_bytes = hex::decode(&req.message[2..])
    .map_err(|_| /* 400 invalid_message */)?;
```

`ValidatedJson` 提取器（`crates/wallet-server/src/extractor.rs`）负责解析层错误（415/400/422），handler 内只处理业务逻辑错误。

### Telegram 通知约定（关键，勿省略）

- Handler 将 request 中的 `message` 字段**原始字符串**（解码前）传入 `SignEvent`
- **不在 handler 层截断**，截断和 Markdown 转义统一由 `notification.rs` 处理

### 新增 Request/Response schema

- `SignSolanaMessageRequest` — `request_id`(Option<String>), `encoding`(String), `message`(String)
- `SignSolanaMessageResponse` — `request_id`(String), `signature`(String)
- `SignEvmMessageRequest` — `request_id`(Option<String>), `network`(String), `encoding`(String), `message`(String)
- `SignEvmMessageResponse` — `request_id`(String), `network`(String), `message_hash`(String), `signature`(String), `r`(String), `s`(String), `v`(String)

### 路由注册

```
POST /api/wallet/sign/solana/message
POST /api/wallet/sign/evm/message
```

### OpenAPI 更新

- `paths` 加入 `sign_solana_message` 和 `sign_evm_message`
- `components(schemas)` 加入四个新 schema（encoding 字段类型为 String，不注册枚举）
- 每个 handler 的 `#[utoipa::path]` 标注响应码：200 / 400 / 415 / 422 / 503 / 404 / 500

### 集成测试基础设施

**Solana message 测试**组织在 `mod solana_message_sign_tests` 下，复用已有 helper：
- `unlocked_server_with_solana()` → 返回 `(TestServer, TempDir, SolanaKeypair)`，用于成功路径和业务错误测试
- 锁定测试：`let state = AppState::new(...); let server = TestServer::new(router(state))`（不 unlock）
- `wallet_not_found` 测试：unlock 时只保存 ETH 钱包（`Network::Eth`），不保存 Solana 钱包，参考现有 `sign_solana_wallet_not_found_returns_404` 的 setup

**EVM message 测试**组织在 `mod evm_message_sign_tests` 下，复用：
- `unlocked_eth_server()` → 返回 `(TestServer, TempDir)`，用于成功路径测试
- `wallet_not_found` 测试：unlock 时只保存 Solana 钱包，请求 `network: "eth"` → 404

**OpenAPI 回归测试**：在现有 `openapi_does_not_contain_old_sign_path` 测试中**追加断言**，不新建独立测试。使用结构化 JSON 路径访问（与现有断言风格一致，不用字符串搜索）：

```rust
// 追加到现有测试末尾

// 新路径已注册
assert!(body["paths"]["/api/wallet/sign/solana/message"].is_object());
assert!(body["paths"]["/api/wallet/sign/evm/message"].is_object());

// request_id 在新 schema 中为 required
let solana_msg_req = &schemas["SignSolanaMessageRequest"];
let evm_msg_req = &schemas["SignEvmMessageRequest"];
assert!(
    solana_msg_req["required"].as_array().unwrap().iter().any(|v| v == "request_id"),
    "SignSolanaMessageRequest.request_id must be required"
);
assert!(
    evm_msg_req["required"].as_array().unwrap().iter().any(|v| v == "request_id"),
    "SignEvmMessageRequest.request_id must be required"
);
```

### 集成测试矩阵

#### Solana message

**契约测试（解析层，无 request_id）：**

| 测试名 | 断言目标 |
|--------|---------|
| `sign_solana_message_unsupported_media_type_returns_415` | 415；body `{"error":"unsupported_media_type"}`；无 request_id 字段 |
| `sign_solana_message_empty_body_returns_400` | 400；`{"error":"empty_body"}`；无 request_id 字段 |
| `sign_solana_message_malformed_json_returns_400` | 400；`{"error":"malformed_json"}`；无 request_id 字段 |
| `sign_solana_message_invalid_body_returns_422` | 422；`{"error":"invalid_body"}`；无 request_id 字段 |

**业务测试：**

| 测试名 | 断言目标 |
|--------|---------|
| `sign_solana_message_missing_request_id_returns_400` | 400 `missing_request_id`，无 request_id 字段 |
| `sign_solana_message_utf8_success_signature_verifiable` | 200；`signature` base58 解码为 64 字节；Ed25519 验签对原始 UTF-8 字节有效 |
| `sign_solana_message_base64_success_signature_verifiable` | 200；`signature` base58 解码为 64 字节；Ed25519 验签通过 |
| `sign_solana_message_hex_success` | 200；签名结果正确 |
| `sign_solana_message_base58_success` | 200；签名结果正确 |
| `sign_solana_message_utf8_and_hex_same_bytes_same_signature` | utf8 与等价 hex 输入产生相同签名 |
| `sign_solana_message_invalid_encoding_returns_400` | 400 `invalid_encoding`，含 request_id |
| `sign_solana_message_invalid_hex_returns_400` | 400 `invalid_message`，含 request_id |
| `sign_solana_message_invalid_base64_returns_400` | 400 `invalid_message`，含 request_id |
| `sign_solana_message_locked_returns_503` | 503 `wallet_locked`，含 request_id |
| `sign_solana_message_wallet_not_found_returns_404` | 404 `wallet_not_found`，含 request_id |

#### EVM message

**契约测试（解析层，无 request_id）：**

| 测试名 | 断言目标 |
|--------|---------|
| `sign_evm_message_unsupported_media_type_returns_415` | 415；无 request_id 字段 |
| `sign_evm_message_empty_body_returns_400` | 400 `empty_body`；无 request_id 字段 |
| `sign_evm_message_malformed_json_returns_400` | 400 `malformed_json`；无 request_id 字段 |
| `sign_evm_message_invalid_body_returns_422` | 422 `invalid_body`；无 request_id 字段 |

**业务测试：**

| 测试名 | 断言目标 |
|--------|---------|
| `sign_evm_message_missing_request_id_returns_400` | 400 `missing_request_id` |
| `sign_evm_message_utf8_success_signer_recoverable` | 200；ecrecover 地址等于本服务 EVM 地址 |
| `sign_evm_message_hex_success_signer_recoverable` | 200；ecrecover 地址等于本服务 EVM 地址 |
| `sign_evm_message_utf8_and_hex_same_bytes_same_signature` | utf8 与等价 hex 输入产生相同签名 |
| `sign_evm_message_hash_matches_eip191` | `message_hash` 与独立计算的 `eip191_hash_message` 一致 |
| `sign_evm_message_invalid_encoding_returns_400` | 400 `invalid_encoding`，含 request_id |
| `sign_evm_message_invalid_hex_no_0x_prefix_returns_400` | 400 `invalid_message`，含 request_id |
| `sign_evm_message_invalid_hex_bad_chars_returns_400` | 400 `invalid_message`，含 request_id |
| `sign_evm_message_invalid_network_returns_400` | 400 `invalid_network`，含 request_id |
| `sign_evm_message_locked_returns_503` | 503 `wallet_locked`，含 request_id |
| `sign_evm_message_wallet_not_found_returns_404` | 404 `wallet_not_found`，含 request_id |

**OpenAPI 回归：** 追加到现有 `openapi_does_not_contain_old_sign_path` 测试（见"集成测试基础设施"章节）。断言 `body["paths"]["/api/wallet/sign/evm/message"].is_object()` 和两个新 schema 的 `required` 数组含 `request_id`。

---

## Task 3 — 文档同步

**文件**：`README.md`、`docs/simple_requirement.md`

### 必须同步的内容

1. **接口列表**：在签名接口章节末尾新增两行：

   | 接口 | 说明 |
   |------|------|
   | `POST /api/wallet/sign/solana/message` | Solana 任意消息签名（Ed25519，原始字节，无前缀） |
   | `POST /api/wallet/sign/evm/message` | EVM 任意消息签名（EIP-191 personal_sign，keccak256 哈希后 ECDSA） |

2. **示例请求/响应**：每个新接口至少提供一个 curl 示例或 JSON 示例，与现有接口保持风格一致（参考文档中已有的示例格式）。

3. **OpenAPI 入口描述**：`api/mod.rs` 中 `#[openapi(info(description = "..."))]` 里的接口简介需同步加入两个新路径。

4. **共有规则说明**：若文档中有"所有签名接口的共同规则"章节（如 request_id 必填、支持的网络等），需确认新接口是否适用并补充说明（Solana 消息签名无 network 字段，此差异需注明）。
