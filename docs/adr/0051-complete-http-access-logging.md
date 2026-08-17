# ADR-0051：完整 HTTP 访问系统日志

- 状态：Accepted（原始交换字段由 ADR-0081 修订；独立容量由 ADR-0092 修订；分页由 ADR-0107/0162 修订；鉴权拒绝隔离由 ADR-0109 修订；管理活动筛选由 2026-08-16 修订；公开请求 ID 响应头由 ADR-0156 修订；统一实时事件入口由 ADR-0163 修订）
- 日期：2026-07-26
- 修订：2026-08-17；系统日志实时交互由 ADR-0162 修订为常驻统一 SSE，不再提供自动刷新偏好

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

客户端 IP 的规范化和 loopback 判定由 Domain 提供唯一实现：IPv4-mapped IPv6 先通过 `to_canonical()` 转为 IPv4，再调用 loopback 判定。Server 的可信代理解析与 Storage 写入均复用这套语义；追加的 Migration `0009` 把旧版已经持久化的 `::ffff:127.*` 规范为 `127.*`。游标锚点与批次查询引用同一个保留谓词常量，在规范持久化不变量下只识别 `127.*` 与 `::1`，避免查询范围漂移；该降噪规则不授予任何直接 loopback 管理权限。

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

全局中间件生成本地 Request ID，并在响应没有最终上游 `x-request-id` 时用该值补齐唯一的 `x-request-id`；不再写入 any2api 专用响应头。公开模型执行链复用同一个本地 ID 写入 `RequestLog`，系统日志也保存该内部 ID，因此两类日志仍可在服务端按本地 Request ID 关联。

响应 Body 包装器统计成功 yield 的 data frame 字节数，并在 EOF、Body 错误或客户端 Drop 时以原子完成标记只提交一次。状态码在 Handler 返回 Response 时捕获；Body 错误不会伪装成普通完成，Drop 记录为取消。

### 有界写入、保留与清理

系统日志复用现有 `RequestTelemetry` 有界非阻塞写入通道和独立 SQLite writer，不在请求路径等待普通日志落库。队列满时允许丢弃并计入既有遥测丢弃指标。`logs.request.enabled` 与 retention 同时控制 RequestLog 和 HttpAccessLog；`logs.request.max_rows` 只约束 RequestLog，系统日志使用独立的 `logs.http_access.max_rows` 与 `logs.http_access.max_exchange_bytes`。批次提交后按两项容量删除完整记录，新库删除后执行有界增量页面回收；完整边界见 ADR-0092。Gateway 鉴权前拒绝仍逐条进入这条审计链，但使用四分之一队列与持久化子容量，并在全局容量压力下优先淘汰自身；其显式控制流分类和不采用 IP 限流/采样的理由见 ADR-0109。

管理面新增：

```text
GET    /api/admin/system-logs[?cursor=<opaque>][&show_admin_operations=false]
DELETE /api/admin/system-logs
```

列表固定为最近 3 天的单向 Keyset 游标流。首次请求不带 Cursor，服务端按 `(started_at_ms DESC, request_id DESC)` 固定返回最多 100 条摘要；响应只包含 `items`、可选 `next_cursor`、`has_more` 和遥测指标，不包含页码、页大小、精确总数或当前 Cursor。下一批通过头部锚点和上一批末行的排他边界继续，不执行 `COUNT(*)`、OFFSET 或随机跳页。SQLite 的实际保留期限仍由 `logs.request.retention` 控制；DELETE 清理全部保留历史，不只清理可见窗口或当前 Cursor 批次。完整语义由 ADR-0162 定义。

清理、保留或时间窗口推进可以删除 Cursor 范围内的行；Storage 从排他边界之后第一个仍存在的行继续。Web 只在服务端返回 `next_cursor` 时加载更早批次；清理成功后丢弃整条 Cursor 链并回到最新。

列表使用包含 `started_at_ms`、`request_id`、`path`、`client_ip`、`status_code` 与 `outcome` 的摘要覆盖索引。`show_admin_operations=false` 时，锚点和每个游标批次统一排除 `/api/admin` 及其子路径、`/assets/*`，以及 `/any2api-icon.png`、`/boot-theme.js`、`/favicon-16x16.png`、`/favicon-32x32.png`、`/apple-touch-icon.png`、`/index.html` 这些固定管理 Web 根资源；`/api/administrator` 等近似路径仍保留，筛选状态进入 Cursor 作用域。SQLite 只读取 `limit + 1` 个摘要行来判断 `has_more`，不扫描每行最高 2 MiB 的交换详情列。

清理不能绕过 writer 直接删除。DELETE Handler 把带完成回执的 `ClearHttpAccessLogs` 命令发送到同一有序队列：writer 先落盘命令之前的事件，再清空表，最后确认完成。清理请求自己的访问日志在 Response Body 完成后才经过统一规则；本机成功清理会被过滤，外部清理或失败清理形成清理后的审计记录。并发请求在清理边界之后完成时也可以产生新记录。清理命令属于管理操作，可以等待有界队列容量和 writer 回执，普通请求日志仍只使用 `try_send`。

### Web 行为

新增一级菜单“系统日志”和 `/system-logs` deep link。页面展示时间、客户端 IP、method、实际 path、状态、HTTP version、耗时、响应字节与结果，并提供统一游标流、手动刷新、“显示管理操作”Switch 及带确认的历史清理。页面挂载且已认证时始终通过共享 `/api/admin/events` 接收 `system_logs_changed`，不提供自动刷新 Switch 或对应 `localStorage` 偏好；进入历史位置后只在内部暂停可见链改写，通过固定右下角“回到顶部”执行事实追赶，不创建第二条 EventSource，也不显示居中的新日志计数提示。手动刷新、筛选变化和清理成功都清除当前游标并回到最新批次；“显示管理操作”是每个浏览器独立的非敏感界面偏好，使用带版本的 `localStorage` key 持久化。桌面表格继续使用 `@tanstack/react-virtual`，并保持固定表头与独立滚动区；移动端保留自然滚动卡片。

RequestTelemetry Writer 在 RequestLog 批次成功提交后推进对应的进程内 epoch；HttpAccessLog 批次只有包含至少一条非 `GET /api/admin/system-logs` 记录时才推进系统日志 epoch。系统日志列表读取仍按统一规则决定是否持久化，但不能通过记录自身再次触发列表读取。有序清理和保留删除只在确实删除记录后推进。共享 `/api/admin/events` SSE 只发送 `request_logs_changed`、`system_logs_changed` 与 epoch（并可携带总览最新快照），不发送日志正文；同一批次允许合并通知。epoch 不持久化、不恢复、不提供事件回放，新连接先发送当前值以覆盖断线窗口，keepalive 不触发页面读取。

成功通过管理员认证并建立的 `/api/admin/events` SSE 响应由服务端附加 HttpAccessLog 排除标记，避免长连接断开形成系统日志噪音。系统日志列表 `GET` 不排除持久化，只由 Server 抑制其变更通知；客户端 Header 不再拥有或声明任何排除、刷新模式或通知抑制语义。认证失败、无效查询、404/405、首次页面读取、事件驱动同步、手动刷新和其他路径继续由统一审计规则决定。

## 结果

- 管理员可从一处查看有审计价值的 HTTP 异常、外部访问和公开代理历史，而模型请求日志继续保持调度语义。
- 请求路径按客户端实际访问值显示，不被框架模板或归一化逻辑改写。
- 大量历史通过固定 100 条的游标批次连续浏览，不再受固定 200/500 条总量截断，表头在滚动时保持完整且不被数据覆盖。
- 原有“不保存 query、Header 与 Body”结论已由 ADR-0081 取代；系统日志详情现在显式保存原始值并由本地数据目录权限与管理员认证保护。
- Body 生命周期可以区分成功、传输错误和客户端取消。
- 有序清理不会被清理命令之前仍在 writer 队列中的记录回填。
- 事件驱动刷新不会用固定轮询淹没 SQLite 与访问历史，手动读取与异常访问仍可审计。
- 系统日志实时订阅不再受浏览器偏好控制，页面挂载且已认证时始终通过共享 SSE 恢复事实。
- “显示管理操作”选择同样在页面重载和浏览器重启后保持；关闭时管理 API 与管理 Web 静态资源不会占据游标列表。
- 系统日志规模和 Body 捕获增大时，游标批次只扫描摘要覆盖索引，不按详情 BLOB 大小退化。
- 系统日志的原始交换总量和元数据行数具有独立预算，不会再借用 RequestLog 行数上限增长；容量淘汰仍保持单条记录完整。
- Server、Storage 与历史 Migration 对 IPv4、IPv6 和 IPv4-mapped loopback 使用同一规范语义，游标锚点与批次不会再因谓词漂移返回不同集合。
