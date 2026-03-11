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

### 进入 Handler 前的请求解析错误

axum 的 `Json<T>` 提取器在以下情况会在进入业务逻辑前直接返回错误，此类错误**无法携带 request_id**：

| 场景 | HTTP 状态码 | 响应格式 |
|------|------------|---------|
| `Content-Type` 不是 `application/json` | 415 | `{"error": "unsupported_media_type"}` |
| Body 为空 | 400 | `{"error": "empty_body"}` |
| JSON 格式非法（语法错误） | 400 | `{"error": "malformed_json", "message": "<解析错误详情>"}` |
| JSON 结构合法但字段类型不匹配 | 422 | `{"error": "invalid_body", "message": "<字段错误详情>"}` |

实现方式：通过 axum 的 `FromRequest` 自定义提取器（对 `Json<T>` 包装），统一将上述错误格式化为上表约定的格式。这类错误响应中**不包含 request_id**，这是预期行为，需要在 OpenAPI 文档中注明。

**校验优先级**（从高到低）：
1. Content-Type / JSON 解析（axum 层，无 request_id）
2. `request_id` 字段存在性
3. `network` / `encoding` 等枚举字段合法性
4. 钱包锁定状态
5. 业务逻辑校验（交易格式、chainId 等）

### 错误响应格式

除解析阶段错误（无 request_id）和 `missing_request_id` 外，所有错误响应均包含 `request_id` 字段：

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

### Solana 多签失败场景的错误定义

| 场景 | HTTP 状态码 | error 字段 | 说明 |
|------|------------|-----------|------|
| 本地钱包 pubkey 不在交易的 required signers 中 | 400 | `signer_not_required` | 本服务不是该交易的签名方之一 |
| 交易已完整签名（所有 signer 位置均已填充） | 400 | `already_signed` | 无需重复签名 |
| 签名数组长度与 message 中声明的 signer 数量不一致 | 400 | `invalid_transaction` | 交易结构损坏 |

### Solana TxID 定义

TxID 固定取签名后交易的 `signatures[0]`（fee payer 签名槽）的 base58 值：

| 场景 | TxID 值 |
|------|---------|
| 本服务是 fee payer（`signatures[0]` 由本服务填入） | `signatures[0]` 的 base58 编码 |
| 本服务不是 fee payer（`signatures[0]` 签名后仍为空） | `null`，通知显示 N/A |

### Telegram 通知内容

```
[Solana 签名]
TxID: <signatures[0] 的 base58 值，若为空则显示 N/A>
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
| transaction | string | 是 | 未签名的完整 EVM 交易；RLP 编码后转 hex，**含 0x 前缀**；支持 Type 0 / 1 / 2；签名字段（v/r/s）必须为空或零值 |

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
| transaction | string | 签名后的完整 EVM 交易；RLP 编码后转 hex，**含 0x 前缀**；除签名字段（v/r/s）和 chainId 外，其余字段与输入完全一致 |

### 支持的交易类型

| Type | 标准 | 支持网络 |
|------|------|---------|
| Type 0 | Legacy | eth, bnb, arb, polygon |
| Type 1 | EIP-2930（access list） | eth, bnb, arb, polygon |
| Type 2 | EIP-1559（maxFeePerGas） | eth, bnb, arb, polygon |
| Type 3 | EIP-4844（blob 交易） | **不支持** |

### EVM 交易边界规则

| 场景 | 行为 |
|------|------|
| 输入交易已包含有效签名（v/r/s 非零） | 返回 400，`{"error": "already_signed"}` |
| Type 0 Legacy 交易，无 chainId 字段 | 注入 network 对应的 chainId，正常签名 |
| Type 1 / 2 交易，chainId 与 network 一致 | 正常签名 |
| Type 1 / 2 交易，chainId 与 network **不一致** | 返回 400，`{"error": "chain_id_mismatch"}` |
| 输入为 Type 3 交易 | 返回 400，`{"error": "unsupported_tx_type"}` |
| 返回的已签名交易 | 保证除 v/r/s 三个签名字段外，其余字段（nonce/gas/to/value/data/chainId 等）与输入完全一致 |

### chainId 映射

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

### EIP-712 JSON 结构规则

**types 字段：**
- `EIP712Domain` 类型：客户端**可以传也可以不传**；若传入，服务端忽略它（自行根据 domain 字段构造），不报错
- 嵌套 struct 类型：支持；在 `types` 中声明子类型，message 中按结构传入
- 数组类型（如 `address[]`、`uint256[]`）：支持

**字段值编码规则（JSON 中的类型映射）：**

| Solidity 类型 | JSON 中接受的格式 | 说明 |
|--------------|-----------------|------|
| `uint*/int*` | 十进制字符串或数字 | 建议字符串，避免大整数精度丢失 |
| `address` | `"0x"` 开头的 40 字符 hex 字符串 | 大小写不敏感，服务端统一 checksum |
| `bytes32` / `bytes*` | `"0x"` 开头的 hex 字符串 | 长度须与类型声明匹配 |
| `bool` | `true` / `false` | JSON 原生布尔值 |
| `string` | JSON 字符串 | 直接传值 |
| 数组 | JSON 数组 | 元素按上述规则递归处理 |
| 嵌套 struct | JSON 对象 | 字段按上述规则递归处理 |

**未知字段处理：**
- `typed_data` 顶层出现 `types` / `domain` / `primaryType` / `message` 以外的字段：**忽略**
- `message` 中出现 `primaryType` 声明以外的字段：返回 400，`{"error": "invalid_typed_data"}`
- `types` 中的类型定义与 `message` 实际字段不匹配：返回 400，`{"error": "invalid_typed_data"}`

### chainId 处理

| 场景 | 行为 |
|------|------|
| `domain.chainId` 缺失 | 注入 network 对应的 chainId，正常签名 |
| `domain.chainId` 与 network 一致 | 正常签名 |
| `domain.chainId` 与 network **不一致** | 返回 400，`{"error": "chain_id_mismatch"}` |

chainId 映射表同接口 2。

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

## Telegram 通知层改造

当前 `SignEvent` 结构体（`wallet-core/src/notification.rs`）只支持 `network / to / value / description / signed_at`，无法表达新接口所需的通知内容。

需将 `SignEvent` 改造为枚举，每种签名类型独立建模：

```rust
pub enum SignEvent {
    Solana {
        tx_id: Option<String>,   // signatures[0] 的 base58 值；本服务不是 fee payer 时为 None
        transaction: String,     // base58 编码的完整交易
        signed_at: String,
    },
    EvmTransaction {
        network: String,
        from: String,
        to: Option<String>,      // 合约创建交易无 to
        value: String,           // 十进制字符串，单位 wei
        signed_at: String,
    },
    EvmTypedData {
        network: String,
        typed_data_json: String, // typed_data 原始 JSON 字符串
        signed_at: String,
    },
}
```

对应的 `format_sign_message` 按枚举分支生成不同格式的通知文本。
`format_locked_message` 保持不变。

---

## 依赖库变更

| 功能 | 当前状态 | 变更 |
|------|---------|------|
| EVM 交易 RLP 解析/签名 | alloy（仅用于签名哈希） | 启用 alloy `TxEnvelope` 完整 RLP 编解码 |
| EIP-712 运行时动态类型 | 无 | 引入 `alloy-dyn-abi` crate 处理任意 JSON 类型结构 |
| Solana Legacy 交易签名 | `sign_message` 直接签字节 | `solana_sdk::transaction::Transaction` 反序列化后签名再序列化 |
| Solana Versioned 交易签名 | 无 | `solana_sdk::transaction::VersionedTransaction` 反序列化后签名再序列化 |
| axum 请求解析错误统一格式化 | 无 | 自定义 `FromRequest` 包装 `Json<T>`，统一返回约定格式 |

---

## 迁移清单

以下内容需与接口改造同步完成，缺一不可：

| 分类 | 具体内容 |
|------|---------|
| **路由** | `wallet-server/src/api/mod.rs` 中删除旧路由 `POST /api/wallet/sign`，注册三条新路由 |
| **OpenAPI** | `ApiDoc` 的 `paths` 和 `components(schemas)` 中删除旧 schema（`SignRequest` / `SignMetadata` / `SignResponse`），注册新的三组 schema |
| **通知层** | `wallet-core/src/notification.rs`：`SignEvent` 改为枚举，更新 `format_sign_message`，保留 `format_locked_message` |
| **wallet-core** | `solana_wallet.rs`：增加 `sign_transaction` 函数（Legacy + Versioned），保留或废弃旧 `sign_message` |
| **集成测试** | 删除 `api/mod.rs` 中所有针对旧 `/api/wallet/sign` 的测试；为三个新接口补充测试矩阵（见下表） |
| **README** | 更新接口列表和使用示例，删除旧接口相关描述 |
| **docs/simple_requirement.md** | 同步更新需求文档中的接口说明 |

### 新接口测试矩阵

| 接口 | 测试用例 |
|------|---------|
| Solana | 锁定返回 503；缺少 request_id 返回 400；无效 encoding 返回 400；Legacy 交易签名成功；VersionedTransaction 签名成功；signer_not_required 返回 400；already_signed 返回 400 |
| EVM Tx | 锁定返回 503；缺少 request_id 返回 400；无效 network 返回 400；Type 0/1/2 签名成功；已签名交易返回 400 already_signed；Type 3 返回 400 unsupported_tx_type；Type 0 无 chainId 时自动注入并成功；Type 1/2 chainId 不一致返回 400 chain_id_mismatch |
| EIP-712 | 锁定返回 503；缺少 request_id 返回 400；无效 network 返回 400；合法 typed_data 签名成功返回 r/s/v/signature；message 多余字段返回 400 invalid_typed_data；types 与 message 不匹配返回 400 invalid_typed_data；domain.chainId 缺失时自动注入并成功；domain.chainId 不一致返回 400 chain_id_mismatch |
| 通用 | malformed JSON 返回 400 无 request_id；空 body 返回 400 无 request_id；Content-Type 错误返回 415 无 request_id |

---

## 错误码规范

| HTTP 状态码 | error 字段 | 是否含 request_id | 触发场景 |
|------------|-----------|:-----------------:|---------|
| 415 | `unsupported_media_type` | 否 | Content-Type 不是 application/json |
| 400 | `empty_body` | 否 | Body 为空 |
| 400 | `malformed_json` | 否 | JSON 语法错误 |
| 422 | `invalid_body` | 否 | JSON 类型不匹配 |
| 400 | `missing_request_id` | 否 | 未传 request_id |
| 400 | `invalid_network` | 是 | network 不在支持列表 |
| 400 | `invalid_encoding` | 是 | encoding 不是 base64/base58 |
| 400 | `invalid_transaction` | 是 | 交易反序列化失败或结构损坏 |
| 400 | `already_signed` | 是 | 输入交易已包含有效签名 |
| 400 | `chain_id_mismatch` | 是 | 交易/domain 中显式的 chainId 与 network 参数不一致 |
| 400 | `unsupported_tx_type` | 是 | 使用了不支持的交易类型（如 Type 3） |
| 400 | `signer_not_required` | 是 | 本服务 pubkey 不在 Solana 交易的 required signers 中 |
| 400 | `invalid_typed_data` | 是 | EIP-712 数据格式错误或字段不匹配 |
| 503 | `wallet_locked` | 是 | 钱包未解锁 |
| 404 | `wallet_not_found` | 是 | 该网络无对应钱包 |
| 500 | `sign_failed` | 是 | 签名过程内部错误 |
