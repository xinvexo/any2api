# ADR-0125：客户端身份 Header 的重放与账号归属

- 状态：Accepted
- 日期：2026-08-10
- 决策者：maintainer
- 相关决策：ADR-0149

## 背景

同方言直通会投影一组客户端身份、会话和追踪 Header。它们并非都属于某个上游账号：
会话前缀需要在均衡模式的换号重试中保持，attestation 或 turn-state 等值则由上游
按账号签发，不能跨账号发送。Runtime 必须让 Driver 按统一 ownership 分类投影，而不是
在中央调度器中猜测 Header 含义。

## 决策

1. Provider Driver 为每个精确 Header 名称或受控前缀声明 `Replayable`、`SessionScoped`、
   `CredentialOwned` 或 `BoundTurnState`。Provider 固定缺省值在客户端投影之外构造，不受
   该过滤器影响；未声明的客户端 Header 不进入上游请求。
2. `Replayable` 只包含不携带账号或会话关联值的协议能力与通用 persona 字段，可在任何
   Attempt 重放。跨协议 Bridge 不投影源方言 Header。
3. `SessionScoped` 包含协议允许的设备、会话、请求关联和分布式追踪值，包括 installation、
   session、thread、window、conversation、agent、request ID、`traceparent`、`tracestate`
   和 Claude `x-stainless-*` 前缀。
4. `CredentialOwned` 只用于上游为特定账号签发或校验的值，例如 attestation、usage-limit
   证明和账号绑定 Header。`BoundTurnState` 还要求当前请求命中同一 Credential 的既有会话
   绑定；首次建立会话或绑定丢失时必须删除。当前 `x-codex-turn-state` 属于此类。
5. Runtime 根据快照的 `affinity.enabled` 和 Attempt owner 投影：均衡模式换 Credential
   时保留 `Replayable`/`SessionScoped`、删除 `CredentialOwned`/`BoundTurnState`；粘性模式
   换号时删除 `SessionScoped` 以及账号归属值。相同 Credential 的安全重试继续保留允许值。
   Gateway Key、Provider 认证和连接级 Header 始终由网关重建，不从客户端透传。
6. ownership 只使用通用 `ProviderRequestContext`；不随机化、伪造、缓存、持久化或记录
   原始标识值。新增 Provider Header 必须先注册分类和契约测试。

## 后果

- 均衡模式保持 prompt cache 所需的可重放请求面，同时不跨账号发送上游签发的绑定值。
- 粘性模式的账号隔离由会话绑定与 Header ownership 同时保证。
- Header 分类集中在 Provider Driver，Runtime 不增加 Codex/Claude/Grok 分支。

## 验证

- Header policy 测试覆盖精确名称、前缀、四类 ownership 和未声明值拒绝。
- Codex、Claude、Grok 契约覆盖均衡/粘性两种模式、同 Credential 重试、换 Credential
  重试及 `x-codex-turn-state` 的绑定条件。
- 公开请求测试确认 Gateway Key、Authorization、Cookie 和 hop-by-hop Header 永不透传。
