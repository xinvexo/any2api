# ADR-0006: Gateway API Key 管理与公开入口认证边界

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer

## 背景

any2api 需要允许同一个个人实例创建多个客户端访问凭据。它们只代表实例级访问，不代表用户、租户、额度或 ProviderCredential 绑定。现有仓库只有 `GatewayApiKeyId`，没有持久化、运行时快照或公开入口认证实现。

## 决策

- 网关 Key 由服务端生成，格式为 `a2k_v1_` 加 256 位随机值的 URL-safe Base64，无需用户提交 Secret。
- SQLite 只保存名称、前缀、版本、启用/撤销状态和摘要；摘要是由 Secret Vault 主密钥派生的独立 HMAC-SHA256。Provider Credential 的指纹域不可复用。
- 创建和轮换成功时返回一次明文；明文不进入普通配置 DTO、React Query cache、Mutation cache、URL、日志或数据库。撤销记录保留，撤销后不能重新启用或轮换。
- `GatewayApiKeyConfiguration` 与摘要验证材料随 `StoredConfiguration` 进入 `PublishedSnapshot`。管理写入、快照切换和公开鉴权使用同一 revision，旧请求继续持有旧快照。
- 公开入口首版接受 `Authorization: Bearer <token>` 或 `x-api-key: <token>`。多个头携带不同 Token 时拒绝；认证成功后剥离客户端认证头、Cookie 和 `Proxy-Authorization`，仅通过请求扩展传递 Key ID。
- 管理 API 仍沿用当前 loopback-only 门禁；Gateway Key 不能登录管理面。远程管理员认证属于后续切片。
- 本 ADR 只实现认证门和明确的 `public_api_not_implemented` 占位响应，不把协议转换、Provider Driver 或 Transport 执行提前塞入鉴权模块。

## 管理 API

```text
GET  /api/admin/gateway-api-keys
POST /api/admin/gateway-api-keys
PATCH /api/admin/gateway-api-keys/{id}
POST /api/admin/gateway-api-keys/{id}/rotate
POST /api/admin/gateway-api-keys/{id}/revoke
```

写请求使用全局 `expected_revision`，并对单 Key 操作校验 `expected_config_version`；轮换额外校验 `expected_token_version`。所有响应使用 `Cache-Control: no-store`。

## 后果

- 数据库泄露时不能直接得到网关明文；主密钥缺失或不匹配会在启动时失败。
- 公开入口可以先固定认证和 Secret 隔离契约，后续增加 Codex/Claude Adapter 不需要改动 Key 存储或管理页面。
- 轮换会立即使旧 Token 失效，旧请求因持有旧快照仍可完成；新请求只能使用新快照。
- 由于尚未实现完整公开协议，合法 Token 在当前切片会收到明确的未实现响应，而不是被 SPA fallback 吞掉。
