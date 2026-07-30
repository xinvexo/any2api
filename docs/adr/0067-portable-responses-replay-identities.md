# ADR-0067: OpenAI Responses 可重放 Item 身份归一化

- 状态：Accepted
- 日期：2026-07-30
- 决策者：maintainer

## 背景

OpenAI Responses 客户端可以把上一轮 `output` 中的完整 item 放回下一次请求的 `input`。其中
`id` 是上游生成的 item 身份，`call_id` 才是工具调用与工具输出之间的语义关联。兼容上游可能为
`reasoning`、`message` 或工具 item 返回通用 `item_*` ID，而严格的 Responses/Compact 上游会按
item 类型要求 `rs_*`、`msg_*`、`fc_*` 等前缀。

当 `affinity.enabled=false`、绑定过期或远程压缩选择另一个同方言执行目标时，原样重放这类 ID 会
让请求在模型执行前被拒绝。负载均衡开关不应决定一份完整历史是否具有合法 wire 形态，但 Runtime
也不能理解 Responses JSON，Provider Driver 更不能按 OAuth/API Key 分支修补正文。

`previous_response_id` 和 `item_reference.id` 与上述可省略身份不同：它们是显式服务器状态引用，
不能删除、改名或伪造。未知 item 类型也必须继续按同方言透传规则保留。

## 决策

1. OpenAI Responses Adapter 在入口 JSON 解析完成后、请求进入路由前，对顶层 `input` 数组执行一次
   可重放 item 身份归一化。`/v1/responses`、`/v1/responses/compact` 和 Responses v2 远程压缩共用
   同一规则；结果不依赖候选、Credential、OAuth、affinity 状态或重试次数。
2. 对已知的具体历史 item 类型，如果 `id` 是字符串但不具有该类型允许的非空前缀，则只删除该
   `id` 字段。不得通过字符串替换、拼接或生成新值伪造上游身份。当前已知映射覆盖 Codex/OpenAI
   Responses 使用的 message、reasoning、function/custom tool、tool search、local shell、web search、
   image generation、additional tools 和 compaction item。
3. 已具有允许前缀的 `id` 原样保留。`call_id`、`item_reference.id`、`previous_response_id`、
   `encrypted_content`、summary/content、未知 item 类型、嵌套对象中的 `id` 和其他未知字段全部原样
   保留。非字符串 `id` 不做类型强制或静默删除，继续由上游协议校验。
4. 归一化属于 ProtocolAdapter 的 wire 编解码职责。Runtime 只看到已经可重放的
   `AdapterPayload`，Provider Driver 不新增正文转换 API，候选和调度器不增加 Provider/OAuth 分支。
5. 该规则只保证携带完整 item 内容的手工历史能够跨同方言目标重放。显式服务器状态引用仍遵守
   既有固定绑定和 `session_binding_lost` 语义，不能借归一化变成可跨 Credential 状态。

本决策部分修订 ADR-0032 的“同协议原始 JSON 直通”和 ADR-0059 的“远程压缩请求 JSON 不透明”
表述：未知字段和内容仍保持不透明，只有顶层 `input` 中已知具体 item 的可省略 `id` 允许按上述规则
删除。

## 备选方案

- 强制开启 affinity：拒绝。管理员关闭普通 Session 粘性后仍应能正常使用完整历史和负载均衡。
- 只在 Codex OAuth 候选上修补：拒绝。这会把协议正文语义泄漏到 Credential 类型，并让同一请求的
  合法性取决于调度结果；同类错误也会出现在 API Key 的 Responses Compact 上游。
- 把 `item_*` 改名为 `rs_*` 或 `msg_*`：拒绝。前缀正确不代表该 ID 对目标上游真实存在，伪造身份
  会掩盖状态归属错误。
- 删除全部 `id`：拒绝。会误删 `item_reference.id` 等必要状态引用，也会无必要地改写已经合法的
  具体 item 身份。
- 把完整 Responses 请求转换为封闭 Canonical IR：拒绝。当前只需识别一个稳定的线协议边界；封闭
  IR 会丢失未知字段并使新增 Responses item 必须修改中央模型。

## 后果

- 关闭普通会话粘性、热启 OAuth 或执行远程压缩时，旧兼容上游产生的通用 item ID 不再使严格上游
  在推理前拒绝完整历史。
- 同协议路径不再是绝对逐字段透明；可观察差异仅限已确认不可移植且可省略的错误类型 `id`。
- 新增具有类型化 ID 的 Responses item 时，只需在 Responses 协议模块扩展映射和枚举测试，不修改
  Runtime、Provider 或调度器。

## 验证

- Protocol 单元测试枚举全部已知类型，覆盖错误前缀删除、允许前缀保留、空后缀、未知类型、
  `item_reference.id`、`call_id`、加密内容和嵌套 `id`。
- HTTP 契约测试覆盖 `affinity.enabled=false` 时同一 Session 在两个 Credential 间轮询，两次出站
  请求都使用相同的归一化历史。
- OAuth 路由契约确认固定 ChatGPT Codex 数据面收到归一化历史，同时认证、模型和其他正文不变。
- Responses Compact 契约覆盖 `message id=item_*` 的真实回归形态。
