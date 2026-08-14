# ADR-0152：OpenAI Alpha Search 入口（`POST /v1/alpha/search`）

- 状态：Accepted
- 日期：2026-08-15
- 决策者：maintainer
- 修订：`ARCHITECTURE.md` §11.2 支持矩阵、错误 envelope 归属、会话粘性与模型允许列表清单

## 问题

Codex CLI 自 0.147 起内置 standalone web search 扩展工具（对模型暴露为 `web` 命名空间的
`web__run` 函数工具）。工具由客户端执行：Codex 直接向模型 Provider 发起
`POST {base_url}/alpha/search`，使用与模型请求相同的鉴权，请求体为
`{id, model, input?, commands?, settings?, max_output_tokens?}`（`id` 为 Codex 会话 UUID，
`input` 为最近对话尾部的 Responses `ResponseItem` 数组，`commands` 携带
`search_query`/`open`/`find`/`click` 等操作），响应为非流式 JSON
`{output, results?, encrypted_output?}`。

该工具在 `use_responses_lite` 模型目录条目（当前 `gpt-5.6` 系列）上是唯一的联网搜索通道：
这些模型不下发托管 `web_search` 工具，托管搜索无从代替。客户端启用条件是 Provider 配置
`supports_standalone_web_search = true` 且 `web_search_mode` 未禁用。搜索请求失败映射为
`FunctionCallError::Fatal`，直接终止整个回合；any2api 现状对该路径返回 404，等于在启用搜索的
配置下无法完成任何触发 `web__run` 的回合。

## 决策

1. 新增 `ProtocolOperation::AlphaSearch`（wire 值 `alpha_search`，公开路径
   `POST /v1/alpha/search`），归属 `openai_responses` 方言，`allows_stream = false`
   （`stream=true` 在解码期拒绝）。入口复用 Gateway Key 鉴权、公开模型允许列表、模型路由、
   RPM 预留、代理、健康、重试与请求日志，不新增设置项。
2. 请求体按既有 RawJson 直通规则处理：只解析路由必需的 `model`（改写为上游模型名后原字节
   转发），其余字段——包括 `id`、`input`、`commands`、`settings`——保持不透明，不改写、
   不删减。any2api 不在本地实现搜索，也不解析 `results`/`encrypted_output`；成功与非 2xx
   响应都按既有 buffered 直通规则原样返回（响应体无 `model` 字段时字节不变）。
3. 仅同方言直通：跨方言 Bridge 不声明 `alpha_search`；OAuth 候选按 `oauth_supports_operation`、
   API Key 候选按 Driver endpoint 计划在候选构造阶段过滤，不声明该操作的 Provider 在 RPM 预留
   和上游 I/O 前排除。Provider 侧只有 Codex Driver 声明支持——OAuth（ChatGPT 数据面
   `{base}/alpha/search`）与 OpenAI API Key（`{base_url}/alpha/search`）；Grok、Kimi、Claude
   不声明。路径由既有 `endpoint_url` 后缀映射派生，与 Codex 客户端对官方 Provider 的
   `url_for_path("alpha/search")` 拼接一致。
4. 会话粘性：请求体顶层 `id` 是 Codex 会话 UUID，与同会话模型请求的 `session_id` 请求头
   同值。`AlphaSearch` 把它提取为 `codex` 命名空间的显式会话标识，与模型请求共用同一
   会话→凭据绑定，使搜索命中同一上游账号（上游 `turn0searchN` 引用状态与配额结算随会话
   连续）。`id` 缺失时按普通候选调度；非字符串时拒绝。不建立续接绑定，响应 `id` 不进入
   续接注册表。
5. 尺寸与时限沿用标准档：请求体 32 MiB、buffered 响应 16 MiB、超时使用当前设置
   （默认 `upstream.read_timeout = 300s`），不设专用下限。zstd 请求体编码不对该路径开放。

## 边界与非目标

- 不实现本地搜索、结果缓存或对 `commands`/`settings` 的语义校验；上游行为差异（包括上游
  不支持该端点时的 404）原样透传。
- 不改变托管 `web_search` 工具路径：`/v1/responses` 请求体内的 `tools:[{"type":"web_search"}]`
  与响应中的 `web_search_call` 项继续按既有 Responses 直通规则处理。
- 请求体凭据无关（ADR-0149 请求面不受影响）；Codex OAuth 的 Responses 体规范化
  （store/include 重写）不适用于该操作。
- 客户端 `originator`、`x-codex-turn-metadata` 等请求头继续按既有 Codex 请求头投影规则
  处理，不新增投影名单。

## 后果

- Codex CLI ≥0.147 在 `supports_standalone_web_search = true` 的 any2api Provider 配置下,
  `web__run` 全链路可用；未启用搜索的客户端行为不变。
- `domain` 新增操作枚举值后，路由检查、能力矩阵、请求日志与管理路由巡检自动覆盖新操作；
  `server` 增加路由与路径映射，`protocol` 增加方言归属与会话标识提取，`provider` 只扩展
  Codex Driver 的两个操作声明，`runtime`、`transport`、`storage` 零改动，前端仅补充操作
  标签与联合类型。
