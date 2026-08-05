# ADR-0117：OpenAI Images 到 Chat Completions 图片上游桥

- 状态：Accepted
- 日期：2026-08-05
- 决策者：maintainer
- 修订：ADR-0032、ADR-0054

## 背景

any2api 已经提供标准 `POST /v1/images/generations` 与 `POST /v1/images/edits`，但原实现只支持
Images → Images 直通。一类 OpenAI 兼容上游把图片模型挂在 `POST /v1/chat/completions`：请求仍使用
Chat `messages`，成功响应则在 `choices[].message.content` 中返回 Markdown 图片链接。

2026-08-05 对当前管理员已配置的 `gpt-image-2` 上游做了脱敏探测：非流式 Chat 请求返回标准
Chat Completions 外壳、唯一 assistant choice 和一个 HTTPS Markdown 图片链接；响应不是
`message.images`，也不是 base64。探测只记录字段、类型和长度，不保存或展示 Provider API Key、完整
URL 或图片正文。

OpenAI 官方 Image API 的入口仍是 `/v1/images/generations` / `/v1/images/edits`。GPT Image 的官方
非流式结果通常是 `b64_json`，流式协议发送 base64 partial/completed 事件。协议层若为了适配 URL 结果
自行下载图片，会越过现有 Transport、代理、严格 SSRF、超时和重试边界；把 URL 伪装成 base64 或官方
partial event 也会破坏客户端契约。

## 决策

1. 静态注册 `OpenAI Images → OpenAI Chat Completions` Bridge；Runtime、Provider Driver、Storage 和
   Web 只通过现有 `ProtocolRegistry` 与能力目录发现它，不增加 Provider 类型分支。
2. Bridge 只声明 `ProtocolOperation::ImagesGenerations`。`ImagesEdits` 继续只允许原生 Images 上游，
   因为 Chat 文本消息不能无损表达 multipart 图片、mask 和编辑语义。
3. 首版只支持非流式 JSON。`stream=true`、非零 `partial_images` 在 RPM 预留和上游 I/O 前失败；Bridge
   不合成非标准 URL 图片 SSE，也不把完整 Chat 流缓冲成伪流。
4. 请求必须包含非空字符串 `prompt`。Bridge 构造
   `messages=[{"role":"user","content":prompt}]`，把最终 Route Target 模型写入 Chat `model`，并固定
   `stream=false`。
5. 以下 Images 生成选项作为已登记的 Chat 图片上游扩展字段原值转发：`background`、`moderation`、
   `n`、`output_compression`、`output_format`、`quality`、`size`、`style`、`user`。`n` 缺省为 1，显式值
   必须是 `1..=10` 的整数；成功响应必须返回相同数量、index 唯一的 choices。其他未知请求字段在上游
   I/O 前 fail-closed。
6. `response_format` 缺省或等于 `url` 时接受，但不发送到 Chat，因为 Chat 的同名字段具有结构化文本的
   不同语义；`b64_json` 及其他值明确拒绝。`partial_images=0` 可接受并从 Chat 请求移除，其他值拒绝。
7. 每个成功 choice 必须满足：非负整数 `index`、`finish_reason=stop`、`message.role=assistant`，且字符串
   `message.content` 精确包含一个 HTTP(S) 裸 URL 或一个 Markdown 图片。Markdown 的 alt 文本不进入
   Images 响应；图片表达式之外只允许空白。缺失、多个图片、普通文本、data URL、非 HTTP(S) URL、重复
   index 或不完整 finish reason 都视为无效上游成功响应，禁止返回部分成功。
8. Bridge 按 choice index 排序并生成标准 Images 成功 envelope：
   `{"created":...,"data":[{"url":"..."}]}`。它不主动访问、缓存或改写图片 URL，不生成
   `b64_json`、`revised_prompt` 或虚假图片元数据。
9. Chat `prompt_tokens` / `completion_tokens`（兼容上游的 `input_tokens` / `output_tokens` 回落字段）映射
   为 Images `input_tokens` / `output_tokens`，`total_tokens` 使用经过校验的上游值或安全求和。遥测继续
   使用 Chat Adapter 已解析的同一 usage，不在 Runtime 重复解析 JSON。
10. 最终上游非 2xx 继续按 ADR-0061 原样返回；只有 2xx Chat 成功正文进入 Bridge。客户端认证头仍在
    Provider Driver 前剥离，只有调度器选中的 ProviderCredential 可以注入上游认证。
11. 任一声明 Chat Completions 上游能力的 Provider Driver 都可以通过该协议对形成 Images 接受选项；
    能否实际生成图片由管理员选择的上游模型和上述响应契约决定。不得按 Codex、Grok 或某个 Base URL
    硬编码例外。

## 备选方案

- 要求上游改成 `/images/generations`：最纯粹，但无法使用已经存在的 Chat 图片兼容服务。
- 在 Images Adapter 内按 Base URL 或模型名调用 Chat：会把协议、Provider 和部署配置耦合，拒绝。
- 下载 Markdown URL 再返回 `b64_json`：需要一次新的受控网络 Attempt、代理选择、SSRF、大小、超时和
  重试语义，不是协议投影可以安全承担的工作，首版拒绝。
- 把 URL 写进 `b64_json` 或流式 completed 事件：客户端会把 URL 当作 base64 解码，拒绝。
- 静默删除未知 Images 字段：会让尺寸、质量或输出格式看似成功但不生效，拒绝。

## 后果

- 标准 OpenAI SDK/HTTP 客户端可以继续调用 `/v1/images/generations`，而管理员把 Endpoint 的接受协议
  设为 OpenAI Images、内部转换协议设为 OpenAI Chat Completions。
- 该兼容路径返回标准 `data[].url` 变体，不承诺 GPT Image 官方上游的 base64-only 行为；需要
  `b64_json`、SSE partial image 或 edits 的客户端必须配置原生 Images 上游。
- 管理面协议选项由现有能力目录自动出现；Codex/Grok 等 Provider 不需要图片专用代码。

## 验证

- Protocol 单元测试覆盖请求投影、已登记字段原值转发、未知字段、`response_format`、`partial_images`、
  stream 和 edits 的 fail-closed 边界。
- buffered 响应测试覆盖 Markdown/裸 URL、多 choice 排序、usage、重复 index、普通文本、多图片、
  非 HTTP(S) URL、非 stop finish reason 和无效成功正文。
- Registry 契约枚举两座真实 Bridge，并证明 Images → Chat 只支持 generations。
- 配置能力测试证明支持 Chat 的 Provider 自动出现 Images → Chat 选项，而不支持 Chat 的 Provider 不出现。
- 端到端 HTTP 契约证明 `/v1/images/generations` 调用上游 `/chat/completions`、恢复公开 envelope、剥离
  Gateway Key，并在上游 I/O 前拒绝不支持的请求。
