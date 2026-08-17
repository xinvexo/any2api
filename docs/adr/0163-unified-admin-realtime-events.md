# ADR-0163：统一管理员实时事件与总览快照

- 状态：Accepted
- 日期：2026-08-17
- 关联：`ARCHITECTURE.md` 第 18 节、第 19.4 节、ADR-0069、ADR-0160、ADR-0162

## 背景

总览的进程/主机资源和调度负载本来由多个管理查询分别读取，Web 端还为不同视图建立独立连接或定时刷新。日志变更、总览运行态和 Provider 运行态因此拥有不同的刷新时机，连接数和采样成本随打开的页面增长；认证失效时，浏览器的自动重试还可能形成重连风暴。追加需求要求单节点服务只采样一次，并让所有管理实时视图共享同一条管理员 SSE。

## 决策

1. 服务端提供已认证的 `GET /api/admin/events`，作为管理员实时事件的统一入口。它替代旧的 `/api/admin/log-events`；公开 `/v1` 和 `/api/health` 不变。响应不承载日志正文或 Secret，且由统一 HTTP 访问日志边界排除成功建立的通知流。
2. AppState 持有一个进程内 `AdminRealtimeHub`。Hub 只维护最新总览快照、请求日志 epoch 和系统日志 epoch，并向所有连接广播；它不持久化、不回放历史，也不把连接数或事件发送作为路由/准入状态。
3. Hub 启动一个由应用生命周期管理的共享 sampler，每 2 秒读取一次资源采样器和当前运行态聚合，生成 `overview_snapshot`。采样失败时保留上一份有效快照并携带 freshness/error 状态；尚无有效快照时通过稳定错误字段表达不可用，不伪造零值。每个新连接立即收到最新快照（若存在）和当前日志 epoch。
4. 浏览器在已认证管理壳内只创建一个 `EventSource`。`AdminRealtimeProvider` 将 `overview_snapshot`、请求/系统日志 epoch、`oauth_quota_changed` 与 `oauth_refresh_diagnostic_changed` 分发给各 feature；短时间内的日志事件由客户端合并，再使用当前游标链的 HTTP 查询恢复事实数据。额度与刷新诊断事件只使对应 HTTP Query 失效，不携带业务正文。旧 `/api/admin/log-events` 与 `/api/admin/oauth/quota-events` 均不再保留。总览快照不要求逐事件回放。
5. 连接断开或 SSE 出错时，浏览器保留最后一份快照并显示 stale/disconnected 状态；即使连接仍存在，连续 7 秒未收到新的 fresh snapshot 也将最近值标记为 stale。重连成功后服务端重新发送最新快照。收到 session 失效或 401/403 后关闭 EventSource、停止重连并清除管理员状态，不能持续创建新连接。手动刷新仍可执行 HTTP bootstrap。
6. `overview_snapshot` 至少包含 `sampled_at_ms`、资源字段、运行态负载字段和 `freshness`；其结构复用资源/负载管理 DTO 的安全投影，不包含逐凭据明细、Secret、Token、提示词或日志正文。历史 `overview/usage` 仍通过 HTTP 按当前范围刷新，默认 60 秒；实时事件不得触发聚合统计重算。
7. 实时资源与运行态管理查询不再配置固定客户端 `refetchInterval`。HTTP 端点保留用于首屏回退、手动刷新、权限/能力探测和历史统计；服务端共享采样是唯一的周期来源。

## 取舍

- 采用 SSE 而非 WebSocket，复用现有管理员认证、反向代理和浏览器重连语义；不引入新协议栈或大型依赖。
- 采用最新快照而非事件回放，避免为单节点运行态建立持久化队列；日志仍以 epoch + 游标 HTTP 作为一致性事实来源。
- 2 秒是资源可见性与采样开销之间的固定产品默认值，后续若需调整必须通过配置/ADR 明确改变，不由客户端自行决定。

## 验证

- Server 测试确认新连接立即收到快照/epoch、多个连接共享同一 sampler、draining 结束流，以及认证失效不继续推送。
- Web 契约测试确认单个 `EventSource`、重连后的 snapshot bootstrap、stale 状态、session 失效停止重连和实时 Query 不再使用固定轮询。
- 集成验证确认历史 usage 仍按 60 秒 HTTP 刷新、日志事件只触发游标同步，且 `/v1` 与 `/api/health` 契约不变。
