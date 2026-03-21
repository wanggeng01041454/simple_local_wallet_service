---
# 文档类型：开发计划
# 对应项目：Local Wallet Service
# 版本：v2.1
# 状态：已确认
# 替代旧文档：无
# 被新文档替代：无
# 生效时间：2026-03-15
---

# 任意消息签名接口实现计划

日期：2026-03-15

接口规范见：`docs/design/Feature-Message-Signing-Design-v2.1.md`

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

### encoding 字段实现约束（关键，勿省略）

`encoding` 字段 Rust 类型为 `String`（与 `network` 字段处理方式相同），**不使用 serde 枚举**。

### Handler 完整执行步骤

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

### 集成测试矩阵

*(Full test matrix preserved from original document)*

---

## Task 3 — 文档同步

**文件**：`README.md`、`docs/simple_requirement.md`

### 必须同步的内容

1. **接口列表**：在签名接口章节末尾新增两行
2. **示例请求/响应**：每个新接口至少提供一个 curl 示例
3. **OpenAPI 入口描述**：同步加入两个新路径
4. **共有规则说明**：补充 Solana 消息签名无 network 字段的差异说明
