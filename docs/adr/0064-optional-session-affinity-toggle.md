# ADR-0064: 普通显式 Session 的可选会话粘性开关

- 状态：Accepted
- 日期：2026-07-29
- 决策者：maintainer

## 背景

“绑定建立后必须固定目标”和“管理员是否希望普通显式 Session 参与粘性”是两个独立决策。普通显式
Session 标识可能由客户端通用配置自动携带；在管理员未明确选择时，不应让它改变正常候选池的负载
均衡。因此普通 Session 粘性默认关闭，需要固定路由时由管理员显式开启。

Codex `previous_response_id` 不只是负载均衡提示，而是对原 Credential 上游状态的续接引用。允许它在
关闭开关后改走其他目标会破坏会话正确性，也可能把账号专属状态错误地交给另一个 Credential。

## 决策

- SettingRegistry 提供 `affinity.enabled`，类型为 boolean，编译默认值为 `false`，支持覆盖、底层删除覆盖和
  PublishedSnapshot 热更新；Web 只允许写入具体覆盖值。
- 开关只作用于 ProtocolAdapter 标记为可首次创建的普通显式 Session。关闭时 Runtime 把这些标识按
  `IngressAffinity::None` 路径处理，不创建、命中或等待普通 Session 绑定。
- Continuation 不受开关影响；`previous_response_id` 始终必须命中原绑定，未命中返回
  `session_binding_lost`。
- 关闭开关不清空 `AffinityRegistry`。重新开启后，仍在统一 TTL 内的普通 Session 绑定可以继续命中；
  Response ID 绑定在关闭期间仍照常建立、刷新和清理。
- 不恢复 `soft` / `hard`、`prefer` / `strict`、绑定强度或可重绑模式。所有实际绑定继续使用 ADR-0062
  的同一种目标、TTL、等待和失败语义。
- Web 在“设置 → 路由策略”直接展示该开关；TTL 与等待超时仍属于高级设置。

## 后果

普通 Session 默认不影响路由；管理员显式开启后才建立固定绑定，同时不会削弱 Continuation 的安全
边界。开关热更新只改变新快照如何解释普通 Session，不需要持久化或迁移运行态绑定，也不引入第二套
Registry。

## 验证

- Domain 与管理 API 测试覆盖默认关闭、开启覆盖、持久化和恢复默认。
- Runtime/契约测试确认默认关闭时相同普通 Session 可选择不同 Credential，显式开启后固定目标。
- 契约测试确认关闭期间 `previous_response_id` 仍固定到原 Credential，丢失绑定仍返回
  `session_binding_lost`。
- Web 测试确认开关直接显示在路由策略页、可保存具体覆盖值且不存在恢复默认入口。
