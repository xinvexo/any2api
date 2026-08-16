# ADR-0156: 移除重复的本地请求 ID 响应头

- 状态：Accepted
- 日期：2026-08-16
- 决策者：maintainer
- 关联：ADR-0015（请求遥测）、ADR-0051（HTTP 访问系统日志）

## 背景

Server 以前在每个响应中同时发送 `x-any2api-request-id` 和 `x-request-id`。当上游没有请求 ID
时，两者是同一个本地 UUID；当上游已经返回请求 ID 时，前者仍然暴露网关生成的另一个关联值。
这会给客户端增加没有额外语义的 any2api 专用 Header，也容易让响应看起来不像直接来自上游。

## 决策

1. Server 继续为每个 HTTP 请求生成本地 `RequestId`，并把它放入请求扩展，供 Runtime、RequestLog
   和 HttpAccessLog 做内部关联。
2. 公开响应最多保留一个 `x-request-id`：最终 Attempt 已返回可归一化的上游请求 ID 时保留上游值；
   没有该值时才用本地 `RequestId` 补齐。
3. 所有公开、管理、健康、静态资源和 Responses WebSocket 尝试的 HTTP 426 回退响应都不再发送
   `x-any2api-request-id`。客户端传入的 `x-request-id` 仍按既有鉴权边界剥离，不会被转发给上游。
4. HttpAccessLog 仍在最终响应 Header 完成归一化后捕获 Header；日志中的 `request_id` 字段继续保存
   内部本地 ID，不改数据库结构、不做数据迁移。

## 取舍

- 客户端失去读取网关本地 ID 与上游 ID 的双字段视图，但同一响应始终有一个可用的
  `x-request-id`，本地错误仍能关联系统日志。
- 上游返回自己的请求 ID 时，客户端只能看到上游值；需要网关内部关联时使用已认证系统日志中的
  Request ID，而不是增加第二个公开 Header。

## 验证

- SSE、Responses WebSocket 尝试的 HTTP 426 回退响应和系统日志详情契约断言不出现 `x-any2api-request-id`。
- 上游提供请求 ID 时响应保留该值；本地错误/上游缺失时响应 `x-request-id` 仍为可解析的本地 UUID。
- 现有 RequestLog、HttpAccessLog 和客户端 Header 剥离测试继续验证内部 Request ID 与上游请求面不变。
