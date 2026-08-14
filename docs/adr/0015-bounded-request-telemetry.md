# ADR-0015: 有界请求遥测、Attempt 历史与查询边界

- 状态：Accepted
- 日期：2026-07-20
- 修订：2026-08-14（分页由 ADR-0107 修订；鉴权拒绝队列隔离由 ADR-0109 修订；在途所有权字节边界由 ADR-0114 修订）
- 决策者：maintainer

## 背景

数据面已经具备同协议 JSON/SSE、可靠性分类和提交前多 Attempt，但 Attempt 结果只用于进程内健康状态，没有形成可查询的 RequestLog 历史。日志写入不能阻塞请求、延长 GuardedBody Drop、争抢配置事务，也不能被误用来恢复并发、队列、会话或熔断状态。

## 决策

- Server 在 `/v1` 鉴权层之外生成本地 `RequestId`，始终写入响应 `x-any2api-request-id`，并把同一个 ID 传入 Runtime。最终上游 Attempt 的安全 `x-request-id` 可以保留；缺失时才用本地 ID 补齐 `x-request-id`。
- RequestLog 持久化已通过 GatewayApiKey 鉴权并进入模型执行链的请求，包括解码、规划、排队和上游执行错误。鉴权失败、未知公开路由和方法错误只返回本地 Request ID 并写 HttpAccessLog，不写 RequestLog。
- Runtime 使用每请求 `RequestRecorder` 和每次上游执行 `AttemptRecorder`。Attempt 在健康状态结算之后、运行态 Guard 结算之前完成；正常 JSON、错误、超时、取消和流式 Drop 都只能完成一次。
- Request 与全部 Attempt 先在当前请求内存中聚合。请求结束时只执行一次同步 `try_send`，把完整聚合记录放入有界队列；队列满、Writer 已关闭或 SQLite 写入失败时丢弃该条遥测并增加计数，禁止等待、重试或反压数据面。
- 队列容量和记录指标使用两套同源但不同语义的计数：内部 queue slot 维持有界准入；公开 `queued_records` 只统计仍在 channel 中的数据记录，Writer 接收时把它们转入 `in_flight_records`，存储成功或失败后再分别结算到累计 `persisted_records` 或 `dropped_records`。Gateway 鉴权前拒绝的 HttpAccessLog 另有不超过逻辑 queue slot 四分之一的低优先级子计数，避免廉价未认证流量占满全部槽；它仍使用同一 channel、Writer 与总体指标，完整边界由 ADR-0109 定义。管理清理等控制消息占用 queue slot 但记录数为零，不污染四项记录指标。
- 指标按被接受的数据记录计数，不按实际 SQLite 语句数计数；因此 Gateway Key `last_used_at` 在同一批次内按 Key 折叠后，成功或失败仍按折叠前记录数结算。Writer 正常退出、任务失败或超时 abort 并完成 join 后，任何尚未获得存储终态的 queued 与 in-flight 记录都转入 dropped，两个瞬时计数归零。
- SSE 在首帧验证和会话绑定提交成功后才把请求最终完成权交给 `GuardedBody`。正常 EOF、提交后错误和客户端 Drop 由 GuardedBody 记录最终结果；首帧或提交前失败仍由普通请求路径完成 RequestLog。
- Runtime 后台 Writer 从有界队列按最多 64 条的小批次读取，在一个 SQLite 事务中先写父 RequestLog、再写 RequestAttempt，并使用写入时最新 PublishedSnapshot 的 `logs.request.max_rows`。同一事务插入后按 `(started_at_ms, request_id)` 删除最旧父记录，每笔事务最多删除集中定义的 10,000 行；稳定状态下新增量小于该预算，所以任何持续写入速率都不能让行数上限净增长。
- 配置下调或历史积压超过单笔删除预算时，Storage 的写入/清理结果返回 `has_more`。Writer 使用独立清理唤醒在普通事件处理之间继续有界事务，直到收敛；配置发布更新清理策略后立即触发同一唤醒，不等待下一次公开请求或 60 秒周期。周期任务仍作为保留期和容量兜底扫描，不读取历史记录重建任何运行态。
- SQLite 使用 `request_logs` 与 `request_attempts` 两张表。配置实体删除后历史外键使用 `ON DELETE SET NULL`，RequestLog 删除时 Attempt 使用 `ON DELETE CASCADE`。
- 自动物化的 Route/Target 按稳定 ID 差异同步；仍然有效的 Target 不得为了重算配置而删除重建，以免 `ON DELETE SET NULL` 不可逆地清除历史 Attempt 关联。只有候选配置中真正失效的 Target 才触发该删除语义。
- `ON DELETE SET NULL` 的子表列必须有以前导外键列开头的索引。除既有索引外，`request_attempts.route_target_id/credential_id/proxy_profile_id` 与 `request_logs.provider_endpoint_id/proxy_profile_id` 需要独立索引，使删除 Target、Credential、Endpoint 或 Proxy 不会在 `BEGIN IMMEDIATE` 配置事务内反复全表扫描遥测历史。
- `request_attempts` 的复合主键已经生成 `(request_id, attempt_no)` 自动索引，并完整覆盖详情按 Request 查询与 Attempt 顺序；不再额外维护同列同序的 `request_attempts_request_idx`。冻结的 `0001` 保持不变，由连续前向 Migration `0008` 只删除这棵冗余 B-tree，主键约束与代表性历史行必须保留。
- 管理列表固定查询最近 3 天，并使用有界 `page`/`page_size`、版本化不透明 `cursor` 和头部锚点范围内的精确 `total` 做服务端锚定分页；相邻下一页走 Keyset，随机页只用现有索引驱动且仅投影排序键的 OFFSET 查询定位边界，再按 Keyset 读取该页完整记录。响应提供服务端实际页码、当前与可选下一 Cursor，不再使用完整日志行 OFFSET 或固定 100/200 条上限。单条详情仍提供 Attempt 时间线。Web 使用真实 `/logs` 与 `/logs/:requestId` deep link，不把 Prompt、请求体或响应体放入缓存或 DOM。完整协议由 ADR-0107 定义。
- RequestLog 列表逐行执行领域解码。当前页某行损坏时跳过该行，按查询汇总一次仅含 `corrupt_rows` 数量的告警，不让一条可丢遥测使整页返回 500；窗口 `total` 继续计算实际持久化行数，因此损坏页可以少于 `page_size`。SQL/事务错误、单条 RequestLog 详情或 Attempt 损坏继续失败，配置和 Secret Repository 保持 fail-closed，不使用这条遥测列表例外。
- 保留窗口或清理使总页数收缩时，Storage 把请求页码夹到最后一个合法位置，Web 使用响应的实际 `page` 和 `cursor` 一次收敛当前定位；下一页资格只取决于 `next_cursor`，不能从总数猜测。
- Writer 在 RequestLog SQLite 批次成功提交后推进进程内变更 epoch。请求日志 Web 只在未固定 Cursor 的最新页订阅已认证的 `/api/admin/log-events`，收到 `request_logs_changed` 后重新读取；历史页暂停订阅，手动刷新清除当前锚点并回到第一页。事件只携带 epoch，不携带 RequestLog 或 Attempt 正文。同一批次只产生一次失效通知，失败写入不通知。首次连接会发送当前 epoch，断线由浏览器原生重连，不持久化或回放通知历史。
- SettingRegistry 注册 `logs.request.enabled`、`logs.request.retention`、RequestLog 专用的 `logs.request.max_rows`、HttpAccessLog 专用的 `logs.http_access.max_rows`/`logs.http_access.max_exchange_bytes` 与 `logs.telemetry_queue_capacity`。策略按 PublishedSnapshot revision 进入请求，已开始的长流不会在结束时混用新 revision；系统日志独立容量与 SQLite 页面回收由 ADR-0092 定义。
- 请求级策略是从已捕获 PublishedSnapshot 的 revision 和 `LoggingSettings` 纯计算得到的值，不读写 RequestTelemetry 的共享策略锁。共享策略只供 Writer、Gateway usage 和周期清理使用；它在启动时由已加载配置初始化，之后唯一更新点是配置成功发布后的 `PublishedSnapshotReconciler`。同一 reconcile 还按当前 Gateway Key 集合淘汰实时使用/节流状态；查询请求策略不得以副作用补做 reconcile 或触发清理。完整生命周期见 ADR-0105。
- `first_token_ms` 与 Token Usage 只由 ADR-0025 定义的协议级精确钩子产生。不得把首个 SSE 控制事件猜成首 Token，也不得解析未知 JSON 字段推测 usage。
- RequestLog 与本地文件日志保持两条独立的有界写入链，但 `logs.request.*` 与已经实现的 `logs.file.*` 共同接入同一 SettingRegistry，不建立第二套配置来源。

## 后果

- SQLite 变慢或锁竞争只影响历史遥测完整度，不影响代理请求延迟、Guard 结算或故障切换。
- RequestLog 与 Attempt 具有一致父子事务和稳定 Request ID，能够还原重试路径而不把 GatewayApiKey 与 ProviderCredential 误建成配置绑定。
- 进程重启后可以查询已持久化日志，但所有 RPM、`in_flight`、队列、健康、会话和请求进度仍从空状态开始。

## 验证

- Domain/Storage 测试覆盖日志设置默认值、记录往返、父子事务、请求日志列表隔离并计数单行损坏、单条详情损坏继续失败、RequestLog 持续批量写入不突破行数上限及多轮有界收敛、两类日志各自的时间/行数/交换字节清理、未变化 Target 的历史引用保留，以及配置实体真正删除后的历史引用置空；Migration 升级测试固定外键与容量索引列及删除查询计划，并证明删除重复 Attempt 索引后主键约束、代表性数据和主键索引查询计划不变。
- Runtime 测试覆盖有界队列立即丢弃、channel/in-flight 转换、存储成功与失败结算、shutdown abort 剩余记录计入 dropped、Gateway 更新折叠后的记录守恒、Attempt 单次完成、取消兜底、Writer 空闲清理、配置下调立即唤醒、请求策略查询无共享状态副作用而发布 reconcile 仍唯一推进共享 revision，以及真实 SQLite 在超过旧 17 req/s 阈值的突发写入后仍满足 RequestLog 行数上限。
- 公共请求契约覆盖本地 Request ID、成功 JSON、Credential 切换后的多 Attempt、预算耗尽、SSE 正常 EOF、提交后错误和客户端 Drop 的真实 SQLite 持久化。
- Storage/Runtime/Server/Web 测试覆盖 3 天窗口、跨页总数、锚定随机跳页与越界页收敛、提交后 epoch、SSE 失效事件、详情契约、列表与详情的成功/空态/错误态、事件驱动刷新、DTO 解析、敏感文本不展示和 SPA deep link；统一 Playwright 套件使用真实服务覆盖登录后的 `/logs` 导航、390×844 视口无水平溢出和浏览器错误检查。
