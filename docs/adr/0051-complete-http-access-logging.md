# ADR-0051：完整 HTTP 访问系统日志

- 状态：Accepted（原始交换字段由 ADR-0081 修订）
- 日期：2026-07-26
- 修订：2026-07-31

## 背景

`RequestLog` 描述已经通过网关鉴权并进入模型执行链的请求。它包含模型、路由、上游凭据、Attempt 和 Token Usage，因此不适合表达管理 API、健康检查、Web 资源、公开鉴权失败、404 或 405。独立 HTTP 访问日志覆盖整个 Axum 服务，并支持 Web 事件驱动刷新和手动清理。

本 ADR 最初排除了 query、Header 与 Body。操作员随后明确要求系统日志保存原始客户端侧 HTTP 交换；该字段范围、1 MiB 单向 Body 捕获边界和安全例外由 ADR-0081 取代本 ADR 的旧排除规则。

## 决策

### 独立模型与完整覆盖

新增独立 SQLite `http_access_logs` 表和 `HttpAccessLog` 领域模型。全局 Axum 中间件位于所有公开路由、管理路由、健康检查与 Web fallback 之外，先观察每个请求的完整 Body 生命周期，再按审计价值决定是否写入：

- `/v1` 公开代理请求无论结果都保留；
- 客户端地址未知或不是 loopback 的外部访问无论结果都保留；
- 任意 HTTP 4xx/5xx、Body 错误或取消都保留；
- loopback 发起、非 `/v1`、状态低于 400 且 Body 正常完成的管理 API、健康检查、Web 资源和 deep link 属于内部正常流量，不写入。

列表 SQL 复用相同谓词，立即隐藏规则发布前已写入的内部噪音；不能只在写入端生效而让旧噪音继续展示 3 天。

Schema 对 `method` 只要求非空且去除首尾空白，不设置人为 32 字符上限。

`HttpAccessLog` 保存：

- 全局 Request ID；
- 请求开始时间与捕获的 PublishedSnapshot revision；
- 可信代理规则解析后的规范客户端 IP；
- HTTP method；
- Server 实际收到的原始 URI path；
- HTTP version；
- 最终状态码；请求在 Handler 返回 Response 前取消时为空；
- 直至响应 Body 结束的耗时；
- 实际向下游 yield 的响应字节数；
- `completed`、`body_error` 或 `cancelled` 结果。

`path` 仍直接来自 `request.uri().path()` 并服务摘要与保留规则，不使用 `MatchedPath`、通配归一化、框架路由模板或内部重写结果。ADR-0081 另行增加含 query 的完整 URI、两侧 Header 与有界 Body 详情；这些值不做脱敏。

### 生命周期与关联

全局中间件生成 Request ID 并为全部响应写入 `x-any2api-request-id`；仅在响应没有最终上游 `x-request-id` 时用本地值补齐该字段。公开模型执行链复用同一个本地 ID 写入 `RequestLog`，使两类日志可按 `x-any2api-request-id` 关联。

响应 Body 包装器统计成功 yield 的 data frame 字节数，并在 EOF、Body 错误或客户端 Drop 时以原子完成标记只提交一次。状态码在 Handler 返回 Response 时捕获；Body 错误不会伪装成普通完成，Drop 记录为取消。

### 有界写入、保留与清理

系统日志复用现有 `RequestTelemetry` 有界非阻塞写入通道和独立 SQLite writer，不在请求路径等待普通日志落库。队列满时允许丢弃并计入既有遥测丢弃指标。`logs.request.enabled` 同时控制 RequestLog 与 HttpAccessLog；retention 和 max_rows 对两个顶层日志表分别应用。

管理面新增：

```text
GET    /api/admin/system-logs?page=1&page_size=20
DELETE /api/admin/system-logs
```

列表固定为最近 3 天的服务端分页，并返回窗口内精确 `total`、当前 `page` 与 `page_size`。SQLite 的实际保留期限仍由 `logs.request.retention` 控制；DELETE 清理全部保留历史，不只清理可见窗口或当前页。

清理不能绕过 writer 直接删除。DELETE Handler 把带完成回执的 `ClearHttpAccessLogs` 命令发送到同一有序队列：writer 先落盘命令之前的事件，再清空表，最后确认完成。清理请求自己的访问日志在 Response Body 完成后才经过统一规则；本机成功清理会被过滤，外部清理或失败清理形成清理后的审计记录。并发请求在清理边界之后完成时也可以产生新记录。清理命令属于管理操作，可以等待有界队列容量和 writer 回执，普通请求日志仍只使用 `try_send`。

### Web 行为

新增一级菜单“系统日志”和 `/system-logs` deep link。页面展示时间、客户端 IP、method、实际 path、状态、HTTP version、耗时、响应字节与结果，并提供与请求日志一致的分页、手动刷新、自动刷新 Switch 和带确认的历史清理。Switch 开启后订阅已认证的 `/api/admin/log-events`，收到 `system_logs_changed` 后重新读取当前页；关闭后断开订阅。自动刷新状态是每个浏览器独立的非敏感界面偏好，使用带版本的 `localStorage` key 持久化，不写入服务端 SettingRegistry；没有保存值、值无效或浏览器拒绝存储时默认开启。桌面表格继续使用 `@tanstack/react-virtual`，并保持固定表头与独立滚动区；移动端保留自然滚动卡片。

RequestTelemetry Writer 在 RequestLog 批次成功提交后推进对应的进程内 epoch；HttpAccessLog 批次只有包含至少一条非 `GET /api/admin/system-logs` 记录时才推进系统日志 epoch。系统日志列表读取仍按统一规则决定是否持久化，但不能通过记录自身再次触发列表读取。有序清理和保留删除只在确实删除记录后推进。SSE 只发送 `request_logs_changed`、`system_logs_changed` 与 epoch，不发送日志正文；同一批次允许合并通知。epoch 不持久化、不恢复、不提供事件回放，新连接先发送当前值以覆盖断线窗口，keepalive 不触发页面读取。

成功通过管理员认证并建立的 SSE 响应由服务端附加 HttpAccessLog 排除标记，避免长连接断开形成系统日志噪音。系统日志列表 `GET` 不排除持久化，只由 Server 抑制其变更通知；客户端 Header 不再拥有或声明任何排除、自动刷新或通知抑制语义。认证失败、无效查询、404/405、首次页面读取、自动刷新、手动刷新和其他路径继续由统一审计规则决定。

## 结果

- 管理员可从一处查看有审计价值的 HTTP 异常、外部访问和公开代理历史，而模型请求日志继续保持调度语义。
- 请求路径按客户端实际访问值显示，不被框架模板或归一化逻辑改写。
- 大量历史通过服务端分页浏览，不再受固定 200/500 条截断，表头在滚动时保持完整且不被数据覆盖。
- 原有“不保存 query、Header 与 Body”结论已由 ADR-0081 取代；系统日志详情现在显式保存原始值并由本地数据目录权限与管理员认证保护。
- Body 生命周期可以区分成功、传输错误和客户端取消。
- 有序清理不会被清理命令之前仍在 writer 队列中的记录回填。
- 事件驱动刷新不会用固定轮询淹没 SQLite 与访问历史，手动读取与异常访问仍可审计。
- 自动刷新选择在页面重载和浏览器重启后保持，同时不会把一台设备的界面偏好扩散为实例级配置。
