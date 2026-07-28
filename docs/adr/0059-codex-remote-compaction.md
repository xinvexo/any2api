# ADR-0059：Codex 远程压缩识别与长时执行预算

- 状态：Accepted
- 日期：2026-07-28
- 决策者：maintainer

## 背景

Codex CLI 当前稳定且默认启用 `remote_compaction_v2`。该路径不会调用旧的
`POST /responses/compact`，而是把完整历史放入普通 Responses `input`，在数组末尾追加
`{"type":"compaction_trigger"}`，再通过流式 `POST /responses` 等待压缩事件。Codex
`rust-v0.145.0` 的 Provider 默认流空闲时长为 300 秒；旧 `/responses/compact` 是 unary 请求，
客户端把同一时长乘以 4，形成 1200 秒完整响应 timeout。

any2api 原先把现代压缩当作普通 Responses：等待响应头 15 秒、首事件 5 秒、提交前总预算
20 秒、提交后空闲 60 秒。旧 Compact 也只得到 15/20 秒预算。真实长上下文压缩的首事件可能在
一分钟以后才出现，因此正常上游会被 any2api 提前终止。单纯全局提高这些设置会无必要地放宽
全部文本请求；把 `compaction_trigger` 翻译成 Chat Completions 又没有无损语义。

## 决策

1. 协议稳定 API 在 `DecodedRequest` 中携带强类型 `RequestExecutionProfile`，首版包含
   `Standard` 与 `RemoteCompaction`。它是从已解析请求得到的旁路元数据，不进入 SQLite、管理
   DTO、Provider Driver 或 wire body。
2. OpenAI Responses 解码只在以下两种情况标记 `RemoteCompaction`：操作本身是
   `ResponsesCompact`；或操作是 `Responses`，且顶层 `input` 是数组、最后一项是对象并且其
   `type` 精确等于 `compaction_trigger`。不递归搜索用户内容，不因其他位置出现同名字符串而
   放宽预算。
3. `RemoteCompaction` 要求入口与上游方言相同。候选构造在选择 Credential、RPM 预留和上游
   I/O 前排除跨协议 Target；现有 Responses → Chat Completions Bridge 继续只负责能够无损表达的
   普通 Responses，不增加压缩兼容分支。
4. 现代 `/responses` 远程压缩继续走现有 Responses JSON/SSE、Header 投影、zstd、模型恢复和
   GuardedBody 链路。请求 JSON 与 `item.type=compaction` SSE 事件保持不透明直通，不新增 payload
   翻译、专用 Provider 分支或第二套流状态机。
5. Runtime 的单一 execution-limits 模块按操作和执行 profile 计算下限：
   - 现代 Responses 远程压缩：等待响应头/错误正文读取、首事件、提交后流空闲和提交前总预算
     均至少 300 秒；
   - 旧 Responses Compact：等待响应头、buffered body 每 chunk 空闲和提交前总预算均至少
     1200 秒；
   - 管理员配置大于下限时保留更大值，普通请求的 SettingRegistry 默认值不变。
6. 部署文档中的 Nginx 示例关闭响应缓冲，并把代理读写 timeout 设为至少 1200 秒。外部 CDN 或
   反向代理仍必须提供相同或更长的请求窗口；any2api 无法从应用内覆盖外层 502/504 timeout。

## 备选方案

- 全局把普通读取和 SSE timeout 提高到 1200 秒：扩大所有请求的资源占用与故障发现时间，拒绝。
- 为现代压缩注册新的公开路径或 `ProtocolOperation`：Codex CLI 的 wire 契约仍是
  `/responses`，会造成入口与真实客户端不一致，拒绝。
- 在 Runtime 递归搜索 JSON 或解析加密 compaction item：协议 Adapter 已有唯一解析边界，且响应
  内容应保持不透明，拒绝。
- 把 `compaction_trigger` 转为 Chat Completions system prompt：不能保证压缩结果、加密内容和
  Codex 后续历史语义，拒绝。
- 在首事件前向客户端发送代理 keepalive：这会提交下游响应并永久关闭安全切换上游的机会，拒绝。

## 后果

- 当前 Codex CLI 的默认远程压缩和旧 Compact 客户端都能跨过普通请求的短预算，同时仍有明确上界。
- 现代压缩只能使用 Responses 原生 Target；如果某模型只配置了 Chat Completions Bridge，候选为空并
  在上游发送前失败，不会静默降级。
- 普通 Responses、Chat Completions、Messages 和 Count Tokens 的现有故障发现速度保持不变。
- Nginx/CDN timeout 是独立部署边界；配置不足时仍可能在 any2api 正常等待期间由外层返回 502/504。

## 验证

- Protocol 单元测试覆盖最终 trigger 识别、非最终/嵌套同名值不误识别、旧 Compact 标记和 JSON
  原样保留。
- Runtime 单元测试覆盖 300/1200 秒下限、较大管理员值保持、普通请求和 Images 既有下限不变。
- 候选测试覆盖远程压缩排除 Responses → Chat Completions Bridge、普通 Responses 仍可使用该桥。
- Tokio 虚拟时间契约测试让现代压缩首事件晚于普通 5 秒预算、旧 Compact body 晚于普通 15/20 秒
  预算后到达，确认两者成功且 Transport 收到相应 read timeout。
- SSE 契约确认 `compaction_trigger` 请求仍发往 `/v1/responses`，并原样返回当前 Codex
  `response.output_item.done` compaction 事件。
