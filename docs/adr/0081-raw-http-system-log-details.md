# ADR-0081：系统日志保存原始 HTTP 交换详情

- 状态：Accepted
- 日期：2026-08-02
- 决策者：maintainer

## 背景

现有 `HttpAccessLog` 只保存 method、path、状态、耗时和响应字节数，主动丢弃 query、Header 与 Body。这不足以从系统日志复盘客户端实际发送的参数和服务端实际返回的内容，也无法为后续实时关键词策略提供可核对的历史样本。模型 `RequestLog` 继续承担路由、Attempt 和 Token 观测，不能混入 HTTP 正文。

## 决策

- 最外层 Axum 系统日志中间件记录客户端侧完整 URI（含 query）、鉴权剥离前的全部请求 Header、注入本地 Request ID 后的全部响应 Header，以及两侧 Body 捕获。它不记录 Runtime 构造的上游请求，也不把 Provider 原始上游响应与最终客户端响应混为一谈。
- Header 保留同名多值和原始值字节。管理 DTO 对有效 UTF-8 原样返回；其他字节使用 Base64 并携带编码字段。
- Body 继续按帧旁路观察，不预先缓冲、不改变背压。请求和响应各保存最多前 1 MiB，并同时保存实际观察字节数、`complete` 和 `truncated`；未读请求体、错误、取消或无界流必须显式显示未完整或截断。
- 这些原始 HTTP 字段不做 Secret、Cookie、认证头、query 或正文脱敏。这是操作员明确选择的系统日志例外，SQLite/数据目录权限与管理员认证是保护边界。普通 tracing/file log、模型 RequestLog、错误正文、Debug、日志变更 SSE 和浏览器持久化仍维持 Secret 禁止规则。
- 分页端点只查询摘要列。新增 `GET /api/admin/system-logs/{request_id}` 单条详情端点，避免一页日志同时载入多条正文；Web 通过可访问的行操作按需打开详情。
- 追加前向 Migration 扩展 `http_access_logs`。旧记录保留原摘要，`uri` 回填为既有 path，并以 `exchange_captured = 0` 明确表示迁移前没有原始交换，禁止把默认空值解释为真实空 Header 或 Body。
- 现有写入范围、3 天管理查询窗口、有界非阻塞遥测队列、保留上限、有序清理和 SSE 通知规则不变。

## 后果

- 管理员能在系统日志中查看一条请求的完整客户端侧输入与输出语义，包括查询参数、认证字段和正文。
- 数据库可能保存高敏感值，读取权限等同于本地数据目录和管理员权限；产品不提供脱敏视图或浏览器持久化。
- 单条大正文和流式响应不会无限增长日志；详情会准确说明捕获边界，后续关键词拦截应在实时请求/响应流上执行，不能把历史 Body 前缀当作完整拦截数据面。
- 原始线协议的 Header 大小写、排列和分块格式在进入 Axum/hyper 时已经规范化；系统日志保存的是框架收到的完整语义值，而不是 TCP 字节转储。

## 验证

- Server 模块测试覆盖任意 Body 分帧、未读请求、EOF、Body error、Drop、重复 Header、UTF-8/二进制值和 1 MiB 截断。
- Storage 升级测试用既有系统日志记录验证摘要保留和 `exchange_captured = 0`，Repository 测试验证新详情逐字节往返且列表不读取详情列。
- 管理契约测试发送含 query、重复 Header 和正文的请求，验证详情端点返回原值、响应 Header/Body 与完成状态；Web 契约和组件测试验证详情懒加载及原始内容展示。
