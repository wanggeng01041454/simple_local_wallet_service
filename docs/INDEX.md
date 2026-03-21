---
# 文档类型：索引
# 对应项目：Local Wallet Service
# 版本：v1.0
# 状态：已确认
# 替代旧文档：无
# 被新文档替代：无
# 生效时间：2026-03-21
---

# 文档索引

## 设计文档

| 文档 | 版本 | 状态 | 说明 |
|------|------|------|------|
| [System-Design-v1.1.md](design/System-Design-v1.1.md) | v1.1 | 已确认 | 需求概述 + 系统整体架构设计，同步 v2.0/v2.1 变更（Polygon、7 个签名接口、request_id、完整错误码、SignEvent 枚举） |
| [Feature-Signing-API-Design-v2.0.md](design/Feature-Signing-API-Design-v2.0.md) | v2.0 | 已确认 | 签名接口重设计：拆分为 Solana/EVM/EIP-712 三个专用接口，引入 request_id 机制（Breaking Change） |
| [Feature-Message-Signing-Design-v2.1.md](design/Feature-Message-Signing-Design-v2.1.md) | v2.1 | 已确认 | 任意消息签名接口：Solana Ed25519 + EVM EIP-191，兼容扩展 v2.0 |

## 开发计划

| 文档 | 版本 | 状态 | 对应设计 |
|------|------|------|---------|
| [Plan-System-v1.0.md](plan/Plan-System-v1.0.md) | v1.0 | 已确认 | System-Design-v1.1 |
| [Plan-Signing-API-v2.0.md](plan/Plan-Signing-API-v2.0.md) | v2.0 | 已确认 | Feature-Signing-API-Design-v2.0 |
| [Plan-Message-Signing-v2.1.md](plan/Plan-Message-Signing-v2.1.md) | v2.1 | 已确认 | Feature-Message-Signing-Design-v2.1 |

## 版本演进

```
v1.0  系统初始设计与实现（2026-03-04）
│  ├─ [Feature] v2.0  签名接口重设计 — Breaking Change（2026-03-11）
│  └─ [Feature] v2.1  任意消息签名 — 兼容扩展 v2.0（2026-03-15）
│
v1.1  系统设计同步更新 — 合并需求文档，同步 v2.0/v2.1 变更（2026-03-21）
```
