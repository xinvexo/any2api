# ADR-0054: OpenAI Images API 与媒体缓冲边界

- 状态：Accepted
- 日期：2026-07-27
- 修订：2026-08-03
- 决策者：maintainer

## 背景

标准 OpenAI SDK 的 `images.generate()` 和 `images.edit()` 使用 `POST /v1/images/generations` 与 `POST /v1/images/edits`，支持普通 JSON 响应和 SSE；生成请求使用 JSON，编辑请求既支持 JSON 图片引用，也支持 multipart 文件上传。GPT Image（包括 `gpt-image-2`）可返回较大的 base64 图片，复杂请求可能处理约两分钟，普通文本请求的 `32 MiB` 请求体、`16 MiB` buffered 响应、`16 MiB` SSE 硬上限和短预提交预算无法可靠承载。

Images 必须继续经过统一鉴权、模型路由、RPM、代理、健康、重试和遥测，不能另建媒体调度器。Grok 图片接口的生成与编辑请求形状并不同时等价于 OpenAI Images 方言，不能为了接入一个操作而对整个 Provider 宣称错误能力。

## 决策

1. Images 使用独立 `ProtocolDialect::OpenAiImages`，稳定持久化值为 `openai_images`；`ImagesGenerations` 与 `ImagesEdits` 操作的稳定日志值分别为 `images_generations`、`images_edits`。
2. Server 注册 `POST /v1/images/generations` 与 `POST /v1/images/edits`。两条入口使用统一 Gateway API Key 中间件、`PublicRequestService`、PublishedSnapshot 和模型允许列表，但不参与会话粘性。Images 没有会话或续接语义，即使出现通用 Session Header 或 `conversation_id` 也始终按普通候选调度。本地错误使用 Images Adapter 的 OpenAI 兼容 envelope；最终上游非 2xx 按 ADR-0061 透明返回。
3. Images Adapter 对生成 JSON 和编辑 JSON 复用“保留未知字段、只替换 model”的策略。编辑 multipart 使用结构化解析器读取 Part，要求唯一 UTF-8 `model`，在解析边界按 Rust Unicode whitespace `trim` 得到非空规范名称并存入结构化 payload；Adapter 必须把同一规范值用于入口 `DecodedRequest.model`、模型策略与 Route 查找，禁止校验 trim 值却保留原始带空白路由值。第二个 `model` 无论内容是否相同都拒绝。可选 `stream` 必须为布尔文本；所有未知字段、重复 `image[]`、文件字节和安全 Part Header 进入结构化 payload，出站时以 multipart 重新编码并替换模型，禁止搜索或原地修改二进制 Body。
4. multipart 解析需要异步消费，因此 `ProtocolAdapter::decode_ingress_request` 是异步对象安全接口；Axum Handler 不复制 Images 解析规则。
5. Codex/OpenAI API Key Driver 声明 `openai_images` JSON/SSE 能力及 `images/generations`、`images/edits` 固定后缀。Codex OAuthAccount 不支持该操作；Claude 与 Grok API Key/OAuth 均不声明 `openai_images`。
6. Images 继续使用现有同协议直通 Exchange，不注册图片跨协议 Bridge。一个 Provider Endpoint 只有一个接受方言，因此同时使用同一 OpenAI API Key 的文本和图片能力时配置两个 Endpoint/Credential 记录。
7. 聚合缓冲使用独立硬上限：multipart 编辑请求 `64 MiB`、Images buffered 成功 JSON `64 MiB`、Images 单个 SSE 帧及首个编码后事件 `64 MiB`。普通请求与响应继续保持 `32 MiB`、`16 MiB` 和现有可配置 SSE 预算，不能因图片能力整体放宽文本路径。
8. Images 的等待响应头、buffered body 空闲、流式首事件、提交后流空闲和提交前绝对预算取当前设置与 `180s` 的较大值。常量集中在单一 execution limits 模块；Provider、Server 和 Transport 不各自复制数字。
9. 普通 JSON 成功响应解析 usage，并像其他 OpenAI Adapter 一样只恢复顶层已知 `model` 为公开模型名；无需改写时保留上游 wire bytes。SSE 事件保留原始事件名与 base64 字段，只改写已知模型字段，并从 `image_generation.completed` 和 `image_edit.completed` 提取 `input_tokens`、`output_tokens`，不把图片事件标记为文本 content delta。最终上游非 2xx 响应仍透明返回状态、允许 Header 和有界正文。
10. 首版不实现 `/v1/images/variations`、Files API、Responses 图片工具桥接或管理 Web 图片工作台。
11. 公开请求不设置进程级总内存预算、加权内存 Semaphore、Permit 或基于预计工作集的本地 `429`。Body、buffered 响应和 SSE 只执行各自的单对象硬上限；实际工作集按真实对象所有权释放，不参与 Credential 选择或 RPM 准入。
12. multipart 首版仍完整缓冲以支持结构化校验、模型替换和安全重试，但解析时复用单分片 `Bytes`，重编码时按实际写入增长，避免可消除的整包中间复制。若未来要支持超过当前硬上限的媒体，迁移边界是 Transport 与协议 payload 共同支持可重放的临时文件/流式 multipart，而不是增加全局内存门槛。

## 备选方案

- 把 Images 塞进 `openai_responses`：会让 Route 能力与路径错误合并，也无法表达 multipart 编辑，拒绝。
- 只支持生成：标准 OpenAI Images API 和 SDK 编辑调用仍会失败，拒绝。
- 只透传原始 multipart：无法在 Route Target 使用不同上游模型时保持协议契约，也缺少结构化校验，拒绝。
- 全局提高已有文本缓冲和超时：会无必要地扩大所有推理请求的内存与等待边界，拒绝。
- 立即把 xAI 图片接口标记为 `openai_images`：xAI 编辑请求契约与 OpenAI multipart/JSON 组合不一致，会产生错误能力声明，拒绝。

## 后果

- 标准 OpenAI 客户端可通过 any2api 生成和编辑图片，并继续得到统一的凭据调度、代理与可观测性。
- 编辑请求仍被完整缓冲以支持重试与模型替换，但 `64 MiB` 单请求上限和真实对象生命周期限制单个请求的媒体缓冲；不设置与请求数量无关的全局内存门槛。
- ProtocolAdapter 的 ingress decode 是异步边界，所有 Adapter、fake 和契约测试遵守同一接口。
- 文本链路的默认缓冲与超时保持不变。

## 验证

- Domain/Storage 测试覆盖方言、操作稳定值和规范首版 Schema 对 Images 值的接受。
- Protocol 测试覆盖生成 JSON、编辑 JSON、multipart 多文件、模型替换、未知字段/Part 保留、畸形 boundary、缺失/重复 model、ASCII 与 Unicode 首尾空白规范化、纯 Unicode 空白拒绝、stream、普通 usage、两类完成事件 usage，以及 JSON/multipart 请求忽略会话标识。
- Provider 契约覆盖 Codex API Key 能力和两个固定路径，并确认 Codex OAuth、Claude、Grok 不产生 Images 候选。
- HTTP 契约使用本地上游覆盖生成/编辑 JSON、multipart、SSE、Gateway Key 剥离、Unicode 空白 model 使用规范名称路由、重复 model 在上游 I/O 前拒绝、模型改写、忽略会话标识并继续 Credential 轮询、错误透明返回、普通 `32 MiB` 与 Images `64 MiB` 请求边界、Images 大 JSON/SSE 响应和 `180s` 预算选择；并覆盖实际对象在取消后释放。
- 提交前运行相关 fmt、clippy、Rust 单元/契约测试、前端 typecheck/lint/build 与 embedded 资源校验。

本决策与 ADR-0073 的公开模型允许列表、ADR-0061 的上游错误透明返回和 ADR-0062 的统一固定会话绑定共同生效。

本 ADR 中“Images 不注册跨协议 Bridge”的范围已由 ADR-0117 扩展为 generation-only、buffered-only 的
Images → Chat Completions 兼容桥；原生 Images 直通、edits、SSE 与媒体缓冲边界保持不变。
