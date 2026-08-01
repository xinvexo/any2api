# ADR-0040：Grok API Key OpenAI 兼容 Provider

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

any2api 接入 xAI Grok 时继续遵守 Provider、协议、调度和凭据类型分离边界。xAI 的公开推理 API 使用 Bearer API Key，提供 OpenAI 兼容的 Responses、Chat Completions、Responses Compact 与模型目录接口；Grok OAuthAccount 则由独立的固定订阅数据面 Profile 管理。

## 决策

1. 新增独立的 `ProviderKind::Grok`，SQLite 文本值固定为 `grok`，Provider Secret 指纹域稳定代码固定为 `3`。
2. `GrokDriver` 只支持 `CredentialKind::ApiKey`，使用 `Authorization: Bearer <XAI_API_KEY>`。Web 的官方默认 Base URL 为 `https://api.x.ai/v1`。
3. Grok 注册 OpenAI Responses 与 OpenAI Chat Completions 能力，支持 Responses、Responses Compact、Chat Completions 的 JSON/SSE 现有执行路径，以及标准 `GET /models` 模型发现。
4. Grok 复用现有 OpenAI ProtocolAdapter、协议桥、错误分类、Transport、RPM、轮询、粘性、健康、重试、代理和遥测实现。中央调度器不增加 Grok 分支。
5. `provider_endpoints` 与 ProviderCredential 只接受 Grok API Key；Grok OAuth JSON 只能创建独立 OAuthAccount，不能进入该表。
6. Provider Registry 契约必须枚举并验证 Grok Driver；Storage 契约验证 Grok Endpoint 与 Credential 可写。

## 依据

- xAI Text Generation：<https://docs.x.ai/developers/model-capabilities/text/generate-text.md>
- xAI Chat Completions：<https://docs.x.ai/developers/rest-api-reference/inference/chat.md>
- xAI Models：<https://docs.x.ai/developers/rest-api-reference/inference/models.md>

## 结果

- Grok API Key 与 Codex/Claude API Key 一起编译为统一 `RoutingCredential` 候选，不产生第二套调度逻辑。
- 同一公开模型和入口协议下，Grok 可以与其他兼容 Endpoint 一起参与稳定轮询与 RPM 准入。
- Grok OAuthAccount 使用 ADR-0041 的固定订阅数据面，与 API Key 只在通用 `RoutingCredential` 投影处合流。

## 未选择方案

- 把 Grok 伪装成 Codex：会混淆 Provider 能力、管理展示、Secret 指纹域和运行态汇总。
- 为 Grok 新建协议栈：其当前接口可由既有 OpenAI 协议 Adapter 表达，会造成重复实现。
- 把 Grok OAuth JSON 存入 ProviderCredential：会破坏 API Key 与 OAuthAccount 的持久化和 Secret 边界。
