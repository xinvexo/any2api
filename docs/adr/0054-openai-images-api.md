# ADR-0054: OpenAI Images API 与媒体缓冲边界

- 状态：Accepted
- 日期：2026-07-27
- 决策者：maintainer

## 背景

any2api 已支持 OpenAI Responses 与 Chat Completions，但标准 OpenAI SDK 的 `images.generate()` 和 `images.edit()` 仍没有公开入口。OpenAI 当前 Images API 使用 `POST /v1/images/generations` 与 `POST /v1/images/edits`，支持普通 JSON 响应和 SSE；生成请求使用 JSON，编辑请求既支持 JSON 图片引用，也支持 multipart 文件上传。GPT Image（包括 `gpt-image-2`）可返回较大的 base64 图片，复杂请求可能处理约两分钟，现有 `32 MiB` 请求体、`16 MiB` buffered 响应、`16 MiB` SSE 硬上限和 5–20 秒预提交预算无法可靠承载。

Images 必须继续经过统一鉴权、模型路由、RPM、代理、健康、重试和遥测，不能另建媒体调度器。Grok 图片接口的生成与编辑请求形状并不同时等价于 OpenAI Images 方言，不能为了接入一个操作而对整个 Provider 宣称错误能力。

## 决策

1. 新增独立 `ProtocolDialect::OpenAiImages`，稳定持久化值为 `openai_images`；新增 `ImagesGenerations` 与 `ImagesEdits` 操作，稳定日志值为 `images_generations`、`images_edits`。
2. Server 注册 `POST /v1/images/generations` 与 `POST /v1/images/edits`。两条入口使用既有 Gateway API Key 中间件、`PublicRequestService`、PublishedSnapshot、模型允许列表和协议错误 envelope。
3. Images Adapter 对生成 JSON 和编辑 JSON 复用“保留未知字段、只替换 model”的策略。编辑 multipart 使用结构化解析器读取 Part，要求唯一非空 `model`，可选 `stream` 必须为布尔文本；所有未知字段、重复 `image[]`、文件字节和安全 Part Header 进入结构化 payload，出站时以 multipart 重新编码并替换模型，禁止搜索或原地修改二进制 Body。
4. multipart 解析需要异步消费，因此把 `ProtocolAdapter::decode_ingress_request` 改为异步对象安全接口；现有 Adapter 和协议契约测试一并迁移，不在 Axum Handler 中复制 Images 解析规则。
5. Codex/OpenAI API Key Driver 增加 `openai_images` JSON/SSE 能力及 `images/generations`、`images/edits` 固定后缀。Codex OAuthAccount 不支持该操作；Grok API Key/OAuth 首版均不声明 `openai_images`。
6. Images 继续使用现有同协议直通 Exchange，不注册图片跨协议 Bridge。一个 Provider Endpoint 只有一个接受方言，因此同时使用同一 OpenAI API Key 的文本和图片能力时配置两个 Endpoint/Credential 记录。
7. 聚合缓冲使用独立硬上限：multipart 编辑请求 `512 MiB`、Images buffered 成功 JSON `512 MiB`、Images 单个 SSE 帧及首个编码后事件 `128 MiB`。普通请求与响应继续保持 `32 MiB`、`16 MiB` 和现有可配置 SSE 预算，不能因图片能力整体放宽文本路径。
8. Images 的等待响应头、buffered body 空闲、流式首事件、提交后流空闲和提交前绝对预算取当前设置与 `180s` 的较大值。常量集中在单一 Images execution limits 模块；Provider、Server 和 Transport 不各自复制数字。
9. 普通 JSON 响应解析 usage，并像其他 OpenAI Adapter 一样只恢复顶层已知 `model` 为公开模型名；无需改写时保留上游 wire bytes。SSE 事件保留原始事件名与 base64 字段，只改写已知模型字段，并从 `image_generation.completed` 和 `image_edit.completed` 提取 `input_tokens`、`output_tokens`，不把图片事件标记为文本 content delta。
10. 首版不实现 `/v1/images/variations`、Files API、Responses 图片工具桥接或管理 Web 图片工作台。

## 备选方案

- 把 Images 塞进 `openai_responses`：会让 Route 能力与路径错误合并，也无法表达 multipart 编辑，拒绝。
- 只支持生成：标准 OpenAI Images API 和 SDK 编辑调用仍会失败，用户已明确要求保留编辑，拒绝。
- 只透传原始 multipart：无法在 Route Target 使用不同上游模型时保持协议契约，也缺少结构化校验，拒绝。
- 全局提高已有文本缓冲和超时：会无必要地扩大所有推理请求的内存与等待边界，拒绝。
- 立即把 xAI 图片接口标记为 `openai_images`：xAI 编辑请求契约与 OpenAI multipart/JSON 组合不一致，会产生错误能力声明，拒绝。

## 后果

- 标准 OpenAI 客户端可通过 any2api 生成和编辑图片，并继续得到统一的凭据调度、代理与可观测性。
- 编辑请求仍被完整缓冲以支持重试与模型替换，单请求最坏内存占用显著增大；有 Gateway 鉴权和 `512 MiB` 硬上限，但本切片不宣称支持无限数量的最大尺寸输入图。
- ProtocolAdapter 的 ingress decode 变为异步，现有 Adapter、fake 和契约测试需要一次性迁移。
- 文本链路的默认缓冲与超时保持不变。

## 验证

- Domain/Storage 测试覆盖方言、操作稳定值和规范首版 Schema 对 Images 值的接受。
- Protocol 测试覆盖生成 JSON、编辑 JSON、multipart 多文件、模型替换、未知字段/Part 保留、畸形 boundary、缺失/重复 model、stream、普通 usage 与两类完成事件 usage。
- Provider 契约覆盖 Codex API Key 能力和两个固定路径，并确认 Codex OAuth、Claude、Grok 不产生 Images 候选。
- HTTP 契约使用本地上游覆盖生成/编辑 JSON、multipart、SSE、Gateway Key 剥离、模型改写、错误 envelope、普通 32 MiB 与 Images 512 MiB 请求边界、Images 大 JSON/SSE 响应和 180 秒预算选择。
- 提交前运行相关 fmt、clippy、Rust 单元/契约测试、前端 typecheck/lint/build 与 embedded 资源校验。
