# ADR-0125: 客户端身份 Header 的 Credential ownership

- 状态：Accepted
- 日期：2026-08-10
- 决策者：maintainer

## 背景

同方言直通会按 Provider 白名单保留客户端 Header。其中不仅有协议能力标志，还有 Codex installation/session/thread、Claude session/container/agent、Grok conversation/session/agent、client request ID 与分布式追踪值。当首次 Attempt 在提交前安全失败并重选另一 Credential 时，旧实现除 `x-oai-attestation` 和 `x-codex-turn-state` 外仍会投影这些值。因此同一设备、会话或追踪标识可以与两个不同上游 Authorization 同时出现。

Runtime 已在每个逻辑请求内锁定首次选中的 `RoutingCredentialId`，并向 Driver 传递 `allow_credential_bound` 和 `allow_turn_state` 事实。缺口是各 Provider 的白名单没有完整声明并执行 ownership。

## 决策

1. Provider 请求 Header 的每个精确名称必须在 Driver 内声明为 `Replayable`、`CredentialOwned` 或 `BoundTurnState`。Provider 前缀规则也必须有 ownership，不得作为绕过精确名称分类的通道。
2. `Replayable` 只用于无单次请求、会话、设备或账号关联值的协议能力和通用 persona 字段。Provider 固定缺省值在客户端投影之外构造，不受 ownership 过滤影响。
3. `CredentialOwned` 覆盖客户端传入的 installation、session、conversation、thread、window、turn metadata、container、agent、request ID、attestation、`traceparent`/`tracestate` 以及可能携带同类关联值的 Provider 前缀。Claude 的客户端 `x-stainless-*` 前缀整体归入此类，避免未来新名称绕过 owner 边界。
4. Runtime 在首次成功选择 Attempt 候选后锁定 owner，之后不改写。同 Credential 重试仍允许 `CredentialOwned`；任何重选 Credential 的 Attempt 必须删除它们。不 fail-closed 整次请求，因为这些 Header 不是已支持公开协议的必填字段；删除是最短且可验证的安全路径。
5. `BoundTurnState` 同时要求 Credential owner 一致且命中已有会话绑定；当前只有 `x-codex-turn-state`。首次建立会话时仍必须删除。
6. 跨协议桥继续不投影源方言 Header。不随机化、伪造、缓存、持久化或记录原始标识值。
7. ownership 选择只使用通用 `ProviderRequestContext`；Runtime 不得为 Codex、Claude 或 Grok 增加分支。实现复用现有布尔事实，不新增与当前风险不成比例的跨层状态机。

## 后果

- 首个 Attempt 仍保留同方言客户端的原始有界标识，直通 fidelity 不变。
- 同 Credential 的安全重试仍保留这些值；换 Credential 的预提交 failover 只保留可重放协议字段和 Provider 缺省身份。
- 同一逻辑请求的两个 Authorization 不再共享客户端 installation/session/request/trace 关联值。
- 新增 Provider Header 时必须选择 ownership，缺少声明时不能进入白名单。

## 验证

- Header policy 单元测试覆盖精确名称和前缀 ownership，包括禁止被前缀规则重新引入。
- Codex、Claude 和 Grok Driver 测试枚举各自的 Credential-owned 值，断言 owner 命中时保留、owner 切换时删除，可重放字段和 Provider 缺省值不受影响。
- 公开请求契约使用两个 API Key 和一次可安全重选的 401，直接观测第一个上游 Attempt 携带 installation/request/trace 值，第二个不携带，且两者 Authorization 不同。
