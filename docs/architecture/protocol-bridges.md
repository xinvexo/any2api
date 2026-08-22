# Provider、协议与桥接

本文是 Provider 能力模型、协议转换和目标 Profile 的当前规范。公开 URL 清单属于
[README](../../README.md#public-api)，具体枚举值和注册项以 `domain::kind`、`provider::api` 与
`protocol::api` 为准。

## 词汇和所有权

- **Provider** 表示上游供应商契约，包括认证、端点、错误分类、OAuth 和目录/额度操作。
- **ProtocolDialect** 表示一组线协议，例如 Responses、Chat Completions、Images 或 Messages。
- **ProtocolOperation** 表示一个具体操作；Operation 归属于一个 Dialect，并决定是否允许流式响应。
- **ProtocolBridge** 把一个入站 Dialect 的 Operation 显式转换到另一个上游 Dialect。
- **ProtocolTargetProfile** 描述目标 Dialect 的可验证差异。它是 Provider Driver 的静态契约，不是管理员
  配置、数据库记录或运行时自动生成内容。

`protocol` 拥有请求、响应、usage、工具和 SSE 的线格式；`provider` 拥有供应商行为；`transport` 拥有实际
连接。Provider 可以选择 Protocol 定义的 Profile，但不能在 Driver 中复制或执行 Bridge。

## Provider capability model

每个 `ProviderDriver` 返回一个进程期静态、不可变的 `ProviderDescriptor`；它是该 Driver 结构能力的单一事实
源。Descriptor 集中声明：

- Provider kind；
- API Key 可执行的 Operation；
- 是否支持 OAuth，以及登录方式、OAuth Operation 和额度相关能力；
- 可使用的 Transport mode。

Descriptor 不另存一份 protocol 清单；`protocols()` 从 API Key 与 OAuth Operation 所属 Dialect 的并集推导。
Provider 文本 ID 统一由 `ProviderKind::as_str()`/`FromStr` 拥有，Storage 和管理投影不维护第二套映射。

基础 Driver 只提供 API Key、请求准备和错误分类等所有 Provider 都需要的行为。OAuth 授权码、设备码、Token、
路由、额度、补充购买与重置通过独立 facet 暴露。Registry 在注册时验证 descriptor 与 facet 一致：OAuth
必须同时拥有 Token 与 Routing facet，login flow 只能对应一种登录 facet，quota/supplement/reset flag 必须
与实际 facet 匹配。因此“声明支持但没有实现”和“实现存在但未声明”都会失败。

Runtime 只查询 descriptor 或请求对应 facet，不按 Provider kind 维护一组平行中央分支。新增 Provider 在
Composition Root 静态注册；管理能力响应也从同一 Registry 投影，不能由 Web 维护独立后端能力矩阵。

标准 OpenAI 与 Codex 即使共享部分 wire protocol，也保持不同 Provider descriptor：前者表示管理员配置的
标准 OpenAI API 契约，后者拥有 Codex 的 OAuth、请求身份和操作面。已有 Endpoint 不因 URL、模型名或新增
OpenAI 支持被自动改类；其他供应商同样保持显式 Provider kind，不提供宽松万能兼容类别。

OAuth routing facet 同时提供稳定主体、目录 scope、模型目录请求和 OAuth 认证 Header。相同且可证明共享的
目录 scope 可以复用一份有界目录快照；无法证明时保持账号级隔离。登录激活和管理员显式刷新可以更新目录，
但目录结果不自动改变账号已选模型或发布路由，模型变更仍需单独配置提交。

Codex 模型目录 scope 从当前 token 的官方 `chatgpt_plan_type` 稳定派生，不在本地维护套餐枚举；不同 plan
不会共享目录快照。所有模型目录响应只做名称合法性校验、去重和排序，不根据套餐或上游附带的可用性字段
删除模型。

需要官方客户端身份版本的 Provider 通过独立 facet 声明官方版本源、响应解析和运行态版本。Runtime 启动时
从 SQLite 恢复最后一次成功值，并按时效并发刷新；新值先持久化，再原子发布给 Driver。数据面热路径只读取
内存快照，官方源暂时失败时继续使用最后一次成功值。Provider 合成的 Header 使用对应官方客户端身份，
不加入本应用自己的出站标识。

## 请求计划

Runtime 先确定入站 Operation、目标 Operation、目标模型和凭据，再向 Provider 与 Protocol 查询计划：

1. Provider 验证该凭据种类和 Operation，并解析目标 URL、认证 Header、可选请求 Header/Body 调整。
2. Protocol Registry 决定同方言直通或选择已注册 Bridge。
3. Provider 根据目标 Dialect 和模型返回 `ProtocolTargetProfile`；Bridge 仅据 Profile 选择目标字段和能力。
4. Transport 接收最终 URL、Header、Body、代理和编码策略并发起请求。

同方言请求优先保持客户端 wire 信息；只有明确归属 Provider 的认证或必要契约由 Driver 调整。跨方言请求才
物化桥接结构。客户端 Gateway 凭据在进入 Provider 计划前已经隔离，不能透传成上游认证。

## Responses 到 Chat Completions

Responses → Chat 是一个共享 Bridge，不为每个供应商复制实现。当前 Chat target profile 以正交字段描述：

- token limit 字段和 instruction role；
- reasoning 请求与响应字段；
- cached-token usage 位置；
- custom tool 表达和工具名约束；
- 可接受的 Chat 请求字段；
- image URL/detail、input audio 和 file input 支持。

Protocol 提供当前 OpenAI 和兼容基线等可复用 Profile；Driver 也可以根据有证据的模型契约返回静态组合，
例如 Kimi 的模型差异。只有 Driver 明确返回 Profile 的目标才能进入 Bridge；系统不根据 Base URL、未知模型名
或一次响应自动学习 Profile。

Bridge 负责保持 Responses 的 item 顺序和工具调用关联，把 instructions、message content、reasoning、usage
与流式 delta 映射到目标协议。一个能力在 Chat 中没有可靠表达时，Bridge 返回明确的不支持错误；不会静默
删除托管工具、伪造结果或把不完整工具调用当作成功。工具投影和 streaming tool state 共享同一语义模型，
避免 unary 与 SSE 两套规则漂移。

## 响应、错误和编码

- Provider 对解码后的非成功响应做内部错误分类，为健康、冷却和重试提供类型化证据。
- 未被 any2api 自己消费的上游错误状态、Header 和 Body 尽可能透明返回；内部错误使用稳定的本地错误格式。
- Content-Encoding 由 Transport 在 Provider 分类和 Protocol 解码前处理；未知、损坏或过深的编码链失败，
  不能删除 Header 后把压缩字节冒充协议内容。
- SSE 由 Protocol 增量解析，网络 chunk 不等同于事件边界。详细提交规则见
  [路由与流式](routing-and-streaming.md#提交边界与流式响应)。

## 证据和验证

Registry 契约测试必须枚举实际注册的 Provider、Protocol 和 Bridge，验证 descriptor/facet 一致、Operation
可达性和 Profile 选择。最终请求路径、认证 Header 和 wire surface 使用少量 loopback 契约测试；纯字段映射
在 Protocol 单元测试中覆盖。

官方客户端的脱敏观测保存在 [docs/baselines/official-clients](../baselines/official-clients/README.md)。这些
证据可以支持一个明确契约，但不能扩展成身份伪装、随机指纹或对未知 Provider 的兼容猜测。
