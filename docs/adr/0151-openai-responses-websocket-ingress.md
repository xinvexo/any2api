# ADR-0151：OpenAI Responses WebSocket 入口

- 状态：Accepted
- 日期：2026-08-15
- 决策者：maintainer
- 修订：`ARCHITECTURE.md` 需求 16、§3.2 首批范围外清单与 §11.2 支持矩阵中"不接受 WebSocket Upgrade"的表述

## 问题

Codex CLI 自 0.147 起为 `prefer_websockets` 模型目录条目（当前 `gpt-5.6` 系列）与
`supports_websockets = true` 的 Provider 默认使用 `responses_websocket` 传输：对
`/v1/responses` 同路径发起 WebSocket Upgrade（`OpenAI-Beta: responses_websockets=2026-02-06`），
在同一连接上以 `{"type":"response.create",...}` 文本消息发起请求，并以 SSE 等价的 JSON 事件消息
接收流式响应。客户端只有在 Upgrade 握手收到 **HTTP 426** 时才干净回退 HTTP 传输；其余握手失败
（包括 any2api 现状对 `GET /v1/responses` 返回的 405）都是会话内硬错误。

该传输还带有两个连接内语义，代理若不理解就无法正确服务：

1. **增量请求**：客户端在复用连接且新请求是上一次逻辑请求的严格扩展时，只发送
   `previous_response_id` + 新增 `input` 项；完整输入 = 上一请求 `input`
   + 上一响应 `response.output_item.done` 项 + 新增项。基线只在 `response.completed`
   后建立；服务端状态缺失时以 `previous_response_not_found` 结构化错误触发客户端全量重试。
2. **warmup**：`generate: false` 的 `response.create` 只上传前缀、不生成内容，客户端等待
   `response.completed` 并把其 `response.id` 用作下一次增量请求的 `previous_response_id`。

## 决策

1. `POST /v1/responses` 同路径接受 `GET` WebSocket Upgrade，进入 OpenAI Responses WebSocket
   入口。Upgrade 经过与 HTTP 完全相同的中间件栈：HttpAccessLog 记录、客户端地址解析、
   Gateway API Key 认证与凭据头剥离。WebSocket 入口常开，不新增设置项；操作侧回退手段是
   客户端配置（`supports_websockets = false`）。并发连接数达到硬上限（64）时对新 Upgrade 返回
   **426**，让客户端按协议降级 HTTP 而不是硬失败。
2. 连接内每条 `response.create` 消息还原为一个独立的公开请求，走既有
   `PublicRequestService::execute` 全链路：模型允许列表、Route、RPM 预留、会话粘性、重试、
   Responses→Chat Bridge、请求日志与健康结算全部复用，不新增第二套调度。上游传输保持
   HTTP JSON/SSE（`TransportMode` 不变）；上游 WebSocket 传输是明确非目标（见"边界"）。
3. 还原规则：剥离 WebSocket 专属字段——顶层 `type`、`previous_response_id`、`generate`，
   `client_metadata` 中的 `x-codex-turn-state`（映射为同名请求头，交由既有 BoundTurnState
   投影）与 `x-codex-ws-stream-request-start-ms`（传输计时键，丢弃）；`client_metadata` 其余
   键保留在请求体内（与 Codex HTTP 基线一致）。`client_metadata.session_id`/`thread_id`
   同时映射为 `session-id`/`thread-id` 请求头，使会话粘性与上游会话头投影与 HTTP 入口一致。
   还原后的上游请求不携带 `previous_response_id`，是凭据无关的全量无状态请求，与 ADR-0149
   的缓存连续请求面一致。
4. 增量状态按连接维护在内存：上一逻辑请求 JSON、流经的 `response.output_item.done` 项与
   `response.completed` 的 `response.id`。只有 `response.completed` 终止才更新基线；失败、
   不完整、取消与传输错误都清空状态。`previous_response_id` 与状态不匹配、进程重启或状态
   超限时，回复协议原生的 `previous_response_not_found` 包装错误，由客户端全量重试自愈。
   连接状态总量硬上限 32 MiB（与标准公开请求体上限对齐），超限即丢弃状态。
5. warmup（`generate:false`）在本地完成：按同样规则并入连接状态，回复合成的
   `response.created` + `response.completed`（连接内合成 `resp_any2api_` 前缀 ID）。该 ID
   只存在于连接状态，永不发往上游；warmup 不占用凭据、不产生上游 Attempt，因此不写入
   RequestLog，保持 ADR-0146 额度统计的样本真实性。
6. 流式桥接：runtime 返回的 egress SSE 字节流按既有 SSE 解码器还原帧，每个 `data:` JSON 载荷
   作为一条 WebSocket 文本消息下发（Responses 事件自描述，无需 SSE event 名）；注释/心跳帧
   不下发。终止事件后请求结束、连接保持等待下一条消息。上游或本地错误按协议包装为
   `{"type":"error","status":<HTTP 状态>,"error":<上游 error 对象或 OpenAI 错误体>,"headers":
   <经既有清洗的响应头>}` 文本消息，保留 ADR-0136 的精确拒绝保真；流已提交后发生的传输
   截断以 WebSocket Close 帧结束整条连接，不伪造终止事件。
7. 长连接跨配置代际：每条消息用当前 `PublishedSnapshot` 重新校验 Gateway API Key 仍然存在且
   启用（按 Upgrade 时验证过的密钥 ID），失效即回复 401 包装错误并关闭连接；路由、模型
   允许列表与设置照常按当前快照生效。
8. 资源生命周期：入站消息与还原后请求体沿用 32 MiB 硬上限；egress 帧重解析上限 64 MiB
   （与远程压缩单帧上限对齐）；连接空闲（无在途请求且无消息）30 分钟关闭；客户端断连或
   Close 立即取消在途上游请求，复用既有流式 Guard 一次性结算。以上均为集中常量，不进入
   SettingRegistry。

## 边界与非目标

- **上游 WebSocket 传输不在本决策内**：所有上游请求继续走 HTTP JSON/SSE。上游 WS 涉及
  线路 profile 固定（ADR-0126/0130/0131/0135 的 conformance fixture 面）、连接与凭据生命
  周期绑定、60 分钟上游连接上限镜像等独立问题，需要单独 ADR。
- `/v1/responses/compact` 保持 unary JSON；其余方言入口不接受 WebSocket Upgrade。
- 入口不协商 permessage-deflate 扩展（客户端 offer 被忽略是协议合法行为）。
- HttpAccessLog 只保留 Upgrade 握手交换；WebSocket 帧不进入 HttpAccessLog。每条桥接的模型
  请求仍按既有规则写入 RequestLog，可观测性以 RequestLog 为准。
- 不实现服务端主动 Ping 与上游式 60 分钟连接寿命上限；客户端 Ping 由读循环即时回应 Pong。

## 后果

- Codex CLI ≥0.147 对 any2api 直接以 WebSocket 工作；不支持 WS 的部署链路（如无 WS 的反向
  代理）下客户端收到非 426 握手失败仍是硬错误，需要在部署文档中说明。
- 增量与 warmup 语义由连接状态完整承接，上游请求面保持全量、凭据无关，prompt cache 连续性
  与 HTTP 入口一致；WS 带来的收益是客户端侧连接复用与增量上传，上游侧时延收益待上游 WS
  传输另行评估。
- 协议帧编解码与增量重建位于 `protocol` crate 并单独测试；`server` 只做 Axum WS 装配与
  桥接循环；`runtime`、`provider`、`transport`、`storage` 与前端零改动。
