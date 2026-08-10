# ADR-0129: 可查询的协议 fidelity 与 Bridge capability contract

- 状态：Accepted
- 日期：2026-08-10
- 决策者：maintainer

## 背景

Endpoint 管理 API 原先只返回“接受方言对应哪些上游方言”，Web 也只显示方言名称。管理员无法从配置面区分同方言直通与跨协议重建，更无法在发送请求前知道 Bridge 支持哪些字段、工具类型和限制。另一方面，Responses → Chat Completions 的顶层字段 allowlist、operation 判断和工具类型判断分别位于不同函数；如果另写一份展示清单，执行契约与管理契约会独立漂移。

跨协议转换不可能逐字节透明。Responses → Chat Completions 必然生成 canonical Chat request，返回方向还会合成 Responses ID、事件与 continuation。这里的目标不是伪装成原生 Responses，而是让 translated semantics 明确、可查询并由同一份静态事实驱动。

## 决策

1. Protocol API 增加 `ProtocolFidelity::{Direct, Translated}`。Direct 只表示本次协议对不创建 `ProtocolBridge`；模型替换、stream 裁剪、重复 key 消歧、Responses replay identity 或 Provider request profile 仍可能局部重写 Body，因此不使用 `Transparent` 或 `BytePreserving` 名称。
2. 每个 `ProtocolBridge` 返回一个静态、版本化 `ProtocolBridgeCapabilities`，包含 contract ID、支持的 operation、顶层请求字段及处理方式、允许的工具类型和静态限制。
3. 字段处理方式只有四种：`Forwarded` 原值进入目标方言；`Translated` 改名或改结构；`ValidatedOnly` 只校验或限制、不作为同名目标字段发送；`LocalState` 由 any2api 的 continuation/响应投影拥有。未知顶层字段仍 fail-closed。
4. Bridge 的 operation 准入、顶层字段准入和工具类型准入必须读取 capability table。嵌套结构和值域继续由职责明确的转换模块校验，不为了展示而复制整套 JSON Schema。
5. `ConfigurationCapabilities` 把 Provider 支持的每个上游方言改为结构化 upstream option：方言、fidelity、支持的 operation，以及 Translated 路径的完整静态 Bridge contract。Direct option 没有 Bridge contract。
6. 管理 Endpoint HTTP 契约以 `upstream_options` 取代旧的纯字符串 `upstream_protocols`。项目不保留旧字段别名或双轨解析；Web 同步使用当前契约，并验证 Direct/Translated 与 Bridge contract 是否自洽。
7. Endpoint 编辑器在转换选择器旁展示当前 option 的 fidelity。Translated 展示 contract ID、支持 operation、字段处理表、工具类型和已知限制；Direct 明确“没有 Bridge，但不保证逐字节不变”。这些内容从 API 数据生成，不按 ProviderKind 写分支。
8. Capability 内容全部是编译期静态说明，不包含 Base URL、模型、Credential、客户端字段值、Session ID 或 Secret，也不执行真实上游探测。

## 当前 contract

- `openai-responses-to-chat-completions/v1`：只支持 `responses`；顶层字段表与 request converter 共用；工具类型只允许 `function`；明确登记 single-choice、canonical reconstruction、validated-only metadata、local continuation 和 synthetic response identity。
- `openai-images-to-chat-completions/v1`：只支持 `images_generations`；明确登记非流式、URL-only、无 partial image 和 canonical reconstruction。

## 后果

- Kimi Responses Endpoint 在保存前会明确显示 Translated，而不是看起来像原生 Responses。
- 新增 Responses 字段时，开发者必须同时决定它的转换行为；未登记字段继续返回精确路径错误。
- 管理 API 体积会增加少量静态 contract 数据，但不增加数据库字段、配置代际或运行时网络请求。
- F-008 的确定性重建不会消失，而是变成受版本控制、可查询且可测试的显式产品边界。

## 验证

- Protocol Registry 测试枚举实际 Bridge，证明 capability operation 与 exchange 准入一致。
- Responses/Images 请求转换测试证明未知字段与非登记工具类型由 capability table 拒绝。
- Runtime capability 测试验证 Direct 与 Translated option 及 Bridge contract。
- Server/Web contract 测试验证当前 `upstream_options` JSON，编辑器测试验证 Kimi 默认 Translated 路径及 fidelity 展示。
