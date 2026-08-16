# ADR-0161：移除 OpenAI Responses WebSocket 入口

- 状态：Accepted
- 日期：2026-08-17
- 决策者：maintainer
- 取代：ADR-0151

## 问题

Responses WebSocket 入口要求 any2api 为每个客户端维护长连接、连接内增量状态、取消和配置代际
校验。该入口没有带来上游 WebSocket 能力：每条消息最终仍被转换成独立的 HTTP JSON/SSE 请求，
而远程压缩等 Codex 工作流在长连接状态和响应头语义缺失时不稳定。维护第二套入口还增加了
客户端、反向代理和测试的故障面。

## 决策

移除 server 的 WebSocket Upgrade、protocol 帧状态模块、runtime 导出和对应契约测试。公开
`POST /v1/responses` 继续提供 JSON/SSE；`GET /v1/responses` 保留为明确的 HTTP 426 响应，错误码
为 `websocket_unavailable`，让 Codex 客户端按其回退约定重试 HTTP，而不是收到 405 后将握手失败
视为会话错误。服务端不创建 WebSocket 连接，也不保存 WebSocket 连接内状态。

Codex memory prompt cache 的稳定 key 和 explicit breakpoint 逻辑位于共享的 HTTP Responses 请求
准备路径，不再为 WebSocket 入口保留分支。

## 后果

- 客户端不再占用 any2api 的长连接，连接生命周期、增量重建和 WebSocket 帧桥接代码被移除。
- 支持 WebSocket 的客户端必须接受 HTTP 426 并回退到 JSON/SSE；不实现该回退的客户端需要关闭其
  WebSocket 偏好或直接使用 HTTP 入口。
- 上游 TransportMode 仍只有 JSON 和 SSE，普通 HTTP Responses、远程压缩和 memory 请求共享同一
  执行路径。
