# ADR-0006: Gateway API Key 管理与公开入口认证边界

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer

## 背景

any2api 需要允许同一个个人实例创建多个客户端访问凭据。它们只代表实例级访问，不代表用户、租户、额度或 ProviderCredential 绑定。现有仓库只有 `GatewayApiKeyId`，没有持久化、运行时快照或公开入口认证实现。

## 决策

- 网关 Key 由服务端生成，格式为 `a2k_v1_` 加 256 位随机值的 URL-safe Base64，无需用户提交 Secret。
- SQLite 保存名称、完整明文 token、前缀、版本、启用状态和无密钥 SHA-256 摘要。摘要使用 Gateway 专用稳定域前缀，不能复用 Provider Credential 指纹域。
- 管理列表、创建和轮换响应始终可以查看明文；响应使用 `Cache-Control: no-store`，Web 只在当前内存查询缓存中消费，不写入 URL、日志、浏览器持久化或导出文件。
- 创建和轮换请求不接受客户端 token。删除执行物理删除，不保留撤销状态或第二套生命周期。
- `GatewayApiKeyConfiguration` 与摘要验证材料随 `StoredConfiguration` 进入 `PublishedSnapshot`。管理写入、快照切换和公开鉴权使用同一 revision，已开始请求继续持有其捕获快照。
- 公开入口首版接受 `Authorization: Bearer <token>` 或 `x-api-key: <token>`。多个头携带不同 Token 时拒绝；认证成功后剥离客户端认证头、Cookie 和 `Proxy-Authorization`，仅通过请求扩展传递 Key ID。
- 管理 API 使用独立的单管理员认证；Gateway Key 不能登录管理面。
- 认证门只负责验证 Gateway Key、剥离客户端凭据并传递 Key ID；协议转换、Provider Driver 和 Transport 执行始终位于各自模块。

## 管理 API

```text
GET  /api/admin/gateway-api-keys
POST /api/admin/gateway-api-keys
PATCH /api/admin/gateway-api-keys/{id}
POST /api/admin/gateway-api-keys/{id}/rotate
DELETE /api/admin/gateway-api-keys/{id}?expected_revision=...&expected_config_version=...
```

写请求使用全局 `expected_revision`，并对单 Key 操作校验 `expected_config_version`；轮换额外校验 `expected_token_version`。所有响应使用 `Cache-Control: no-store`。

## 后果

- 数据库泄露会暴露 Gateway Key、Provider Secret 与代理密码明文，这是 ADR-0074 明确接受的本地部署信任边界。
- 公开入口认证和 Secret 隔离契约不依赖具体的 Provider 或协议 Adapter。
- 轮换会立即使被替换的 Token 失效；已开始的请求因持有其捕获快照仍可完成，新请求只能使用当前快照。
- 公开协议路由与 SPA fallback 保持严格分离，未知 `/v1/*` 返回公开 API 404，不回落管理页面。
