# local-wallet service

## 管理界面功能设计
管理界面通过本地 9292 端口提供服务

## REST API 接口设计
REST API 接口通过本地 9293 端口提供服务，支持以下接口：
1. `GET /api/wallet/address` - 获取钱包地址
2. `POST /api/wallet/sign` - 对交易进行签名
3. `GET /api/wallet/balance` - 获取钱包余额
   
## openapi文档接口
http://127.0.0.1:9293/api/docs/openapi.json