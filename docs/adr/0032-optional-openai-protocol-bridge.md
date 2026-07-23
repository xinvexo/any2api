# ADR-0032: 可选 OpenAI 协议桥与有效上游协议

- 状态：Accepted
- 日期：2026-07-23
- 决策者：maintainer

## 背景

Provider Endpoint 目前只有一个 `protocol_dialect`，它同时表示客户端入口协议和实际上游协议。Runtime 因而假定同一个 `ProtocolAdapter` 可以贯穿入口解码、上游编码、上游响应解码和客户端响应编码，并在配置发布时拒绝协议不一致的 Route Target。

管理员需要把只支持 OpenAI Chat Completions 的兼容上游暴露为 Responses：客户端继续请求 `/v1/responses`，any2api 内部调用 `/v1/chat/completions`，再把 JSON 或 SSE 转回 Responses。管理员同时明确要求内部转换协议不是必选项；未选择转换时，上游协议必须自动等于接受协议。

## 决策

- `ProviderEndpoint.protocol_dialect` 保留为必填的客户端接受协议。
- 新增可空的 `upstream_protocol_dialect`，管理界面命名为“内部转换协议（可选）”。其有效值定义为：

  ```text
  effective_upstream = upstream_protocol_dialect ?? protocol_dialect
  ```

- 空值表示严格同协议直通。数据库不重复保存与接受协议相同的值；管理 API 收到相同值时归一化为空值。
- 内部 ModelRoute 继续以接受协议作为 `ingress_protocol`；Route Target 保存已经解析的有效上游协议，使运行时、硬粘性和请求日志不依赖回查可变 Endpoint。
- 新增 `openai_chat_completions` 方言和 `/v1/chat/completions` JSON/SSE 入口。第一批有效组合固定为 Responses → Responses、Responses → Chat Completions、Chat Completions → Chat Completions、Messages → Messages。
- 在 `ProtocolRegistry` 中按 `(ingress_dialect, upstream_dialect)` 静态注册 `ProtocolBridge`。协议相同走 Adapter 快速路径，不查找 Bridge；协议不同时必须在配置发布前找到 Bridge，否则拒绝发布。Domain、Runtime、Storage 和 Server 都不得为 Responses → Chat 写专用协议对分支；新增转换只增加 Bridge 实现、能力声明和 Composition Root 注册。
- 第一座桥只实现 Responses → Chat Completions。它负责请求、非流式响应、SSE、工具调用、usage 和错误转换；无法可靠表达的字段在上游提交前 fail-closed，禁止静默丢弃。
- Responses → Chat 的 `previous_response_id` 使用本地合成 ID 和有界内存对话历史；现有硬粘性注册表同时固定 Credential、Route Target、上游模型和协议对。任一状态过期或重启后都返回 `session_binding_lost`，不持久化、不恢复、不猜测。
- Base URL 仍是管理员填写的受信任 HTTP(S) 目标。Provider Driver 仅根据有效上游协议结构化追加 `/responses` 或 `/chat/completions`，不改变 authority、不增加 HTTP/内网授权开关。
- `/v1/responses/compact` 不桥接到 Chat Completions；Codex/OpenAI 与 Claude Messages 的双向转换不在本决策范围。

## 备选方案

- 强制同时填写接受协议和上游协议：重复保存同协议事实，增加 UI 噪音，也违背“内部转换可选”的管理语义。
- 只在 Provider Endpoint 保存实际上游协议、另建手工 Route 页面选择入口协议：理论上更规范，但当前模型 Route 是由 Credential 模型选择自动物化；为一个字段引入完整手工路由控制面会扩大产品复杂度。
- 在 Responses Adapter 内按 Provider 分支调用 Chat：会把 Provider、协议和转换逻辑耦合，并继续让 Runtime 误以为只有一个方言，无法正确固定路径、遥测和粘性。
- 静默删除无法映射的 Responses 字段：会产生看似成功但语义改变的请求，不可接受。

## 后果

- 原有 Endpoint 迁移后 `upstream_protocol_dialect=NULL`，行为保持同协议直通。
- 管理员创建 Provider 时只需选择接受协议；只有兼容上游协议不同时才额外选择内部转换协议。
- Runtime Attempt 必须同时持有入口 Adapter、有效上游 Adapter 和可选 Bridge，流式提交状态机与 Permit 生命周期仍保持不变。
- Chat Completions 没有服务端 Response 状态，桥接多轮会增加有界内存占用；该状态遵守现有“进程重启全部清空”边界。

## 验证

- Domain/Storage 测试覆盖空值回退、相同值归一化、无 Bridge 组合拒绝和迁移后旧 Endpoint 行为不变。
- Protocol 契约覆盖 Responses 请求、JSON、任意字节切分 SSE、CRLF、多行 data、工具调用、usage、错误和不支持字段。
- Runtime/HTTP 契约覆盖直通不进入 Bridge、Responses → Chat 路径、首字节后禁止切换、Permit 单次释放、合成 Response ID、多轮历史和重启后 `session_binding_lost`。
- Web 测试覆盖转换协议可留空、空值说明、按接受协议过滤可用转换目标和编辑回显。
