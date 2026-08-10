# ADR-0124: Kimi 服务身份与 OpenAI Chat 方言分离

- 状态：Accepted
- 日期：2026-08-10
- 决策者：maintainer

## 背景

Kimi API 使用 OpenAI-compatible Chat Completions wire dialect，但“兼容某种协议”不表示它属于 Codex 或 Grok。旧模型只有 `ProviderKind::{Codex,Claude,Grok}`，管理员要路由 Kimi 时只能把 Moonshot Endpoint 标成 Codex/Grok，因此 Kimi 会继承错误的固定 Header、OAuth 能力、错误分类、原生协议声明和 Secret 指纹域。Responses → Chat Completions Bridge 已能表达 Kimi K3 请求与多轮工具调用，但协议桥不能替代 Provider 服务身份。

Moonshot 官方契约确认中国站 API Key 使用 `https://api.moonshot.cn/v1`、`Authorization: Bearer`、`GET /v1/models` 与 `POST /v1/chat/completions`。Kimi K3 的思考强度、`reasoning_content` 和工具调用属于 Chat payload/response 契约，由现有 OpenAI Chat Adapter 与通用 Responses Bridge 处理，不应在 Runtime 按模型名分支。

## 决策

1. 新增独立 `ProviderKind::Kimi`，稳定 SQLite 文本值为 `kimi`，Provider API Key 指纹域代码为 `4`。`ProviderKind` 表示服务身份，`ProtocolDialect` 继续独立表示 wire dialect。
2. `KimiDriver` 首版只声明 `OpenAiChatCompletions`、JSON/SSE 和 `CredentialKind::ApiKey`。它使用 Base URL 下的 `chat/completions` 与 `models`，认证为 Bearer API Key；Web 默认 Base URL 为 `https://api.moonshot.cn/v1`。
3. Kimi 不声明 OAuth、原生 Responses、Responses Compact、Anthropic Messages 或原生 Images。Responses 请求只有在 Endpoint 显式配置 `openai_responses -> openai_chat_completions` 时进入既有通用 Bridge；Driver、Runtime 和 Storage 禁止按 `kimi-*` 模型名增加分支。
4. Kimi 不伪造 Codex/Grok/Claude 客户端 persona Header。请求只使用协议层根据最终正文重建的必要 Header 和当前 Kimi Credential 的认证；响应仅投影明确的内容类型、请求 ID、Retry-After 与限速 Header。
5. Kimi 使用独立错误分类器，只识别 Moonshot 官方 error envelope 中声明的 `type`/`code`。认证、权限、额度、限速、过载、模型不存在与无效请求按现有强类型错误映射；未知字段和自然语言 message 不参与分类。
6. 追加前向 Migration 扩展 `provider_endpoints.provider_kind` CHECK。Migration 保留全部 Endpoint、Credential、模型、Route、日志与版本；不根据 Base URL 猜测并重写现有 Codex/Grok 记录。ProviderKind 仍是创建后不可变身份，旧错标 Endpoint 由管理员显式新建 Kimi Endpoint，避免静默改变认证和路由语义。
7. Composition Root 静态注册 Kimi Driver，Provider Registry 契约枚举实际 Registry 并验证四种 Provider。管理 API、Web 分类、运行态汇总和持久化只增加通用 `ProviderKind` 值，不增加中央调度分支。

## 后果

- Kimi 请求不再带 Codex/Grok 身份，也不会继承三者的 OAuth 与能力声明。
- 直接 Chat → Chat 保留同方言快速路径；Responses → Chat 仍具有明确的 translated fidelity，但转换行为按协议对复用。
- 其他 OpenAI-compatible 服务不会因为本决策自动获得 Kimi 身份；新增服务仍需独立 Driver 或另行决策一个能力受限、诚实名义的 generic Provider。
- 已有错标配置不会被 URL 推断迁移，避免把自定义代理或兼容网关静默改成 Kimi。

## 验证

- Domain/Storage/Migration 测试覆盖 `kimi` 稳定值、OAuth 禁止、指纹域隔离和带关联 Credential/Route/RequestLog 的升级保留。
- Provider 契约测试覆盖 Chat/models URL、Bearer 认证、无 persona Header、能力集、模型目录和 Moonshot 错误码。
- Registry/管理 API/Web 测试覆盖 Kimi 分类与 Responses → Chat 配置选项，并确认没有 Kimi → Responses 的反向声明。
- 现有 Bridge 多轮 `reasoning_content`、工具调用、SSE 和 direct Chat 测试继续作为协议正确性证据。

## 依据

- Moonshot Kimi API 快速开始：<https://platform.kimi.com/docs/overview>
- Moonshot Chat Completions：<https://platform.kimi.com/docs/api/chat>
- Moonshot 模型目录：<https://platform.kimi.com/docs/api/list-models>
- Moonshot 错误码：<https://platform.kimi.com/docs/api/errors>
