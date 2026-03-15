# 本地钱包服务需求描述

## 需求概述
我们要实现一个本地钱包服务，它在安装后成为系统的独立服务. 服务同时提供一个基于网页的管理界面和 REST API 接口。

### 钱包服务功能描述
1. 支持 solana 网络;
2. 支持 EVM 兼容网络, 包括 Eth/BNB/Arb;
3. 每个网络只维护1个钱包;
4. 钱包私钥被加密后存放在文件中;
5. 每次服务重启后, 都需要用户通过管理界面输入密码来解密钱包私钥;
6. 私钥解密后, 常驻在内存中;
7. 外部应用可以通过REST API 接口来查询私钥地址和使用私钥对交易进行签名;
8. 服务每次对交易进行签名时， 都通过一个 telegram bot 发送通知到管理员的 telegram 账号，通知内容包括交易的详细信息和签名时间。

### 服务实现设计
服务支持以下系统的安装
- Windows
- Linux(systemd)
- Macos
- Android Termux

为服务安装生成脚本和安装说明文件.

### 管理界面功能设计
管理界面通过本地 9292 端口提供服务，支持以下功能：
1. 设置钱包密码
2. 导入钱包私钥
3. 创建钱包私钥(1个网络只能有1个私钥存在)
4. 查看钱包地址
5. 查看钱包余额
6. 转账

### REST API 接口设计
REST API 接口通过本地 9293 端口提供服务，支持以下接口：
1. `GET /api/wallet/address` - 获取钱包地址
2. `GET /api/wallet/balance` - 获取钱包余额
3. `POST /api/wallet/sign/solana` - Solana 交易签名（支持 base64/base58 输入，返回 base58 编码的已签名交易）
4. `POST /api/wallet/sign/evm/transaction` - EVM 交易签名（支持 Legacy/EIP-2930/EIP-1559，输入输出均为 hex 编码）
5. `POST /api/wallet/sign/evm/typed-data` - EIP-712 结构化数据签名（返回完整签名及 r/s/v 分量）
6. `POST /api/wallet/sign/solana/message` - Solana 任意消息签名（Ed25519，原始字节，无前缀）
7. `POST /api/wallet/sign/evm/message` - EVM 任意消息签名（EIP-191 personal_sign，keccak256 哈希后 ECDSA）

> **Breaking Change (v2.0.0):** `POST /api/wallet/sign` 已移除，请迁移到上述五个专用签名接口。

所有签名接口均需提供 `request_id`（必填）和 `network`（Solana 交易签名和 Solana 消息签名接口除外）字段，并要求钱包处于已解锁状态。


### 钱包服务实现代码
- 使用 rust 语言

### 管理页面实现代码和框架
- 使用 react 框架 
- 使用 typescript 语言
