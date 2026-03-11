# 签名接口重设计计划

日期：2026-03-11

## 背景

对现有签名接口进行重构，拆分为三个专用接口，并统一引入 `request_id` 机制。
废弃原有 `POST /api/wallet/sign` 接口，不保留兼容。

---

## 变更概览

| 变更类型 | 接口 |
|---------|------|
| 废弃 | `POST /api/wallet/sign` |
| 新增 | `POST /api/wallet/sign/solana` |
| 新增 | `POST /api/wallet/sign/evm/transaction` |
| 新增 | `POST /api/wallet/sign/evm/typed-data` |

---

## 全局规则

### Request-ID

所有接口均通过 JSON Body 传递 `request_id`，服务原样透传至响应。

| 场景 | 行为 |
|------|------|
| 请求体中有 `request_id` | 原样透传到所有响应（含错误响应） |
| 请求体中缺少 `request_id` | 返回 400，`{"error": "missing_request_id"}`（此时无 request_id 可透传） |

- `request_id` **不提供幂等保护**：相同 `request_id` 发送两次，服务会执行两次签名。

### 错误响应格式

除 `missing_request_id` 外，所有错误响应均包含 `request_id` 字段：

```json
{
  "request_id": "uuid-xxx",
  "error": "invalid_transaction",
  "message": "..."
}
```

### OpenAPI 字段注释规范

所有 Request/Response Schema 中的字段，必须在 `utoipa` 的 `#[schema]` 属性中通过 `description` 注明：
1. **数据内容**：字段表示什么
2. **编码格式**：采用何种编码（hex / base58 / base64 / plain string）
3. **长度约束**：如果有固定长度，注明字节数和对应的字符数

示例写法（Rust）：
```rust
/// 签名后的完整 EVM 交易；RLP 编码后转 hex，含 0x 前缀
#[schema(example = "0x02f87...")]
transaction: String,
```

---

## 接口 1 — Solana 交易签名

```
POST /api/wallet/sign/solana
```

### Request Body

```json
{
  "request_id": "uuid-xxx",
  "encoding": "base64",
  "transaction": "<编码后的完整交易>"
}
```

| 字段 | 类型 | 必填 | OpenAPI 注释要求 |
|------|------|------|-----------------|
| request_id | string | 是 | 请求唯一标识，原样透传至响应 |
| encoding | string | 是 | 输入交易的编码格式，枚举值：`"base64"` 或 `"base58"` |
| transaction | string | 是 | 完整 Solana 交易，已含 recent_blockhash；编码格式由 encoding 字段指定 |

### Response

```json
{
  "request_id": "uuid-xxx",
  "transaction": "<base58 编码的签名完整交易>"
}
```

| 字段 | 类型 | OpenAPI 注释要求 |
|------|------|-----------------|
| request_id | string | 透传自请求的唯一标识 |
| transaction | string | 签名后的完整 Solana 交易；**固定 base58 编码**，与请求的 encoding 无关 |

### 行为说明

- 支持两种交易格式，自动识别并分别处理：
  - **Legacy `Transaction`**：使用 `solana_sdk::transaction::Transaction` 反序列化后签名
  - **`VersionedTransaction`**：使用 `solana_sdk::transaction::VersionedTransaction` 反序列化后签名
- `recent_blockhash` 由调用方在构造交易时填入，本服务不读取也不修改
- 多签交易：本服务只填充自己持有密钥对应的 signer 位置，其余 signer 位置保留原状
- 输出编码固定为 base58，与输入 encoding 无关

### Telegram 通知内容

```
[Solana 签名]
TxID: <签名后可计算的 TxID，如无则显示 N/A>
交易: <base58 编码的签名完整交易>
时间: <UTC 时间>
```

---

## 接口 2 — EVM 交易签名

```
POST /api/wallet/sign/evm/transaction
```

### Request Body

```json
{
  "request_id": "uuid-xxx",
  "network": "eth",
  "transaction": "0x02f8..."
}
```

| 字段 | 类型 | 必填 | OpenAPI 注释要求 |
|------|------|------|-----------------|
| request_id | string | 是 | 请求唯一标识，原样透传至响应 |
| network | string | 是 | 目标网络，枚举值：`eth` \| `bnb` \| `arb` \| `polygon`；决定签名所用私钥及 chainId |
| transaction | string | 是 | 未签名的完整 EVM 交易；RLP 编码后转 hex，**含 0x 前缀**；支持 Type 0 / 1 / 2 |

### Response

```json
{
  "request_id": "uuid-xxx",
  "network": "eth",
  "transaction": "0x02f8..."
}
```

| 字段 | 类型 | OpenAPI 注释要求 |
|------|------|-----------------|
| request_id | string | 透传自请求的唯一标识 |
| network | string | 透传自请求的网络标识 |
| transaction | string | 签名后的完整 EVM 交易；RLP 编码后转 hex，**含 0x 前缀** |

### 支持的交易类型

| Type | 标准 | 支持网络 |
|------|------|---------|
| Type 0 | Legacy | eth, bnb, arb, polygon |
| Type 1 | EIP-2930（access list） | eth, bnb, arb, polygon |
| Type 2 | EIP-1559（maxFeePerGas） | eth, bnb, arb, polygon |
| Type 3 | EIP-4844（blob 交易） | **不支持** |

### chainId 处理

`chainId` 以 `network` 参数为准，签名前强制将交易体中的 chainId 覆盖为下表对应值：

| network | chainId |
|---------|---------|
| eth | 1 |
| bnb | 56 |
| arb | 42161 |
| polygon | 137 |

### Telegram 通知内容

```
[EVM 签名]
网络: <network>
From: <from 地址>
To:   <to 地址>
Value: <value>
时间: <UTC 时间>
```

---

## 接口 3 — EIP-712 结构化数据签名

```
POST /api/wallet/sign/evm/typed-data
```

### Request Body

对齐 MetaMask `eth_signTypedData_v4` 格式。

```json
{
  "request_id": "uuid-xxx",
  "network": "eth",
  "typed_data": {
    "domain": {
      "name": "MyApp",
      "version": "1",
      "chainId": 1,
      "verifyingContract": "0x..."
    },
    "types": {
      "Transfer": [
        {"name": "to",    "type": "address"},
        {"name": "value", "type": "uint256"}
      ]
    },
    "primaryType": "Transfer",
    "message": {
      "to": "0x...",
      "value": "1000000"
    }
  }
}
```

| 字段 | 类型 | 必填 | OpenAPI 注释要求 |
|------|------|------|-----------------|
| request_id | string | 是 | 请求唯一标识，原样透传至响应 |
| network | string | 是 | 目标网络，枚举值：`eth` \| `bnb` \| `arb` \| `polygon`；决定签名私钥，并覆盖 domain.chainId |
| typed_data | object | 是 | 标准 EIP-712 结构（对齐 eth_signTypedData_v4）；包含 domain / types / primaryType / message 四个子字段 |
| typed_data.domain.chainId | integer | 否 | 签名前会被强制覆盖为 network 对应的 chainId |

### chainId 处理

签名前将 `typed_data.domain.chainId` 强制覆盖为与 `network` 参数一致的值（映射表同接口 2）。

### Response

```json
{
  "request_id": "uuid-xxx",
  "network": "eth",
  "signature": "0x...",
  "r": "0x...",
  "s": "0x...",
  "v": "0x1c"
}
```

| 字段 | 类型 | OpenAPI 注释要求 |
|------|------|-----------------|
| request_id | string | 透传自请求的唯一标识 |
| network | string | 透传自请求的网络标识 |
| signature | string | 完整 ECDSA 签名；**65 字节，hex 编码，含 0x 前缀**（= r \|\| s \|\| v，130 个 hex 字符 + "0x"） |
| r | string | 签名 r 分量；**32 字节，hex 编码，含 0x 前缀**（64 个 hex 字符 + "0x"） |
| s | string | 签名 s 分量；**32 字节，hex 编码，含 0x 前缀**（64 个 hex 字符 + "0x"） |
| v | string | 签名 v 分量（recovery id）；**1 字节，hex 编码，含 0x 前缀**（如 `"0x1b"` 或 `"0x1c"`） |

### Telegram 通知内容

```
[EIP-712 签名]
网络: <network>
数据: <typed_data 的 JSON 字符串>
时间: <UTC 时间>
```

---

## 依赖库变更

| 功能 | 当前状态 | 变更 |
|------|---------|------|
| EVM 交易 RLP 解析/签名 | alloy（仅用于签名哈希） | 启用 alloy `TxEnvelope` 完整 RLP 编解码 |
| EIP-712 运行时动态类型 | 无 | 引入 `alloy-dyn-abi` crate 处理任意 JSON 类型结构 |
| Solana Legacy 交易签名 | `sign_message` 直接签字节 | `solana_sdk::transaction::Transaction` 反序列化后签名再序列化 |
| Solana Versioned 交易签名 | 无 | `solana_sdk::transaction::VersionedTransaction` 反序列化后签名再序列化 |

---

## 错误码规范

| HTTP 状态码 | error 字段 | 触发场景 |
|------------|-----------|---------|
| 400 | `missing_request_id` | 未传 request_id（响应中无 request_id 字段） |
| 400 | `invalid_network` | network 不在支持列表 |
| 400 | `invalid_encoding` | encoding 不是 base64/base58 |
| 400 | `invalid_transaction` | 交易反序列化失败 |
| 400 | `unsupported_tx_type` | 使用了不支持的交易类型（如 Type 3） |
| 400 | `invalid_typed_data` | EIP-712 数据格式错误 |
| 503 | `wallet_locked` | 钱包未解锁 |
| 404 | `wallet_not_found` | 该网络无对应钱包 |
| 500 | `sign_failed` | 签名过程内部错误 |
