# ADR-0075: Revision-scoped Runtime Binding 与配置发布隔离

- 状态：Accepted
- 日期：2026-07-31
- 修订：2026-08-03
- 决策者：maintainer

## 背景

`PublishedSnapshot` 通过 `ArcSwap` 切换，但旧实现会在构造下一快照时修改共享 `CredentialRuntimeHandle` 的 RPM 限额、当前认证代际和 retired 状态，并全局裁剪 Affinity。于是仍持有旧快照的请求会在指针切换前观察到新配置，甚至在上游成功后因新 revision 删除 Credential 而无法提交绑定。

## 决策

- `CredentialRuntimeHandle` 只持有必须跨配置 revision 复用的滚动请求时间戳、`in_flight`、固定等待者和观测计数。
- `CredentialRuntimeBinding` 按 PublishedSnapshot 固化 `requests_per_minute` 与 `CredentialGenerationRuntime`。RPM 预留在同一个 Handle 锁内使用调用 Binding 的限额判断，因此旧、新 revision 共享时间戳但不会互相改写限额。
- PublishedSnapshot 仅由有序 `RoutingCredential` 投影持有每个 `CredentialRuntimeBinding`。全量 Runtime 视图从该投影按同一顺序提供借用迭代器，不克隆 `Arc` 建立第二个 Binding 向量。
- 新快照准备阶段可以复用或创建内部 Handle、Generation、Endpoint/Proxy Health 和 tier cursor，但不得修改任何已发布 Binding 的准入策略、认证材料或健康代际。
- Credential 是否可路由由当前 PublishedSnapshot 中是否存在候选决定，不在共享 Handle 上维护全局 `retired` 开关。
- 配置发布不全局裁剪 Affinity，也不阻止已经持有旧快照的请求提交绑定。新 revision 解析绑定时必须用自己的 Route/Credential 候选验证固定目标；目标已不存在则返回 `session_binding_lost`，禁止重新选择。TTL、管理员显式清理和进程重启仍可物理删除绑定。
- 新快照完成单次 ArcSwap 后才推进 scheduler epoch。Registry 索引可以在不影响旧 Arc 所有者的前提下清理；旧请求持有的 Handle、Generation 与 Health Runtime 由引用计数自然回收。

## 后果

- 热更新 RPM 时，旧请求继续使用旧限额，新请求使用新限额；两者共同计入同一滚动 60 秒窗口。
- 删除或轮换 Credential 不再反向改变已经开始请求的认证、准入或绑定提交结果。
- 删除 Credential 后保留到 TTL 的绑定可能在新 revision 中命中后返回 `session_binding_lost`；这符合绑定不可降级、不可重新选择的语义。

## 验证

- Snapshot 测试必须持有旧 Binding，在新 revision 发布后验证旧 RPM 限额和 Generation 未变化，新 Binding 复用窗口但采用新限额。
- Snapshot 的 Runtime Binding 迭代视图必须与有序 Credential 投影数量、顺序和引用身份一致，不得引入第二份所有集合。
- Affinity 测试必须覆盖配置变化期间的旧请求仍可完成 Session 与 Continuation 绑定提交。
- 现有并发 RPM 测试继续证明同一 Handle 上的原子选择与预留不会超过调用 Binding 的限额。
