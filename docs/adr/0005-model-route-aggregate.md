# ADR-0005: 模型路由聚合与稳定 Target 身份

- 状态：Superseded by ADR-0028（内部物化规则仍然保留）
- 日期：2026-07-18
- 决策者：maintainer

## 背景

首版需要把客户端可见模型精确映射到一个或多个同协议 Provider Endpoint。Route 与 Target 如果通过多次独立管理请求保存，会产生只有 Route、没有 Target，或只保存部分 Target 的中间配置；Target ID 同时还会成为后续硬粘性的稳定身份。

## 决策

- `ModelRoute` 是配置与管理聚合根。管理 API 一次提交 Route 元数据和完整 Target 集合，SQLite 在同一个 `BEGIN IMMEDIATE` 事务中完成校验、写入、revision 递增和重载断言。
- Route 的 `config_version` 覆盖 Route 元数据和 Target 集合的任意变化。Target 首版不提供独立管理版本，避免同一个聚合出现多套并发控制语义。
- Target ID 由服务端生成并稳定保留。更新已有 Target 时只允许改变 `fallback_tier` 和 `enabled`；改变 Endpoint 或 `upstream_model` 必须创建新 Target ID，防止旧硬绑定被静默解释为另一上游身份。
- 创建和更新请求中的新 Target 不带 ID；已有 Target 必须携带当前 ID。未知 ID、跨 Route 复用 ID和重复 Target 都在提交前拒绝。
- `public_model` 在同一入口协议内精确、区分大小写唯一；`public_model` 与 `upstream_model` 最长 255 个 Unicode 字符。首版不实现 wildcard、前缀匹配、别名链或 Route 图，因此不存在运行时递归解析和循环检测。
- `fallback_tier=0` 是主 tier，数值越大优先级越低，tier 可以不连续。Route 的可空 `fallback_on_saturation` 表示继承全局设置、明确不溢出或明确进入下一 tier。
- 首版只允许 `openai_responses -> openai_responses` 和 `anthropic_messages -> anthropic_messages`。Target 引用 Endpoint 而不是 Credential；同 Endpoint 下的 Credential 继续由 Runtime 按容量和健康状态选择。
- 每条 Route 至少有一个 Target；启用 Route 至少有一个启用 Target。禁用 Endpoint 或暂时没有可用 Credential 不删除 Route，也不改变持久化映射。

## 管理 API

本 ADR 中的手工管理 API 已由 ADR-0028 移除；以下列表仅保留为历史决策记录，当前不再注册这些路由。

- `GET /api/admin/model-routes`
- `POST /api/admin/model-routes`
- `PATCH /api/admin/model-routes/{id}`
- `DELETE /api/admin/model-routes/{id}?expected_revision=...&expected_config_version=...`

响应返回当前 `config_revision` 和嵌套 Target 的完整 Route 列表。所有写请求继续使用全局 revision；更新和删除同时校验 Route 聚合版本。

## 后果

- Web 可以用一个编辑器原子保存完整映射，不需要编排多次请求或显示半成品 Route。
- Route Target 身份可直接供后续会话粘性、Attempt 日志和 fallback 调度使用。
- 修改单个 Target 也会增加父 Route 版本；个人单管理员场景下，这个较粗粒度 CAS 换来更简单且一致的冲突处理。
- 后续若需要批量操作或 Target 专用端点，可以复用同一聚合事务，不改变领域身份规则。
