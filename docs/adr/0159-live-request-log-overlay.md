# ADR-0159：请求日志使用进程内活动请求投影

- 状态：Accepted
- 日期：2026-08-16
- 决策者：maintainer
- 修订：2026-08-17 由 ADR-0162 将活动投影接入无页码的最新游标批次；统一实时事件入口由 ADR-0163 修订

## 背景

当前模型 RequestLog 只在请求完成时构造：缓冲响应在返回前结算，流式响应在 EOF、错误、取消或
Drop 时结算，然后通过有界非阻塞遥测队列批量写入 SQLite。因此请求日志页面只能看到最终结果，
无法观察正在排队、等待上游或持续流式传输的请求。

RequestLog 是可降级历史遥测。公开数据面不能为了日志等待 SQLite，遥测队列满或 Writer 故障时允许
丢弃。如果在请求开始时写入持久化 pending 行、完成时再更新同一行，开始和完成两个事件可能独立
丢失：完成事件丢失会留下永久 pending，开始事件丢失会使完成更新找不到目标。同步确认 pending
落库又会让 SQLite 成为公开请求准入条件，并与非阻塞遥测边界冲突。进程重启后持久 pending 还会把
已经消失的运行态伪装成仍在执行。

## 决策

`RequestTelemetry` 持有进程内 `ActiveRequestRegistry`。已经通过 Gateway Key 鉴权并进入
`PublicRequestService::execute` 的请求，在 `RequestRecorder` 创建时登记活动投影；更早的鉴权拒绝、
Body 提取失败、管理请求和静态资源仍由独立 HttpAccessLog 覆盖，不进入模型活动请求列表。

活动投影只保存安全且当时已经知道的字段：Request ID、开始时间、规范客户端 IP、配置 revision、
Gateway Key、协议和操作。请求解码后补充公开模型、思考级别与流式标记；每次 Attempt 开始时更新
当前 Endpoint、ProviderCredential 或 OAuthAccount、代理以及已开始的 Attempt 数。禁止保存 Prompt、
Header、Body、Secret、原始 Session ID 或 OAuth JSON。

活动投影不写入 SQLite，不进入 RequestLog/Attempt 容量、保留期、统计、总览、额度估算或 OAuth
telemetry sequence。它不参与启动恢复，进程结束后直接清空。最终成功、失败和取消继续沿用现有
exactly-once 结算，生成一条完整 `CompletedRequestLog` 和全部 Attempt。

管理请求日志的最新游标批次在持久化 `items` 之外返回 `active_items`/`active_total`：

- 只在不带 Cursor 的最新批次返回活动项；历史 Cursor 批次返回空活动集合；
- `active_items` 按开始时间和 Request ID 倒序，最多返回 100 条；
- 精确 `public_model` 与 `gateway_api_key_id` 筛选作用于活动项；任何最终 `outcome` 筛选隐藏活动项；
- 活动项不占用持久化批次大小，也不改变历史锚点；
- Web 把活动项置于最终日志之前，显示“请求中”，最终前不提供详情抽屉；同一 Request ID 的最终项出现后必须去掉活动项，不能短暂显示两行。

活动注册和安全字段变化推进独立 `active_requests_changed` epoch。成功写入最终日志时，Writer 在
SQLite Commit 之后先从注册表移除对应活动项，再推进既有 commit-only
`request_logs_changed`；浏览器一次重新查询即可从活动项切换到最终项。最终遥测无法入队或 SQLite
写入失败时，请求已经结束，终止路径移除活动项并推进 `active_requests_changed`，允许最终历史记录
按既有可降级语义缺失，但禁止留下假的“请求中”。两个事件共用现有已认证 `/api/admin/events`
连接，事件仍只携带 epoch，不携带日志正文；连接同时承载 ADR-0163 的总览快照。

## 备选方案

- SQLite pending 行加完成 UPDATE：能够在数据库中表现为同一行，但必须改变 Schema、统计、容量和
  清理语义；非阻塞队列会产生孤立 pending，同步确认则阻塞数据面，因此不采用。
- 直接展示 `RequestTelemetryMetrics.in_flight_records`：该指标表示 Writer 已收到但尚未持久化的遥测
  数量，不是正在执行的请求，也没有逐请求字段，因此不采用。
- 复用 Credential `in_flight`：它是凭据级资源生命周期观测，不覆盖解码、规划和尚未选路的请求，
  也不能映射到 Request ID，因此不采用。

## 后果

- 页面可以在请求进入 Runtime 后立即显示活动行，并在长时、流式和取消请求完成后切换为最终结果。
- 不新增 Migration，不改变最终 RequestLog、Attempt、统计和 quota fence 的存储契约。
- 活动列表是当前进程的瞬时观察；重启、遥测关闭或请求尚未进入 Runtime 时不会显示。
- 每个活动请求增加一条有界字段投影和少量通知；管理响应按固定批次上限限制正文，不返回无界活动集合。

## 验证

- Runtime 测试覆盖登记、解码/Attempt 更新、过滤和排序，以及成功 Commit、入队失败、SQLite 失败和
  取消后的清理。
- Server DTO/SSE 测试覆盖活动项标签、独立 epoch 和最终项去重。
- Web 契约与组件测试覆盖“请求中”显示、无最终指标、不可打开详情、活动/最终去重切换和历史批次不展示活动项。
- 运行 Rust fmt、相关 clippy/test，以及 Web typecheck、lint、test 与仓库根完整应用 `pnpm build`。
