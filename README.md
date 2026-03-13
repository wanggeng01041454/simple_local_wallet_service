# local-wallet service

## 管理界面功能设计
管理界面通过本地 9292 端口提供服务

## REST API 接口设计
REST API 接口通过本地 9293 端口提供服务，支持以下接口：
1. `GET /api/wallet/address` - 获取钱包地址
2. `GET /api/wallet/balance` - 获取钱包余额
3. `POST /api/wallet/sign/solana` - Solana 交易签名
4. `POST /api/wallet/sign/evm/transaction` - EVM 交易签名（Legacy/EIP-2930/EIP-1559）
5. `POST /api/wallet/sign/evm/typed-data` - EIP-712 结构化数据签名

> **Breaking Change (v2.0.0):** `POST /api/wallet/sign` 已移除。
> 请迁移到以上三个新签名接口。

所有签名接口均要求 `request_id` 字段（必填），且钱包处于已解锁状态。
解析错误（如 JSON 格式错误）的响应不含 `request_id`，这是预期行为。

### 签名接口示例

```bash
# Solana 签名
curl -X POST http://localhost:9293/api/wallet/sign/solana \
  -H "Content-Type: application/json" \
  -d '{"request_id":"req-1","encoding":"base64","transaction":"<base64 tx>"}'

# EVM 交易签名
curl -X POST http://localhost:9293/api/wallet/sign/evm/transaction \
  -H "Content-Type: application/json" \
  -d '{"request_id":"req-1","network":"eth","transaction":"0x02f8..."}'

# EIP-712 签名
curl -X POST http://localhost:9293/api/wallet/sign/evm/typed-data \
  -H "Content-Type: application/json" \
  -d '{"request_id":"req-1","network":"eth","typed_data":{"domain":{...},"types":{...},"primaryType":"...","message":{...}}}'
```

## OpenAPI 文档接口
http://127.0.0.1:9293/api/docs/openapi.json