# ADR-0059：Codex 远程压缩识别与长时执行预算

- 状态：Accepted
- 日期：2026-07-28
- 决策者：maintainer
- 修订：ADR-0115

## 背景

Codex CLI 当前稳定且默认启用 `remote_compaction_v2`。该路径不会调用
`POST /responses/compact`，而是把完整历史放入普通 Responses `input`，在数组末尾追加
`{"type":"compaction_trigger"}`，再通过流式 `POST /responses` 等待压缩事件。Codex
`rust-v0.145.0` 的 Provider 默认流空闲时长为 300 秒；`/responses/compact` 是 unary 请求，
客户端把同一时长乘以 4，形成 1200 秒完整响应 timeout。

远程压缩不能沿用普通 Responses 的等待响应头、首事件、提交前总预算和提交后空闲限制；unary
Compact 也需要独立长时预算。真实长上下文压缩的首事件可能在一分钟以后才出现，普通请求预算
会提前终止正常上游。单纯全局提高这些设置会无必要地放宽全部文本请求；把
`compaction_trigger` 翻译成 Chat Completions 又没有无损语义。

Responses 流必须以显式终止事件而不是 HTTP Body EOF 作为成功边界。CLIProxyAPI 的 Codex
executor 和 Codex `rust-v0.145.0` 客户端都在终止事件到达后立即停止读取，EOF 先到则视为
不完整流。any2api 若在终止事件后继续等待保持开启的上游连接，会让下游请求生命周期错误延长
到 300 秒空闲超时；反过来，终止事件缺失的 EOF 又会被错误记录为成功。

远程压缩还会把不透明加密结果集中放在单个
`response.output_item.done` 事件中。普通流默认的 `256 KiB` 预提交/单帧上限同时作用于
整个解码器，因此大型压缩帧无论是首个事件，还是在 `response.created` 后到达都会被中断。
前者在返回下游响应头前变成 502；后者因 HTTP 状态和部分 Body 已经提交，客户端只能
观察到低层 response body 解码失败。

## 决策

1. 协议稳定 API 在 `DecodedRequest` 中携带强类型 `RequestExecutionProfile`，包含
   `Standard` 与 `RemoteCompaction`。它是从已解析请求得到的旁路元数据，不进入 SQLite、管理
   DTO、Provider Driver 或 wire body。
2. OpenAI Responses 解码只在以下两种情况标记 `RemoteCompaction`：操作本身是
   `ResponsesCompact`；或操作是 `Responses`，且顶层 `input` 是数组、最后一项是对象并且其
   `type` 精确等于 `compaction_trigger`。不递归搜索用户内容，不因其他位置出现同名字符串而
   放宽预算。
3. `RemoteCompaction` 要求入口与上游方言相同。候选构造在选择 Credential、RPM 预留和上游
   I/O 前排除跨协议 Target；现有 Responses → Chat Completions Bridge 继续只负责能够无损表达的
   普通 Responses，不增加压缩兼容分支。
4. 现代 `/responses` 远程压缩继续走现有 Responses JSON/SSE、Header 投影、入口 zstd 解压、identity 上游正文、模型恢复和
   GuardedBody 链路。除 ADR-0067 规定的顶层 `input` 可重放 item 身份归一化，以及 ADR-0115 在最终选中
   Codex OAuth Attempt 后执行的已登记顶层兼容 Profile 外，请求 JSON 与 `response.output_item.done` 中的
   远程压缩项保持不透明直通，不按 `compaction`、
   `compaction_summary` 或其他类型名新增 payload 翻译、专用 Provider 分支或第二套流状态机。
5. OpenAI Responses Adapter 通过协议稳定 API 把 `response.completed` 和 `response.incomplete`
   标记为成功终止，把 `response.failed` 和顶层 `error` 标记为失败终止。GuardedBody 先交付终止
   帧，再立即结束下游 Body 并停止读取上游；成功终止记为成功，失败终止记为上游流错误，终止
   事件前 EOF 是不完整上游流。终止元数据是通用协议生命周期信息，Runtime 不解析 compaction
   内容，也不增加 Codex Provider 分支。
6. 不从 `response.output_item.done` 重建 `response.completed.response.output`。Codex v2 客户端直接
   从前者收集远程压缩项，后者只提供 response id 与 usage；保持 payload 不透明比 CLIProxyAPI
   面向其他客户端的兼容性补全更符合本项目边界。
7. Runtime 的单一 execution-limits 模块按操作和执行 profile 计算下限：
   - 现代 Responses 远程压缩：等待响应头/错误正文读取、首事件、提交后流空闲和提交前总预算
     均至少 300 秒；SSE 单帧与提交前字节上限至少 `64 MiB`，普通 Responses 仍使用设置值；
   - unary Responses Compact：等待响应头、buffered body 每 chunk 空闲和提交前总预算均至少
     1200 秒；
   - 管理员配置大于下限时保留更大值，普通请求的 SettingRegistry 默认值不变。
8. 部署文档中的 Nginx 示例关闭响应缓冲，并把代理读写 timeout 设为至少 1200 秒。外部 CDN 或
   反向代理仍必须提供相同或更长的请求窗口；any2api 无法从应用内覆盖外层 502/504 timeout。

## 备选方案

- 全局把普通读取和 SSE timeout 提高到 1200 秒：扩大所有请求的资源占用与故障发现时间，拒绝。
- 为现代压缩注册新的公开路径或 `ProtocolOperation`：Codex CLI 的 wire 契约仍是
  `/responses`，会造成入口与真实客户端不一致，拒绝。
- 在 Runtime 递归搜索 JSON 或解析加密 compaction item：协议 Adapter 已有唯一解析边界，且响应
  内容应保持不透明，拒绝。
- 全局放大普通 Responses 的 SSE 单帧上限：不必要地放大所有文本流的内存边界，拒绝。
- 聚合 `response.output_item.done` 并补写最终 `response.output`：当前 Codex v2 不读取该字段，且会
  把生命周期修复扩张为 payload 翻译，拒绝。
- 把 `compaction_trigger` 转为 Chat Completions system prompt：不能保证压缩结果、加密内容和
  Codex 多轮上下文语义，拒绝。
- 在首事件前向客户端发送代理 keepalive：这会提交下游响应并永久关闭安全切换上游的机会，拒绝。

## 后果

- 当前 Codex CLI 的流式远程压缩和 unary Compact 请求都能跨过普通请求的短预算，同时仍有明确上界。
- 大型加密压缩项不再被普通文本流的 `256 KiB` 单帧默认值截断；额外内存上限只对精确识别的远程压缩请求生效。
- Responses 终止帧成为真实完成边界；保持开启或在终止后报错的上游 Body 不会拖住客户端，缺少
  终止帧的提前 EOF 也不再伪装成成功。
- 现代压缩只能使用 Responses 原生 Target；如果某模型只配置了 Chat Completions Bridge，候选为空并
  在上游发送前失败，不会静默降级。
- 普通 Responses、Chat Completions、Messages 和 Count Tokens 的现有故障发现速度保持不变。
- Nginx/CDN timeout 是独立部署边界；配置不足时仍可能在 any2api 正常等待期间由外层返回 502/504。

## 验证

- Protocol 单元测试覆盖最终 trigger 识别、非最终/嵌套同名值不误识别、unary Compact 标记和 JSON
  原样保留。
- Runtime 单元测试覆盖 Images 的 180 秒、远程压缩的 300/1200 秒下限与 `64 MiB` 单帧下限、较大管理员值保持和普通请求默认预算不变。
- 候选测试覆盖远程压缩排除 Responses → Chat Completions Bridge、普通 Responses 仍可使用该桥。
- Tokio 虚拟时间契约测试让流式压缩首事件晚于普通 5 秒预算、unary Compact body 晚于普通 15/20 秒
  预算后到达，确认两者成功且 Transport 收到相应 read timeout。
- SSE 契约确认 `compaction_trigger` 请求仍发往 `/v1/responses`，并原样返回当前上游的
  大于普通单帧默认值的首个 `response.output_item.done` 远程压缩事件。
- 流生命周期测试覆盖终止事件后上游永久 pending、终止事件后的传输错误、终止事件前 EOF，以及
  compaction item 与终止帧的原样、有序交付。
