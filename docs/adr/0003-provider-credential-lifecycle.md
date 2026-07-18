# ADR-0003: ProviderCredential API Key 生命周期

- 状态：Accepted
- 日期：2026-07-18
- 决策者：maintainer

## 背景

一个 Provider Endpoint 需要绑定多个独立 API Key，每个 Key 拥有自己的代理、最大并发、启停和运行时健康代际。GatewayApiKey 与这些上游凭据完全独立。Credential 配置还必须在多标签页编辑、Secret 轮换、Endpoint 修改和热更新之间保持可验证的并发语义。

## 决策

- `ProviderCredential` 是独立实体，包含 Endpoint、标签、`api_key` 类型、代理绑定、`max_concurrency`、启用状态及 `config_version`、`secret_schema_version`、`secret_version`、`credential_generation`。
- `max_concurrency` 首版硬限制为 `1..=10_000`；`0` 不表示禁用。
- 创建时四类版本均为 `1`。元数据修改只增加 `config_version`；重新启用额外增加 `credential_generation`；轮换增加 `config_version`、`secret_version` 和 `credential_generation`；无变化 PATCH 不增加任何版本。
- Credential 创建后不能改绑 Endpoint 或改变 CredentialKind。Endpoint 有 Credential 时禁止修改 ProviderKind/ProtocolDialect；Base URL 变化增加所有子 Credential 的 generation。
- API Key 使用 Secret Vault 加密。AAD 绑定 Credential ID、ProviderKind、CredentialKind、Secret Schema Version 和 Secret Version，防止跨记录、跨 Provider、跨类型和同记录旧版本回放。
- 指纹使用主密钥经域分离 HMAC-SHA256 派生的独立键，再对 Provider/Kind/Secret 计算 HMAC-SHA256。SQLite 保存完整 MAC，管理面只显示 `v1:` 加 64 位截断十六进制；长度至少 8 的可见 ASCII Key可显示末 4 位。
- 服务端永不回显 Provider API Key。Web 创建或轮换成功后仅使用本次提交值显示组件内一次性回执；该值不进入 URL、React Query Cache、Mutation Cache、localStorage 或 sessionStorage。
- 元数据更新和 Secret 轮换是两个独立管理端点。创建检查全局 revision；更新检查 revision/config version；轮换检查 revision/config/secret version；删除检查 revision/config version。
- 删除 Endpoint 或 Proxy 时，如果仍有 Credential 引用则返回稳定冲突；禁止依赖原始 SQLite 外键错误作为管理契约。

## API 契约

- `GET/POST /api/admin/provider-endpoints/{endpoint_id}/credentials`
- `PATCH/DELETE /api/admin/provider-credentials/{credential_id}`
- `POST /api/admin/provider-credentials/{credential_id}/rotate-secret`

所有响应都只含脱敏 Credential 配置，并设置 `Cache-Control: no-store`。普通 PATCH DTO 不包含 Secret 字段且拒绝未知字段。

## 后果

Credential 配置可以在当前控制面完整管理，但本切片不声称已经完成真实上游网络测试或负载调度。后续 Runtime 会按稳定 Credential ID 复用容量句柄，并把 Secret/generation 装入代际对象；当前 SQLite 和 PublishedSnapshot 只建立安全配置基础。

## 验证

- Domain、Storage 和管理契约测试覆盖版本矩阵、并发上限、重复标签、代理/Endpoint 引用、revision 冲突和无变化更新。
- Vault 测试覆盖 AAD Schema/Secret Version、防旧密文回放、指纹稳定性和错误密钥。
- HTTP/前端测试搜索响应、缓存、URL 和关闭后的 DOM，确保没有测试 API Key。
