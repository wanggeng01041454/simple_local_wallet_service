---
# 文档类型：功能设计
# 对应项目：Local Wallet Service
# 版本：v2.1
# 状态：已确认
# 替代旧文档：无（兼容扩展 Feature-Signing-API-Design-v2.0）
# 被新文档替代：无
# 生效时间：2026-03-15
---

# 任意消息签名接口规范

日期：2026-03-15

## 背景

在现有三个签名接口（Solana 交易、EVM 交易、EIP-712 结构化数据）基础上，新增两个任意消息签名接口。

| 新增接口 | 路径 |
|---------|------|
| Solana 消息签名 | `POST /api/wallet/sign/solana/message` |
| EVM 消息签名（EIP-191） | `POST /api/wallet/sign/evm/message` |

---

## 全局规则

与现有签名接口完全一致，包括：
- `request_id` 必填，原样透传至所有响应（含错误响应）
- 缺少 `request_id` 时返回 400 `missing_request_id`
- 解析阶段错误（415 / 400 empty_body / 400 malformed_json / 422）不含 `request_id`
- 校验优先级：Content-Type → JSON 解析 → request_id → 字段合法性（encoding / network / message 解码）→ 钱包锁定 → 业务逻辑

详见 `Feature-Signing-API-Design-v2.0.md` 全局规则章节。

---

## 接口 4 — Solana 消息签名

```
POST /api/wallet/sign/solana/message
```

对原始字节进行 Ed25519 签名，不添加任何前缀。与 Phantom 等主流 Solana 钱包行为一致。

### Request Body

```json
{
  "request_id": "uuid-xxx",
  "encoding": "utf8",
  "message": "Sign in to MyApp"
}
```

| 字段 | 类型 | 必填 | OpenAPI 注释要求 |
|------|------|------|-----------------|
| `request_id` | string | 是 | 请求唯一标识，原样透传至响应 |
| `encoding` | string | 是 | 消息编码格式，枚举值：`"utf8"` \| `"base58"` \| `"base64"` \| `"hex"` |
| `message` | string | 是 | 要签名的消息；encoding 为 utf8 时为明文字符串，其余按对应格式解码为原始字节 |

**encoding 解码规则：**

| encoding | 解码方式 | message 格式示例 | 解码失败行为 |
|----------|---------|----------------|------------|
| `"utf8"` | 直接取 UTF-8 字节 | `"Sign in to MyApp"` | 不会失败 |
| `"base58"` | base58 解码 | `"3YZ..."` | 400 `invalid_message` |
| `"base64"` | standard base64 解码 | `"SGVsbG8="` | 400 `invalid_message` |
| `"hex"` | **必须**以 `0x` 开头，再对后续字符十六进制解码 | `"0x48656c6c6f"` | 缺 `0x` 前缀或含非法字符均返回 400 `invalid_message` |

### Response

```json
{
  "request_id": "uuid-xxx",
  "signature": "<base58 编码的 64 字节 Ed25519 签名>"
}
```

| 字段 | 类型 | OpenAPI 注释要求 |
|------|------|-----------------|
| `request_id` | string | 透传自请求的唯一标识 |
| `signature` | string | 64 字节 Ed25519 签名，固定 base58 编码（约 88 字符） |

### 行为说明

- 不需要 `network` 字段，始终使用 Solana 密钥对
- 底层调用 `wallet_core::solana_wallet::sign_message(private_key_bytes, &msg_bytes)`
- 签名结果固定 base58 编码输出，与输入 encoding 无关

---

## 接口 5 — EVM 消息签名（EIP-191）

```
POST /api/wallet/sign/evm/message
```

遵循 EIP-191 标准：签名前在消息字节前追加前缀 `"\x19Ethereum Signed Message:\n{len}"` 并取 keccak256 哈希后签名。

- `utf8` encoding 对应 MetaMask `personal_sign` / viem `signMessage` / ethers.js `signMessage` 传入字符串的场景
- `hex` encoding 对应调用方已将消息编码为原始字节序列的场景

不支持裸签哈希（直接对 32 字节摘要签名）。若需对结构化数据签名，请使用 EIP-712 接口。

### Request Body

```json
{
  "request_id": "uuid-xxx",
  "network": "eth",
  "encoding": "utf8",
  "message": "Sign in to MyApp"
}
```

| 字段 | 类型 | 必填 | OpenAPI 注释要求 |
|------|------|------|-----------------|
| `request_id` | string | 是 | 请求唯一标识，原样透传至响应 |
| `network` | string | 是 | 目标网络，枚举值：`eth` \| `bnb` \| `arb` \| `polygon`；决定签名私钥（EIP-191 无 chainId 概念） |
| `encoding` | string | 是 | 消息编码格式，枚举值：`"utf8"` \| `"hex"` |
| `message` | string | 是 | 要签名的消息；`"utf8"` 时为明文字符串，`"hex"` 时为 `0x` 前缀十六进制字节字符串 |

**encoding 解码规则：**

| encoding | 解码方式 | message 格式示例 | 解码失败行为 |
|----------|---------|----------------|------------|
| `"utf8"` | 直接取 UTF-8 字节 | `"Sign in to MyApp"` | 不会失败 |
| `"hex"` | **必须**以 `0x` 开头，再对后续字符十六进制解码 | `"0x48656c6c6f"` | 缺 `0x` 前缀或含非法字符均返回 400 `invalid_message` |

### Response

```json
{
  "request_id": "uuid-xxx",
  "network": "eth",
  "message_hash": "0x...",
  "signature": "0x...",
  "r": "0x...",
  "s": "0x...",
  "v": "0x1b"
}
```

| 字段 | 类型 | OpenAPI 注释要求 |
|------|------|-----------------|
| `request_id` | string | 透传自请求的唯一标识 |
| `network` | string | 透传自请求的网络标识 |
| `message_hash` | string | EIP-191 prefixed message 的 keccak256 哈希；**32 字节，hex 含 0x 前缀**（66 字符） |
| `signature` | string | 完整 ECDSA 签名；**65 字节，hex 含 0x 前缀**（r\|\|s\|\|v，132 字符） |
| `r` | string | 签名 r 分量；**32 字节，hex 含 0x 前缀**（66 字符） |
| `s` | string | 签名 s 分量；**32 字节，hex 含 0x 前缀**（66 字符） |
| `v` | string | recovery id；**1 字节，hex 含 0x 前缀**，`"0x1b"` 或 `"0x1c"` |

---

## 错误码

复用现有规范（详见 `Feature-Signing-API-Design-v2.0.md`），新增：

| HTTP 状态码 | error 字段 | 是否含 request_id | 触发场景 |
|------------|-----------|:-----------------:|---------|
| 400 | `invalid_encoding` | 是 | encoding 值不在支持列表 |
| 400 | `invalid_message` | 是 | message 字段解码失败（hex 格式非法、base64 非法等） |

现有错误码沿用：

| HTTP 状态码 | error 字段 | 触发场景 |
|------------|-----------|---------|
| 415 | `unsupported_media_type` | Content-Type 不是 application/json |
| 400 | `empty_body` | Body 为空 |
| 400 | `malformed_json` | JSON 语法错误 |
| 422 | `invalid_body` | JSON 类型不匹配 |
| 400 | `missing_request_id` | 未传 request_id |
| 400 | `invalid_network` | network 不在支持列表（EVM 接口） |
| 503 | `wallet_locked` | 钱包未解锁 |
| 404 | `wallet_not_found` | 该网络无对应钱包 |
| 500 | `sign_failed` | 签名过程内部错误 |

---

## Telegram 通知

### 新增 SignEvent 变体

```rust
SolanaMessage {
    /// request 中 message 字段原文（解码前），由 notification.rs 负责截断和 Markdown 转义
    message_preview: String,
    signed_at: String,
},
EvmMessage {
    network: String,
    /// EIP-191 prefixed keccak256 哈希（0x 前缀 hex）
    message_hash: String,
    /// request 中 message 字段原文（解码前），由 notification.rs 负责截断和 Markdown 转义
    message_preview: String,
    signed_at: String,
},
```

**关键约定：**
- Handler 层将 request 中的 `message` 字段**原始字符串**（解码前）传给 `SignEvent`
- 截断（60 字符）和 Markdown 转义**统一由 `notification.rs` 的 `format_sign_message` 处理**，handler 层不做任何截断

### 通知模板

```
[Solana 消息签名]
消息: <前 60 字符，超出截断 + "...">
时间: <UTC 时间>
```

```
[EVM 消息签名]
网络: <network>
Hash: `<message_hash>`
消息: <前 60 字符，超出截断 + "...">
时间: <UTC 时间>
```

---

## OpenAPI 字段注释规范

沿用现有约定（见 `Feature-Signing-API-Design-v2.0.md`），特别注意：

- `request_id` 字段：Rust 类型用 `Option<String>` + `#[serde(default)]`，OpenAPI 标注 `#[schema(required = true, value_type = String)]`，保证 JSON 缺字段时进入业务层再返回 `missing_request_id`（而非 422）
- `encoding` 字段：Rust 类型用 `String`（与 `network` 字段处理方式相同），在 handler 内 `request_id` 校验之后手动 match，非法值返回 400 `invalid_encoding`（含 `request_id`）；**不使用 serde 枚举**，避免在 `request_id` 之前就触发 422

---

## 代码层次划分

| 层 | 新增内容 |
|----|---------|
| `wallet-core/src/evm_wallet.rs` | `EvmMessageSignature` 结构体；`sign_message(private_key_bytes, msg_bytes)` 函数 |
| `wallet-core/src/solana_wallet.rs` | 无需改动（`sign_message` 已存在） |
| `wallet-core/src/notification.rs` | `SignEvent::SolanaMessage` / `SignEvent::EvmMessage` 变体及格式化分支 |
| `wallet-server/src/api/mod.rs` | 2 个 handler、2 条路由、OpenAPI schema |
| `README.md` | 新增两条接口说明 |
| `docs/simple_requirement.md` | 同步接口说明 |
