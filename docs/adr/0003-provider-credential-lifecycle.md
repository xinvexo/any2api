# ADR-0003: ProviderCredential API Key 生命周期

- 状态：Accepted
- 日期：2026-07-18
- 更新：2026-08-02
- 决策者：maintainer

## 背景

一个 Provider Endpoint 需要绑定多个独立 API Key，每个 Key 拥有自己的代理、可选 RPM 限制、启停和运行时健康代际。GatewayApiKey 与这些上游凭据完全独立。Credential 配置还必须在多标签页编辑、Secret 轮换、Endpoint 修改和热更新之间保持可验证的并发语义。

## 决策

- `ProviderCredential` 是独立实体，包含 Endpoint、标签、`api_key` 类型、代理绑定、可空的 `requests_per_minute`、启用状态及 `config_version`、`secret_version`、`credential_generation`。
- `requests_per_minute` 非空时限制为 `1..=100_000`；`NULL` 表示不做本地限速，禁用必须使用 `enabled=false`。
- 创建时四类版本均为 `1`。元数据修改只增加 `config_version`；重新启用额外增加 `credential_generation`；轮换增加 `config_version`、`secret_version` 和 `credential_generation`；无变化 PATCH 不增加任何版本。
- Credential 创建后不能改绑 Endpoint 或改变 CredentialKind。Endpoint 的 ProviderKind 创建后不可修改；接受协议和内部转换协议即使已有 Credential 也允许修改。Base URL 或协议变化增加所有子 Credential 的 generation，协议变化还在同一事务中重建物化 Route/Target 并裁剪失效的公开模型允许列表项。
- API Key 按 ADR-0074 明文保存在独立 SQLite Secret 字段中；`secret_version` 只承担轮换 CAS 和运行时代际语义。
- 指纹使用带稳定 Provider/Kind 域前缀的 SHA-256。SQLite 保存完整摘要，管理面只显示版本前缀加 64 位截断十六进制；长度至少 8 的可见 ASCII Key 可显示末 4 位。
- 服务端永不回显 Provider API Key。Web 创建或轮换成功后仅使用本次提交值显示组件内一次性回执；该值不进入 URL、React Query Cache、Mutation Cache、localStorage 或 sessionStorage。
- Credential 与 Endpoint 的 `enabled` 只控制数据面路由资格；管理面的 API Key 模型探测不要求两者启用，仍使用当前 Endpoint、Secret 和实际代理读取 `/models`。
- 元数据更新和 Secret 轮换是两个独立管理端点。创建检查全局 revision；更新检查 revision/config version；轮换检查 revision/config/secret version；删除检查 revision/config version。
- 删除 Proxy 时，如果仍有 Credential 引用则返回稳定冲突；Endpoint 删除的确认级联语义见 ADR-0122，禁止依赖原始 SQLite 外键错误作为管理契约。

## API 契约

- `GET/POST /api/admin/provider-endpoints/{endpoint_id}/credentials`
- `PATCH/DELETE /api/admin/provider-credentials/{credential_id}`
- `POST /api/admin/provider-credentials/{credential_id}/rotate-secret`

所有响应都只含脱敏 Credential 配置，并设置 `Cache-Control: no-store`。普通 PATCH DTO 不包含 Secret 字段且拒绝未知字段。

## 后果

Credential 配置可以在控制面完整管理。Runtime 按稳定 Credential ID 复用 RPM 窗口和 `in_flight` 观测句柄，并把 Storage 加载的 API Key 装入 generation-scoped 代际对象；`PublishedSnapshot` 只通过受控认证材料持有 Secret，不提供读取接口。认证头只能由调度器选中的请求通过 Provider Driver 生成。

## 验证

- Domain、Storage 和管理契约测试覆盖版本矩阵、RPM 范围与空值、重复标签、代理/Endpoint 引用、revision 冲突和无变化更新。
- Storage 测试覆盖明文 Secret 重启加载、轮换 CAS、指纹稳定性和读取 DTO 脱敏。
- HTTP/前端测试搜索响应、缓存、URL 和关闭后的 DOM，确保没有测试 API Key。
