# ADR-0035: 上游 API Key 与 OAuthAccount 本地调用统计

- 状态：Accepted
- 日期：2026-07-24
- 决策者：maintainer

## 背景

RequestLog 已同时保存客户端鉴权使用的 `gateway_api_key_id`，以及最终上游来源的互斥 `credential_id` / `oauth_account_id`；RequestAttempt 还保存每次重试选择。管理面目前只按 Gateway API Key 聚合调用统计，无法直接判断某把 Provider API Key 或某个 OAuthAccount 在日志保留窗口内实际承接了多少最终请求。

Gateway Key 与上游凭据回答不同问题。新增上游维度不能删除 Gateway Key 统计，也不能把两类 ID 绑定为所有权或路由关系。

## 决策

- 保留 ADR-0030 的 Gateway API Key 统计，不修改其 API、持久化或 Web 语义。
- 新增带来源标签的上游统计身份：`ProviderCredential(CredentialId)` 与 `OAuthAccount(OAuthAccountId)`。即使 UUID 字节相同，两者仍是不同统计对象。
- 统计直接读取最终 RequestLog：每个公开请求只归入最终上游目标一次，最终状态码为 2xx 计成功，其余计失败；每个对象返回总请求数、成功数、失败数与最近 24 次最终状态码。
- RequestAttempt 继续完整记录重试和切换，但中间 Attempt 不重复计入最终请求统计。需要诊断某次切换时使用现有 Attempt 时间线。
- 统计只覆盖当前 `logs.request.retention` / `logs.request.max_rows` 保留窗口。日志关闭、没有记录或查询失败时，Provider/OAuth 配置读取仍成功，并为当前对象返回零值与空结果。
- Provider API Key 列表和 OAuthAccount 列表分别显示自身统计；管理 DTO 只返回来源 ID、计数和状态码，不返回 API Key、Token、OAuth JSON、请求体或响应体。
- 统计只用于单机本地观测，不参与路由选择、Gateway Key 权限、并发、健康、额度、计费或启动恢复。

## 备选方案

- 用 RequestAttempt 数量作为“请求数”：拒绝。一次客户端请求在重试时会被放大，不符合现有 Gateway 统计口径；Attempt 已有独立时间线。
- 删除 Gateway Key 统计并改为上游统计：拒绝。入口使用情况与上游执行情况是两个独立维度。
- 建立永久累计计数表：拒绝。现有 RequestLog 已能提供有界、可清理的观测，永久计数会引入额外写入与恢复语义。
- 按裸 UUID 合并 ProviderCredential 与 OAuthAccount：拒绝。来源标签是统一路由身份的一部分，合并会造成身份碰撞和误导。

## 后果

管理读请求会增加一次有索引的 SQLite 聚合读取，公开数据面仍只通过既有有界遥测队列异步写 RequestLog。旧日志会随保留策略删除，因此统计不是永久账单。删除上游对象后外键置空的历史记录不再归入该对象，与现有 Gateway Key 删除语义一致。

## 验证

- Storage 测试覆盖 API Key/OAuth 来源隔离、相同 UUID 不碰撞、2xx 分类、最近结果顺序和无来源日志。
- 管理契约测试覆盖 Provider API Key 与 OAuthAccount 列表统计、查询降级和响应不含 Secret/Token。
- React 契约与组件测试覆盖两类上游统计、零值、最近状态及 Gateway Key 统计继续存在。
