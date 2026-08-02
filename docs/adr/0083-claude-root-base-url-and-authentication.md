# ADR-0083: Claude 根 Base URL 与认证选择

> 状态：Accepted
> 日期：2026-08-02
> 决策者：maintainer

## 背景

any2api 原先把 Claude API Key Endpoint 与 Codex/Grok 一样建模为“已经包含 API 版本的 Base URL”，再直接追加 `messages` 或 `models`。这会迫使管理员填写结尾 `/v1`，与 Anthropic 官方客户端及常见 Claude 兼容代理的配置语义不一致。管理员填写根地址时，模型探测会错误请求 `/models`；填写 `/v1` 后，路径虽然正确，但 any2api 对所有 Claude Endpoint 固定发送 `x-api-key`，无法访问要求 Bearer API Key 的自定义上游。

实测同一自定义上游时，根地址的 `/models` 返回 2xx 但不是模型目录；`/v1/models` 携带 `x-api-key` 返回 401；`/v1/models` 携带 `Authorization: Bearer` 返回有效模型目录。CLIProxyAPI 对 Claude 也使用根 Base URL，统一追加 `/v1/messages`，并仅对 Anthropic 官方 Origin 使用 `x-api-key`。

## 决策

1. Claude `ProviderBaseUrl` 表示 Anthropic API 版本路径之前的根地址或固定代理前缀。数据面与模型探测由 `ClaudeDriver` 分别追加 `v1/messages`、`v1/messages/count_tokens` 和 `v1/models`。
2. Web 的 Claude 官方默认 Base URL 改为 `https://api.anthropic.com`。Claude OAuth 固定路由 Profile 同样保存该根地址并复用同一 Driver 路径构造。
3. API Key 认证由 `ClaudeDriver` 根据已发布 Endpoint 决定：精确的官方 HTTPS Origin `api.anthropic.com` 使用 `x-api-key`；其他自定义 Claude Endpoint 使用 `Authorization: Bearer`。Runtime 不增加 Provider 分支。
4. Credential 模型探测计划可携带 Provider 定义的非敏感固定 Header。Claude `/v1/models` 固定发送 `anthropic-version: 2023-06-01`，认证 Header 仍由当前 Credential generation 最后注入。
5. 新增一次性前向迁移，把现有 Claude Endpoint 末尾恰好为 `/v1` 的 Base URL 规范化为根地址。生产代码不保留同时接受两种 Claude Base URL 语义的兼容分支。
6. 管理端不能把 401/403 仅描述为 API Key 被拒绝，因为自定义 Endpoint 的 Base URL 与认证契约错误也会产生相同状态。提示必须同时要求核对 Base URL 与上游认证要求，并继续允许手工模型选择。

## 依据

- Anthropic Python SDK 默认 Base URL：<https://github.com/anthropics/anthropic-sdk-python/blob/f5c30d0490fb7bcd8e0b65d8d8e63c0e7d1bfe59/src/anthropic/_client.py>
- Anthropic Python SDK Messages 路径：<https://github.com/anthropics/anthropic-sdk-python/blob/f5c30d0490fb7bcd8e0b65d8d8e63c0e7d1bfe59/src/anthropic/resources/messages/messages.py>
- Anthropic Python SDK Models 路径：<https://github.com/anthropics/anthropic-sdk-python/blob/f5c30d0490fb7bcd8e0b65d8d8e63c0e7d1bfe59/src/anthropic/resources/models.py>
- CLIProxyAPI Claude 路径构造：<https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/internal/runtime/executor/claude_executor_execute.go>
- CLIProxyAPI Claude API Key 认证选择：<https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/internal/runtime/executor/claude_executor.go>

## 结果

- Claude 官方地址与自定义兼容上游都使用相同的根 Base URL 配置语义，不需要管理员手工拼 `/v1`。
- 路径和认证决策集中在 Claude Driver，Credential 测试与真实数据面不会再出现不同契约。
- 已存量 `/v1` 配置在升级时一次性转换，配置发布后只有一种运行时语义。

## 未选择方案

- 继续要求 Claude Base URL 带 `/v1`：与官方 SDK 和 CLIProxyAPI 的配置语义冲突，也无法解决自定义上游认证问题。
- 同时尝试 `x-api-key` 与 Bearer：会增加隐式认证回退和额外上游请求，可能在提交边界后重放请求。
- 按 401 自动切换认证：响应状态不能可靠区分错误 Key、错误 Base URL 与认证方案不匹配，并违反单一确定请求计划。
