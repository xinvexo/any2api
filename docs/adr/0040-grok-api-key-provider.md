# ADR-0040：Grok 作为 API Key-only OpenAI 兼容 Provider

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

any2api 需要接入 xAI Grok，并继续遵守 Provider、协议、调度和凭据类型分离的既有边界。xAI 的公开推理 API 使用 Bearer API Key，提供 OpenAI 兼容的 Responses、Chat Completions、Responses Compact 与模型目录接口。Grok 当前没有进入本项目 OAuth2 账号管理的需求。

## 决策

1. 新增独立的 `ProviderKind::Grok`，SQLite 文本值固定为 `grok`，Secret Vault Provider 稳定代码固定为 `3`。
2. `GrokDriver` 只支持 `CredentialKind::ApiKey`，使用 `Authorization: Bearer <XAI_API_KEY>`。Web 的官方默认 Base URL 为 `https://api.x.ai/v1`。
3. Grok 注册 OpenAI Responses 与 OpenAI Chat Completions 能力，支持 Responses、Responses Compact、Chat Completions 的 JSON/SSE 现有执行路径，以及标准 `GET /models` 模型发现。
4. Grok 复用现有 OpenAI ProtocolAdapter、协议桥、错误分类、Transport、RPM、轮询、粘性、健康、重试、代理和遥测实现。中央调度器不增加 Grok 分支。
5. `provider_endpoints` 通过只前向的 Migration 接受 `grok`。迁移重建受外键约束影响的表并保留数据、索引和外键完整性，不修改任何既有 Migration。
6. `oauth_accounts` 的数据库约束、OAuth Web 类型和 Provider JSON Schema 继续只允许 Codex/Claude。领域构造、序列化和存储边界都显式拒绝 Grok OAuthAccount。
7. Provider Registry 契约必须枚举并验证 Grok Driver；迁移测试必须验证旧数据保留、Grok Endpoint 可写以及 Grok OAuthAccount 不可写。

## 依据

- xAI Text Generation：<https://docs.x.ai/developers/model-capabilities/text/generate-text.md>
- xAI Chat Completions：<https://docs.x.ai/developers/rest-api-reference/inference/chat.md>
- xAI Models：<https://docs.x.ai/developers/rest-api-reference/inference/models.md>

## 结果

- Grok API Key 与 Codex/Claude API Key 一起编译为统一 `RoutingCredential` 候选，不产生第二套调度逻辑。
- 同一公开模型和入口协议下，Grok 可以与其他兼容 Endpoint 一起参与稳定轮询与 RPM 准入。
- OAuth 管理面不会因为 Provider 枚举扩展而错误显示或接受 Grok。
- Provider Endpoint 表的变更需要一次较宽的 SQLite 前向迁移，这是在开启外键且不使用 `writable_schema` 的前提下保持完整性的必要成本。

## 未选择方案

- 把 Grok 伪装成 Codex：会混淆 Provider 能力、管理展示、Vault AAD 和运行态汇总。
- 为 Grok 新建协议栈：其当前接口可由既有 OpenAI 协议 Adapter 表达，会造成重复实现。
- 同时开放 Grok OAuth：没有已确认的 Provider OAuth 契约，会破坏 OAuthAccount 的明确边界。
