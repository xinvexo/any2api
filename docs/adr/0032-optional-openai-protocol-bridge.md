# ADR-0032: 可选 OpenAI 协议桥与有效上游协议

- 状态：Accepted
- 日期：2026-07-23
- 修订：2026-08-03
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

Grok CLI 0.2.118 的真实长会话 recap 还会发送 `prompt_cache_key`、带 `detail` 的 `input_image`，
以及多组 `function_call` / `function_call_output` 历史。`prompt_cache_key` 在当前 Responses 与 Chat
Completions 中都是同名缓存路由提示；真实兼容上游探测也确认该字段可直接接收。图片 `detail` 的
`auto`、`low`、`high`、`original` 在两种协议中含义相同。Chat 历史则要求一次 assistant 消息声明
同轮全部 tool calls，并让对应 tool output 紧随其后。CLIProxyAPI 对连续调用合并和邻接处理可作为
行为参考，但其无条件忽略未知字段的宽松策略不进入 any2api。

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
- Bridge 合成的每个 Responses SSE JSON 事件必须包含 `sequence_number`。单个流从 `response.created=0` 开始，按实际发送顺序连续递增直到 `response.completed` 或 `response.incomplete`；编号由流转换器统一完成，不能散落到 reasoning、文本或工具调用分支。各分支只生成结构化事件；统一注入序号后才执行唯一一次 JSON/SSE 序列化，不先生成随后会被丢弃的字节。原生 Responses 直通流保持上游事件不变。
- 合成 reasoning summary 时，单个 `summary_index=0` part 的生命周期固定为
  `response.reasoning_summary_part.added` → 零或多个 `response.reasoning_summary_text.delta` →
  `response.reasoning_summary_text.done` → `response.reasoning_summary_part.done` →
  `response.output_item.done`。`part.done` 必须携带完整 `{type:"summary_text", text}`，并与其他事件一起
  由统一流状态分配连续 `sequence_number`；正常完成不伪造可选的 `status`。
- Bridge 合成的 Response ID 使用 `resp_*`；output 中的 message、reasoning、function call item ID
  分别使用 `msg_*`、`rs_*`、`fc_*`。流式 item 从 added、delta、各级 done 到最终完整 item 必须复用
  同一 ID。合法 item ID 回传为下一轮 Responses input 时必须被 ADR-0067 的归一化原样保留，不能依赖
  归一化删除本地生成的错误前缀。
- buffered 与流式响应共享唯一 finish-reason 映射：Chat `length` 对应 Responses
  `incomplete_details.reason=max_output_tokens`，Chat `content_filter` 对应
  `incomplete_details.reason=content_filter`；两者的 Response status 都是 `incomplete`，SSE 终止事件
  都是 `response.incomplete`，但 `StreamTermination` 仍按成功协议终止结算。`content_filter` 不是
  `completed`，也不是 Bridge 伪造的 `error` / `response.failed`。
- Bridge 只允许两项经过审计的输出投影降级：
  - `reasoning.summary` 仅接受 `auto`、`concise`、`detailed`。`reasoning.effort` 仍映射为 Chat `reasoning_effort`；summary 不伪造成上游控制字段，Chat 返回的 `reasoning_content` 或 `reasoning` 继续转换为 Responses reasoning summary。summary 可以在没有 effort 时单独出现。
  - `include` 仅接受空数组或 `reasoning.encrypted_content`。跨协议续接由 any2api 的本地 continuation 状态承担，Bridge 不伪造 Chat 上游不存在的不透明 reasoning 内容，也不把该 include 值发送上游。
  - 其他 `reasoning` 子字段、其他 `include` 值和任何未登记字段仍 fail-closed。该例外必须保持集中、可测试，不能扩张成通用未知字段过滤器。
- Responses 与 Chat Completions 具有可靠同义字段时优先等价投影，不把它们误归为未知字段：
  - `prompt_cache_key` 必须是字符串并原值写入 Chat 请求；它只影响上游缓存路由，不进入 Provider、Runtime 或会话分支，也不由 Bridge 生成或改写。
  - 用户消息 `input_image.detail` 缺省时保持缺省；非空时只接受 `auto`、`low`、`high`、`original` 并写入 Chat `image_url.detail`。其他值在上游提交前 fail-closed，禁止静默退回默认清晰度。
- Responses 历史中的连续 `function_call` 必须合并为同一条 Chat assistant `tool_calls` 消息；前置 reasoning summary 若存在则附着到该 assistant 消息。只要同一输入中存在对应 `function_call_output`，任何夹在调用与输出之间的普通消息都必须暂存到相关 tool output 之后，保持严格的 `assistant(tool_calls) -> tool` 邻接。当前 input 中每个 `function_call.call_id` 都必须在同一 input 的唯一 `function_call_output.call_id` 集合中存在；缺失 output 的中断 call 在构造上游请求前 fail-closed。只有 output 的当前 input 可以响应 continuation 已保存的上一轮 assistant call，因此不反向要求每个 output 在当前 input 重复 call。该规则按 `call_id` 工作且不依赖 Provider 类型；缺失、重复或无法表示的调用身份继续 fail-closed。
- Chat Completions 流式 `delta.tool_calls[]` 的非负整数 `index` 是跨 chunk 关联工具调用片段的唯一可靠键。缺失、负数或非整数必须在该 SSE 事件处 fail-closed；Bridge 禁止默认成 `0`、按出现顺序补号或继续生成可被误认为合法的 Responses function call。失败流不能转为 Ready continuation，Runtime 按既有 Guard/Lease 错误路径清理 Pending 状态。
- 流结束时，每个出现过的 tool-call `index` 都必须已累积出非空 `id` 与 `name`，并已建立对应
  function-call output item。Bridge 在任何工具完成事件和 continuation 结算前先统一验证全部状态；若
  当前 chunk 已携带 `finish_reason`，验证必须发生在返回该 chunk 的已构造事件之前，以保留提交前报错
  机会。残缺调用不得被跳过，也不得伪造 `call_id`、name、arguments 或成功 output；未提交时 Runtime
  返回本地上游错误，已提交时 Body 以错误终止且不切换 Attempt，continuation 始终保持 Pending/Abort。
- Chat 兼容上游不含 `choices` 的流事件只接受两种明确形状：带对象型 `error` 的事件转换为官方 Responses 顶层 `error` 事件，从上游 envelope 投影真实 `message`、`code`（缺失时回落到 `type`）和字符串/空 `param`，并标记 `StreamTermination::Failed`；带非空对象型 `usage` 的尾包只合并遥测。其他缺少 `choices` 的 JSON 继续 fail-closed。Bridge 不为错误伪造完整 `response.failed` 对象或成功 continuation；Runtime 允许失败终止 Abort Pending Lease，只有成功终止要求 continuation 已 Ready。
- buffered Chat 响应采用响应侧向前兼容解析：仍要求唯一 choice、对象型 message、assistant role，并严格转换 content、reasoning 与 `tool_calls`；message 中其余未知扩展字段不参与投影并被忽略，使 `annotations`、`reasoning_details` 和未来纯元数据字段不会使整次响应失败。已知但具有当前桥无法表达语义的非空 legacy `function_call`、`refusal`、`audio` 必须继续 fail-closed。该规则不得反向放宽客户端 Responses 请求白名单。
- 上述转换完全由 `ProtocolBridge` 按协议对实现。Provider Driver、Runtime 调度器和管理模型不感知这些字段，也不为任何 Provider 维护第二套转换规则。
- Responses → Chat 的 `previous_response_id` 使用本地合成 ID 和有界内存对话历史；该历史以强类型不透明状态与 Credential、Route Target、上游模型和协议对原子保存在统一会话绑定记录中，Protocol 不维护独立 History 索引。Pending/Ready/Abort 与状态字节上限由 ADR-0076 进一步收敛。任一状态过期或重启后都返回 `session_binding_lost`，不持久化、不恢复、不猜测。
- Responses 顶层 `instructions` 只对当前请求生效。Bridge continuation 只保存 `input` 与 assistant 输出，不保存顶层 `instructions`；每轮当前指令只在 Chat messages 首部注入一次，上一轮指令不得随 `previous_response_id` 继承或被追加到历史中段。显式 `input` 内的 system/developer message 仍是普通对话项并随历史续接。
- Base URL 仍是管理员填写的受信任 HTTP(S) 目标。Provider Driver 仅根据有效上游协议结构化追加 `/responses` 或 `/chat/completions`，不改变 authority、不增加 HTTP/内网授权开关。
- `/v1/responses/compact` 不桥接到 Chat Completions；Codex/OpenAI 与 Claude Messages 的双向转换不在本决策范围。

## 备选方案

- 强制同时填写接受协议和上游协议：重复保存同协议事实，增加 UI 噪音，也违背“内部转换可选”的管理语义。
- 只在 Provider Endpoint 保存实际上游协议、另建手工 Route 页面选择入口协议：理论上更规范，但当前模型 Route 是由 Credential 模型选择自动物化；为一个字段引入完整手工路由控制面会扩大产品复杂度。
- 在 Responses Adapter 内按 Provider 分支调用 Chat：会把 Provider、协议和转换逻辑耦合，并继续让 Runtime 误以为只有一个方言，无法正确固定路径、遥测和粘性。
- 通用静默删除未知 Responses 字段：会产生看似成功但语义改变的请求，不可接受。经过 ADR 明确登记、只影响附加输出投影且不改变模型执行的窄降级不属于通用未知字段过滤。
- 跳过身份残缺的流式工具调用：拒绝。它会把模型明确请求的动作静默删除，并把截断响应伪装成可以
  建立 continuation 的成功结果；客户端无法据此决定是否安全继续。
- 为孤立 `function_call` 合成占位 tool output：拒绝。Bridge 无法知道工具是否执行、失败或被用户
  取消；任何占位内容都会把虚构结果送入模型并污染后续 continuation。不可表达历史必须在上游 I/O
  前显式失败。

## 后果

- 管理员创建 Provider 时只需选择接受协议；只有兼容上游协议不同时才额外选择内部转换协议。
- Runtime Attempt 必须同时持有入口 Adapter、有效上游 Adapter 和可选 Bridge，流式提交状态机与运行态 Guard 生命周期保持不变。
- Chat Completions 没有服务端 Response 状态，桥接多轮会增加有界内存占用；该状态遵守现有“进程重启全部清空”边界。

## 验证

- Domain/Storage 测试覆盖空值回退、相同值归一化和无 Bridge 组合拒绝。
- Protocol 契约覆盖 Responses 请求、JSON、任意字节切分 SSE、CRLF、多行 data、工具调用、usage、错误、不支持字段、所有合成事件连续的 `sequence_number`、统一编号路径每个事件只序列化一次且精确 SSE 字节不变，以及实际客户端使用的 `reasoning.summary`、`include=["reasoning.encrypted_content"]`、`prompt_cache_key`、图片 detail、连续工具调用合并与严格输出邻接；中断历史中的单个孤立 call 以及多 call 仅部分有 output 都必须在上游 I/O 前失败，禁止占位 output；reasoning summary 必须精确覆盖 part added/text delta/text done/part done/item done 顺序和完整 part；buffered 与流式的 `length`/`content_filter` 必须分别生成一致的 incomplete reason；buffered 与流式合成项必须使用合法类型前缀，流式生命周期 ID 完全一致，完整 output 回传后仍保留 ID；缺失或非法 tool-call `index` 必须失败；只有 id 或只有 name 的残缺调用在 finish_reason 或 `[DONE]` 处也必须整体失败、不产生局部 done 且不得产生 Ready continuation；无 `choices` 的错误必须保留真实错误信息并失败终止，纯 usage 尾包必须安全消费；buffered 响应未知 message 字段必须兼容而已知不可表达输出仍失败；三轮以上续接还必须覆盖 `instructions` 的重复发送、替换和省略，证明它只出现在当前轮消息首部一次；HTTP 契约必须证明该行为不依赖 Provider 类型。
- Runtime/HTTP 契约覆盖直通不进入 Bridge、Responses -> Chat 路径、首字节后禁止切换、Guard 单次结算、合成 Response ID、多轮内存状态和重启后 `session_binding_lost`。
- Web 测试覆盖转换协议可留空、空值说明、按接受协议过滤可用转换目标和编辑回显。
