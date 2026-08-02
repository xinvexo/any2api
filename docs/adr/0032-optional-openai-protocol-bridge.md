# ADR-0032: 可选 OpenAI 协议桥与有效上游协议

- 状态：Accepted
- 日期：2026-07-23
- 修订：2026-08-02
- 决策者：maintainer

## 背景

Provider Endpoint 的 `protocol_dialect` 表示客户端入口协议，可选 `upstream_protocol_dialect` 表示实际上游协议。同协议请求由一个 `ProtocolAdapter` 贯穿，允许的跨协议组合由静态 `ProtocolBridge` 明确定义。

管理员需要把只支持 OpenAI Chat Completions 的兼容上游暴露为 Responses：客户端继续请求 `/v1/responses`，any2api 内部调用 `/v1/chat/completions`，再把 JSON 或 SSE 转回 Responses。管理员同时明确要求内部转换协议不是必选项；未选择转换时，上游协议必须自动等于接受协议。

Grok CLI 0.2.111 的实际 Responses 请求会同时发送 `reasoning.summary` 和
`include=["reasoning.encrypted_content"]`。两者都是 Responses 输出投影提示，不改变输入消息、
工具调用或模型采样；Chat Completions 没有同名请求字段。CLIProxyAPI 的同向转换器也只映射
`reasoning.effort`，并由返回的 Chat reasoning 内容构造 Responses reasoning summary。
该请求只是暴露问题的现实样本；规则属于协议对，适用于所有配置为 Responses -> Chat
Completions 的 Provider Endpoint，禁止按 `provider_kind` 增加 Grok 或其他 Provider 分支。

## 决策

- `ProviderEndpoint.protocol_dialect` 保留为必填的客户端接受协议。
- 新增可空的 `upstream_protocol_dialect`，管理界面命名为“内部转换协议（可选）”。其有效值定义为：

  ```text
  effective_upstream = upstream_protocol_dialect ?? protocol_dialect
  ```

- 空值表示严格同协议直通。数据库不重复保存与接受协议相同的值；管理 API 收到相同值时归一化为空值。
- 内部 ModelRoute 继续以接受协议作为 `ingress_protocol`；Route Target 保存已经解析的有效上游协议，使运行时、会话绑定和请求日志不依赖回查可变 Endpoint。
- 有效组合固定为 Responses -> Responses、Responses -> Chat Completions、Chat Completions -> Chat Completions、Images -> Images、Messages -> Messages。Images -> Images 是同协议直通，不注册新的跨协议 Bridge。
- 在 `ProtocolRegistry` 中按 `(ingress_dialect, upstream_dialect)` 静态注册 `ProtocolBridge`。协议相同走 Adapter 快速路径，不查找 Bridge；协议不同时必须在配置发布前找到 Bridge，否则拒绝发布。Domain、Runtime、Storage 和 Server 都不得为 Responses → Chat 写专用协议对分支；新增转换只增加 Bridge 实现、能力声明和 Composition Root 注册。
- 唯一注册的跨协议桥是 Responses -> Chat Completions。它负责请求、非流式响应、SSE、工具调用和 usage 转换；影响模型执行、工具语义、输出格式或续接语义且无法可靠表达的字段，在上游提交前 fail-closed，禁止静默丢弃。最终上游非 2xx 仍透明返回，不经 Bridge 重编码错误正文。
- Bridge 合成的每个 Responses SSE JSON 事件必须包含 `sequence_number`。单个流从 `response.created=0` 开始，按实际发送顺序连续递增直到 `response.completed` 或 `response.incomplete`；编号由流转换器统一完成，不能散落到 reasoning、文本或工具调用分支。原生 Responses 直通流保持上游事件不变。
- Bridge 只允许两项经过审计的输出投影降级：
  - `reasoning.summary` 仅接受 `auto`、`concise`、`detailed`。`reasoning.effort` 仍映射为 Chat `reasoning_effort`；summary 不伪造成上游控制字段，Chat 返回的 `reasoning_content` 或 `reasoning` 继续转换为 Responses reasoning summary。summary 可以在没有 effort 时单独出现。
  - `include` 仅接受空数组或 `reasoning.encrypted_content`。跨协议续接由 any2api 的本地 continuation 状态承担，Bridge 不伪造 Chat 上游不存在的不透明 reasoning 内容，也不把该 include 值发送上游。
  - 其他 `reasoning` 子字段、其他 `include` 值和任何未登记字段仍 fail-closed。该例外必须保持集中、可测试，不能扩张成通用未知字段过滤器。
- 上述转换完全由 `ProtocolBridge` 按协议对实现。Provider Driver、Runtime 调度器和管理模型不感知这些字段，也不为任何 Provider 维护第二套转换规则。
- Responses → Chat 的 `previous_response_id` 使用本地合成 ID 和有界内存对话历史；该历史以强类型不透明状态与 Credential、Route Target、上游模型和协议对原子保存在统一会话绑定记录中，Protocol 不维护独立 History 索引。Pending/Ready/Abort 与状态字节上限由 ADR-0076 进一步收敛。任一状态过期或重启后都返回 `session_binding_lost`，不持久化、不恢复、不猜测。
- Base URL 仍是管理员填写的受信任 HTTP(S) 目标。Provider Driver 仅根据有效上游协议结构化追加 `/responses` 或 `/chat/completions`，不改变 authority、不增加 HTTP/内网授权开关。
- `/v1/responses/compact` 不桥接到 Chat Completions；Codex/OpenAI 与 Claude Messages 的双向转换不在本决策范围。

## 备选方案

- 强制同时填写接受协议和上游协议：重复保存同协议事实，增加 UI 噪音，也违背“内部转换可选”的管理语义。
- 只在 Provider Endpoint 保存实际上游协议、另建手工 Route 页面选择入口协议：理论上更规范，但当前模型 Route 是由 Credential 模型选择自动物化；为一个字段引入完整手工路由控制面会扩大产品复杂度。
- 在 Responses Adapter 内按 Provider 分支调用 Chat：会把 Provider、协议和转换逻辑耦合，并继续让 Runtime 误以为只有一个方言，无法正确固定路径、遥测和粘性。
- 通用静默删除未知 Responses 字段：会产生看似成功但语义改变的请求，不可接受。经过 ADR 明确登记、只影响附加输出投影且不改变模型执行的窄降级不属于通用未知字段过滤。

## 后果

- 管理员创建 Provider 时只需选择接受协议；只有兼容上游协议不同时才额外选择内部转换协议。
- Runtime Attempt 必须同时持有入口 Adapter、有效上游 Adapter 和可选 Bridge，流式提交状态机与运行态 Guard 生命周期保持不变。
- Chat Completions 没有服务端 Response 状态，桥接多轮会增加有界内存占用；该状态遵守现有“进程重启全部清空”边界。

## 验证

- Domain/Storage 测试覆盖空值回退、相同值归一化和无 Bridge 组合拒绝。
- Protocol 契约覆盖 Responses 请求、JSON、任意字节切分 SSE、CRLF、多行 data、工具调用、usage、错误、不支持字段、所有合成事件连续的 `sequence_number`，以及实际客户端使用的 `reasoning.summary` 与 `include=["reasoning.encrypted_content"]`；HTTP 契约必须证明该行为不依赖 Provider 类型。
- Runtime/HTTP 契约覆盖直通不进入 Bridge、Responses -> Chat 路径、首字节后禁止切换、Guard 单次结算、合成 Response ID、多轮内存状态和重启后 `session_binding_lost`。
- Web 测试覆盖转换协议可留空、空值说明、按接受协议过滤可用转换目标和编辑回显。
