# ADR-0015: 有界请求遥测、Attempt 历史与查询边界

- 状态：Accepted
- 日期：2026-07-20
- 决策者：maintainer

## 背景

数据面已经具备同协议 JSON/SSE、可靠性分类和提交前多 Attempt，但 Attempt 结果只用于进程内健康状态，没有形成可查询的 RequestLog 历史。日志写入不能阻塞请求、延长 GuardedBody Drop、争抢配置事务，也不能被误用来恢复并发、队列、会话或熔断状态。

## 决策

- Server 在 `/v1` 鉴权层之外生成本地 `RequestId`，覆盖响应 `x-request-id`，并把同一个 ID 传入 Runtime。客户端提供的同名响应关联值不能覆盖本地 ID。
- 首个切片持久化已通过 GatewayApiKey 鉴权并进入模型执行链的请求，包括解码、规划、排队和上游执行错误。鉴权失败、未知公开路由和方法错误只返回本地 Request ID 并写结构化文件/终端日志，不写 RequestLog。
- Runtime 使用每请求 `RequestRecorder` 和每次上游执行 `AttemptRecorder`。Attempt 在健康状态结算之后、Credential Permit 释放之前完成；正常 JSON、错误、超时、取消和流式 Drop 都只能完成一次。
- Request 与全部 Attempt 先在当前请求内存中聚合。请求结束时只执行一次同步 `try_send`，把完整聚合记录放入有界队列；队列满、Writer 已关闭或 SQLite 写入失败时丢弃该条遥测并增加计数，禁止等待、重试或反压数据面。
- SSE 在首帧验证和软绑定提交成功后才把请求最终完成权交给 `GuardedBody`。正常 EOF、提交后错误和客户端 Drop 由 GuardedBody 记录最终结果；首帧或提交前失败仍由普通请求路径完成 RequestLog。
- Runtime 后台 Writer 从有界队列按小批次读取，在一个 SQLite 事务中先写父 RequestLog、再写 RequestAttempt。保留清理按时间与最大行数任一上限定时分批删除；配置发布会无失败地刷新清理策略，不依赖下一次公开请求，且不读取历史记录重建任何运行态。
- SQLite 使用 `request_logs` 与 `request_attempts` 两张表。配置实体删除后历史外键使用 `ON DELETE SET NULL`，RequestLog 删除时 Attempt 使用 `ON DELETE CASCADE`。
- 首个管理查询提供最近 RequestLog 列表与单条详情/Attempt 时间线。Web 使用真实 `/logs` 与 `/logs/:requestId` deep link，不把 Prompt、请求体或响应体放入缓存或 DOM。
- 首个切片注册 `logs.request.enabled`、`logs.request.retention`、`logs.request.max_rows` 与 `logs.telemetry_queue_capacity`。策略按 PublishedSnapshot revision 进入请求，旧长流不会在结束时混用新 revision。
- `first_token_ms` 与 Token Usage 在没有协议级精确提取钩子前保存为 `NULL`。不得把首个 SSE 控制事件猜成首 Token，也不得解析未知 JSON 字段推测 usage。
- RequestLog 与本地文件日志保持两条独立的有界写入链，但 `logs.request.*` 与已经实现的 `logs.file.*` 共同接入同一 SettingRegistry，不建立第二套配置来源。

## 后果

- SQLite 变慢或锁竞争只影响历史遥测完整度，不影响代理请求延迟、Permit 释放或故障切换。
- RequestLog 与 Attempt 具有一致父子事务和稳定 Request ID，能够还原重试路径而不把 GatewayApiKey 与 ProviderCredential 误建成配置绑定。
- 进程重启后可以查询旧历史，但所有并发、队列、健康、会话和请求进度仍从空状态开始。
- 后续 Token Usage、首 Token、文件日志和筛选分页可以在现有类型与 API 上局部扩展。

## 验证

- Domain/Storage 测试覆盖日志设置默认值、记录往返、父子事务、时间/行数清理与配置实体删除后的历史引用置空。
- Runtime 测试覆盖有界队列立即丢弃、丢弃计数、Attempt 单次完成、取消兜底与 Writer 空闲清理。
- 公共请求契约覆盖本地 Request ID、成功 JSON、Credential 切换后的多 Attempt、预算耗尽、SSE 正常 EOF、提交后错误和客户端 Drop 的真实 SQLite 持久化。
- Server/Web 测试覆盖详情契约、列表与详情的成功/空态/错误态、DTO 解析、敏感文本不展示和 SPA deep link；统一 Playwright 套件使用真实服务覆盖登录后的 `/logs` 导航、390×844 视口无水平溢出和浏览器错误检查。
