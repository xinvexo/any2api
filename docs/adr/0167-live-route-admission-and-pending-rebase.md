# ADR-0167：实时路由准入撤销与 Pending 请求重规划

- 状态：Accepted
- 日期：2026-08-18
- 决策者：maintainer

## 背景

ADR-0075 让请求捕获的 `PublishedSnapshot`、`CredentialRuntimeBinding`、认证材料和策略在配置换代后保持
不可变，避免新配置反向污染已经开始的请求。但旧请求可能仍在收集 Body、等待 RPM/健康/Session、执行
Retry-After 或准备下一次 Attempt。配置发布只推进 scheduler epoch；等待者醒来后仍扫描旧候选，因此管理员
禁用或删除 Credential、OAuthAccount、Endpoint 后，旧请求仍可能首次取得 RPM 并向该路径发送数据面请求。
启用的新候选也不会被旧 Pending 请求观察到。

需要把“已经开始的 Attempt 可以收尾”和“仅仅进入 HTTP 服务的请求可以继续开始旧 Attempt”分开，同时
保持统一会话粘性、关闭粘性时的均衡路由、RetrySafety、RPM 与流提交边界不变。

## 决策

1. RuntimeRegistry 维护版本化 `RouteAdmission` activation。Activation 身份至少包含 RoutingCredential、
   routing generation、Endpoint config generation 和 EffectiveProxy config generation；状态只允许从
   `Active` 单向进入 `Revoked`。重新启用必须取得新的 activation incarnation，旧 Arc 永不重新激活。
2. 新 PublishedSnapshot 可以在切换前准备自己的 active activation；ArcSwap 后的无失败 reconcile 发布当前
   activation 集合并撤销其余集合，之后才推进 scheduler epoch 和返回管理 API 成功。
3. 候选扫描必须跳过已撤销 activation。RPM、健康和 Session lease 已取得但 Attempt 尚未开始时，仍在
   Transport handoff 前原子取得 `AttemptStartPermit`；失败必须精确回滚 RPM、`in_flight`、健康 Guard 和
   Creating lease，且不消耗上游 Attempt、Credential switch 或 RetrySafety 预算。
4. `AttemptStartPermit` 是禁用的线性化边界。Permit 在撤销前取得的 Attempt 属于已经开始，可以自然完成并
   持有旧认证、代理、连接池和流 Guard；撤销后不得再取得 Permit。普通禁用不取消已提交响应或长流。
5. 未绑定 Pending 请求在 Body 解码后、QueueTicket/config epoch 唤醒、Retry-After 返回和下一 Attempt 前可以
   加载最新 PublishedSnapshot 并重建候选。入口 revision 捕获的 QueuePolicy、AffinityPolicy、RetryPolicy、
   HeaderPolicy、超时和计价策略在逻辑请求内保持不变；只有路由候选、Endpoint、Credential、认证材料与代理
   投影切到新快照。重规划复用原 QueueTicket、绝对 deadline、Attempt/switch 计数、失败历史和 RetrySafety；
   入口认证以不含明文 Key 的证明在新快照重新验证，并重新检查 `models.allowed`。
6. 粘性语义不变：`affinity.enabled=true` 时，已经建立的 Session/Continuation 只能解析原 Credential、Route
   Target、上游模型和协议方言；目标撤销时保留绑定并返回 `session_binding_lost`，禁止改选。尚未取得
   `AttemptStartPermit` 的首次 Session 请求还没有建立绑定，可以回滚 Creating lease，并在入口捕获的同一
   Route 与方言内重新选择候选；只有 Attempt 实际开始后才保留其 lease 直到提交或 Drop。
   `affinity.enabled=false` 时普通显式 Session 继续视为无会话请求，重规划使用普通候选池，不创建、恢复或
   命中普通 Session 绑定。Continuation 始终保持必须续接。
7. OAuthAccount 的 `enabled` 继续只控制数据面资格。停用账号仍按 ADR-0048 执行 Token 保活；OAuthToken、
   OAuthQuota 和 Diagnostic 流量不经过数据面 RouteAdmission，管理界面必须区分这些 traffic class。
8. Credential/OAuth mutation 成功响应采用唯一的精简 ACK 契约：只返回已发布 revision 和当前快照中的安全
   核心配置/运行态字段，不包含 RequestLog usage 或 OAuth 模型目录。GET/list 继续返回完整展示数据，Web 在
   收到 ACK 后立即发布核心配置，并异步精确刷新 GET/list。mutation 不再接受或返回旧的完整列表响应，也不
   保留字段别名、双轨解析或兼容适配层。是否进一步引入增量配置编译由阶段计时和规模基准决定。

## 后果

- 管理禁用成功后，新数据面 Attempt 不再使用已撤销路径；此前已取得 Permit 的 Attempt 仍可能在日志中继续
  收尾，因此管理面需要展示 draining，而不能宣称所有网络活动已经终止。
- 配置换代不再等价于“所有旧请求可无限开始旧 Attempt”，但请求级 QueuePolicy、ReliabilityPolicy、已注册
  重试预算和已开始 Attempt 的完整快照仍保持不可变。
- `RouteAdmission` 是独立负向安全边界，不复用 auth error、健康 cooldown、熔断或 RPM；这些状态继续承担原职责。
- ADR-0075 关于旧 Binding 认证、RPM 和健康隔离的决定继续有效；其“旧 Binding 可继续准入”的部分由本 ADR
  限定为“已经取得 AttemptStartPermit 的 Attempt 可继续”。

## 验证

- 并发测试必须覆盖慢 Body、普通队列、固定 Credential 等待、RPM 预留后/Transport 前撤销、Retry-After、
  OAuth 401 refresh、Endpoint 级级联撤销和快速 disable→enable，证明旧 activation 永不复活。
- 管理 ACK 与并发 Attempt 的测试必须证明：撤销先于 StartPermit 时 Attempt 无法进入 Transport；StartPermit
  先于撤销时该 Attempt 可以自然完成。
- 撤销竞态中的 RPM reservation、`in_flight`、健康 Guard、QueueTicket 和 Creating lease 只能结算一次。
- 粘性开启时不得切换绑定目标；关闭时不得创建普通 Session 绑定。两种模式都必须保留现有 Header 投影、
  Transport isolation、fallback tier、RPM、健康和 RetrySafety 契约测试。
