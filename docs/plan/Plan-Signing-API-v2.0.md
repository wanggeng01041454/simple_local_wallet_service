---
# 文档类型：开发计划
# 对应项目：Local Wallet Service
# 版本：v2.0
# 状态：已确认
# 替代旧文档：无
# 被新文档替代：无
# 生效时间：2026-03-11
---

# 签名接口重设计 — 实施计划

日期：2026-03-11
参考设计文档：`docs/design/Feature-Signing-API-Design-v2.0.md`

---

## 阅读须知

本文档用于指导 LLM 逐任务实现签名接口重设计。每个任务遵循 **TDD 原则**：先写测试（红），再写实现（绿），最后验证（通过）。

**不要跳过测试步骤，不要合并多个任务一起做，严格按顺序执行。**

---

## 项目结构速览

```
crates/
  wallet-core/src/
    wallet.rs          # WalletError, WalletManager, WalletKeys
    evm_wallet.rs      # EVM 密钥生成/导入
    solana_wallet.rs   # Solana 密钥生成/导入/签名
    notification.rs    # SignEvent, TelegramClient
    network.rs         # Network 枚举
    crypto.rs          # AES-GCM 加密
    lib.rs
  wallet-server/src/
    api/mod.rs         # HTTP handler + OpenAPI
    state.rs           # AppState
    server.rs          # axum 路由
    util.rs            # parse_network 等工具
    lib.rs
```

---

## 执行顺序

```
Task 0  → Cargo 依赖配置
Task 1  → WalletError 新增错误变体
Task 2  → SignEvent 枚举改造（通知层）
Task 3  → Solana sign_transaction（wallet-core）
Task 4  → EVM sign_transaction（wallet-core）
Task 5  → EIP-712 sign_typed_data（wallet-core）
Task 6  → 自定义 JSON Extractor（wallet-server）
Task 7  → Solana API handler
Task 8  → EVM transaction API handler
Task 9  → EIP-712 API handler
Task 10 → 删除旧接口 + 更新 OpenAPI
Task 11 → 文档更新
```

---

*(Full task details preserved from original document — Tasks 0 through 11)*

---

## 全量验证命令

所有任务完成后，执行以下命令确认无报错：

```bash
# 全量测试
cargo test

# 编译检查
cargo build

# 具体各层验证
cargo test -p wallet-core
cargo test -p wallet-server
```
