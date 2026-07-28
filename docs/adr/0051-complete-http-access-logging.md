# ADR-0051：完整 HTTP 访问系统日志

- 状态：Accepted
- 日期：2026-07-26

## 背景

`RequestLog` 描述已经通过网关鉴权并进入模型执行链的请求。它包含模型、路由、上游凭据、Attempt 和 Token Usage，因此不适合表达管理 API、健康检查、Web 资源、公开鉴权失败、404 或 405。独立 HTTP 访问日志覆盖整个 Axum 服务，并支持 Web 自动刷新和手动清理。

系统仍需遵守 Secret 不落日志的边界。尤其是 OAuth callback、登录链接或客户端 URL 可能在 query 中携带 code、token 或其他敏感值，完整访问日志不能等同于保存完整 URI、Header 或 Body。

## 决策

### 独立模型与完整覆盖

新增独立 SQLite `http_access_logs` 表和 `HttpAccessLog` 领域模型。全局 Axum 中间件位于所有公开路由、管理路由、健康检查与 Web fallback 之外，因此每个到达 Axum 的请求都生成一条系统日志，包括认证失败、404 和 405。

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

它不保存 query、Header、Cookie、User-Agent、Referer、请求体或响应体。`path` 直接来自 `request.uri().path()`，不使用 `MatchedPath`，不做通配归一化，也不保存框架路由模板或内部重写结果。这保证管理页面看到的就是客户端实际访问的路径，同时明确排除最容易携带 Secret 的 query。

### 生命周期与关联

全局中间件生成 Request ID 并为全部响应写入 `x-any2api-request-id`；仅在响应没有最终上游 `x-request-id` 时用本地值补齐该字段。公开模型执行链复用同一个本地 ID 写入 `RequestLog`，使两类日志可按 `x-any2api-request-id` 关联。

响应 Body 包装器统计成功 yield 的 data frame 字节数，并在 EOF、Body 错误或客户端 Drop 时以原子完成标记只提交一次。状态码在 Handler 返回 Response 时捕获；Body 错误不会伪装成普通完成，Drop 记录为取消。

### 有界写入、保留与清理

系统日志复用现有 `RequestTelemetry` 有界非阻塞写入通道和独立 SQLite writer，不在请求路径等待普通日志落库。队列满时允许丢弃并计入既有遥测丢弃指标。`logs.request.enabled` 同时控制 RequestLog 与 HttpAccessLog；retention 和 max_rows 对两个顶层日志表分别应用。

管理面新增：

```text
GET    /api/admin/system-logs
DELETE /api/admin/system-logs
```

清理不能绕过 writer 直接删除。DELETE Handler 把带完成回执的 `ClearHttpAccessLogs` 命令发送到同一有序队列：writer 先落盘命令之前的事件，再清空表，最后确认完成。清理请求自己的访问日志在 Response Body 完成后才排入队列，因此会作为清理后的审计记录保留；并发请求在清理边界之后完成时也可以产生新记录。清理命令属于管理操作，可以等待有界队列容量和 writer 回执，普通请求日志仍只使用 `try_send`。

### Web 行为

新增一级菜单“系统日志”和 `/system-logs` deep link。页面展示时间、客户端 IP、method、实际 path、状态、HTTP version、耗时、响应字节与结果，并提供手动刷新、自动刷新 Switch 和带确认的历史清理。Switch 开启后固定每 5 秒刷新，关闭后停止轮询。自动刷新状态是每个浏览器独立的非敏感界面偏好，使用带版本的 `localStorage` key 持久化，不写入服务端 SettingRegistry；没有保存值、值无效或浏览器拒绝存储时默认开启。桌面表格使用 `@tanstack/react-virtual` 只渲染可视行和少量 overscan，固定表头与虚拟行滚动区分层，避免滚动数据穿透表头；移动端保留自然滚动卡片。

定时自动刷新请求携带内部用途标记，但请求标记本身不能绕过日志。只有请求已经通过管理员认证、查询校验并进入系统日志列表 Handler 后，Handler 才在成功响应扩展中附加排除标记；最外层 HttpAccessLog 中间件看到该响应标记后取消本次记录。首次页面加载与手动刷新不携带自动标记，清理请求、认证失败、无效查询、404/405 和其他路径也无法产生受信任的响应标记，继续完整记录。

## 结果

- 管理员可从一处查看整个 HTTP 服务的访问历史，而模型请求日志继续保持调度语义。
- 请求路径按客户端实际访问值显示，不被框架模板或归一化逻辑改写。
- 大量历史记录不会创建等量桌面 DOM 行，表头在滚动时保持完整且不被数据覆盖。
- query、Header 与 Body 不落库，避免为了“完整 HTTP 日志”破坏 Secret 边界。
- Body 生命周期可以区分成功、传输错误和客户端取消。
- 有序清理不会被清理命令之前仍在 writer 队列中的记录回填。
- 定时轮询不会用系统日志读取记录淹没真正的访问历史，手动读取与异常访问仍可审计。
- 自动刷新选择在页面重载和浏览器重启后保持，同时不会把一台设备的界面偏好扩散为实例级配置。
