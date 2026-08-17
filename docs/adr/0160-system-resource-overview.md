# ADR-0160：系统总览的实时资源与负载指标

- 状态：Accepted（实时传输部分由 ADR-0163 修订）
- 日期：2026-08-16
- 更新：2026-08-17：总览改为资源网格与请求负载面板的仪表盘层级；实时采样与传输改由 ADR-0163 统一定义；首屏文案移除实现术语与 Transport Client 诊断项
- 关联：`ARCHITECTURE.md` 第 18 节、第 19.4 节

## 背景

系统总览同时承担调用分析和运行态判断。原有页面把调度快照中的多个内部计数平铺出来，却没有 any2api 进程和主机资源，使用者很难一眼判断当前是否接近负载边界。资源采样还需要比配置/调度诊断更高的刷新频率，不能把 balancing 响应变成一个混合职责 DTO。

## 决策

1. 新增已认证端点 `GET /api/admin/overview/resources`，响应固定为：
   - `process.resident_memory_bytes`：any2api 当前进程 RSS；
   - `process.cpu_usage_percent`：进程 CPU 占整机逻辑 CPU 总能力的百分比，归一化到 `0..100`；
   - `system.used_memory_bytes`、`system.total_memory_bytes`：sysinfo 报告的系统内存使用/总量；
   - `system.cpu_usage_percent`：系统全局 CPU 百分比，范围 `0..100`；
   - `sampled_at_ms`：采样时间戳。
2. RuntimeRegistry 持有一个进程内 `system_metrics` 采样器。采样只刷新当前 PID、内存和 CPU 必要字段，并通过生命周期追踪的 blocking 任务执行；由应用级共享实时 Hub 每 2 秒调度一次，不为每个浏览器连接或管理查询重复采样。采样不持久化、不恢复，不参与路由、RPM、健康、额度或停机判定。首次采样在 blocking 任务内完成 sysinfo 的最小 CPU 基线间隔，后续采样使用差分样本；尚无完整基线时不伪造百分比。
3. 调度负载继续由现有运行态快照构造。总览以四项资源 tile 加一个请求负载面板呈现实时状态：投影尚未结束的上游请求、排队、近 60 秒请求率，以及 `enabled_credential_count / credential_count` 对应的已启用账号与密钥数量；存在本地限制时才显示达到每分钟请求上限的账号与密钥。scheduler epoch、遥测队列容量/丢弃数、Transport Client 缓存条目与命中等保留给诊断接口，不在首屏占位。资源与运行态快照通过已认证统一管理员 SSE 推送，HTTP 端点仅用于首屏 bootstrap、手动刷新和故障回退。
4. Web 使用一个共享 `EventSource` 订阅 `/api/admin/events`，多个总览/日志/系统日志/Provider 视图只消费该连接，不各自建立连接。服务端连接建立后立即发送 `overview_snapshot`（资源与运行态）及当前日志 epoch；随后每 2 秒广播最新总览快照。连接断开时保留最近快照并显示 stale/disconnected 状态；重连再次以最新快照 bootstrap，日志事件另行通过游标 HTTP 同步。认证 session 失效时服务端关闭流，客户端停止重连并回到登录态，避免重连风暴。
5. 总览历史调用统计仍使用 `GET /api/admin/overview/usage`，按所选范围每 60 秒刷新；实时快照事件不得触发历史聚合风暴。手动刷新同时请求资源、运行态和调用统计，只有三者全部成功才发送成功通知。无数据使用稳定占位符；刷新失败保留最近值并显示明确告警，禁止把错误转成 0。浏览器不再对实时资源或运行态设置固定 `refetchInterval`。
6. 资源端点采样不可用时返回稳定的 `system_metrics_unavailable` 503 错误，不暴露主机路径、Secret 或底层错误正文。资源字段只存在于当前响应和内存采样器中。
7. 普通显式会话的活动/建立中计数继续由受认证的 Affinity 管理 API 提供；系统总览不渲染独立会话卡或两个策略关闭时的零值，并删除无路由入口的旧会话总览前端代码。

## 取舍

- 不把资源字段并入 balancing：两者刷新频率和失败语义不同，且 balancing 仍被其他管理视图复用。
- 不承诺或展示底层 TCP socket 数。Transport 共享 Client 缓存属于实现诊断，不作为用户需要判断的请求负载；当前未结束请求才是首屏展示的直接压力指标。
- 不引入持久化指标表或独立监控服务：单节点个人部署只需要当前负载，历史调用趋势继续由 RequestLog 提供。

## 验证

- Runtime 单元测试覆盖 CPU 百分比归一化和边界；
- Server/contract 测试覆盖资源响应字段、认证路由和 503 映射；
- Web 契约测试覆盖安全数值、百分比范围、内存关系和错误不归零；
- 浏览器验证总览首屏以资源网格和请求负载面板建立层级，包含调用趋势入口与八项用户可理解的负载指标；桌面/手机均无横向溢出，后台刷新不造成按钮闪烁。
