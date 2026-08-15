# 当前 ADR 索引

[ARCHITECTURE.md](../../ARCHITECTURE.md) 是当前架构真相来源。本目录补充仍然有效的设计理由和边界；
不能通过按编号顺序叠加 ADR 来推导当前实现。遇到正文冲突时，以架构基线为准；专项 ADR 只有在本索引
明确列为同一主题的当前入口时补充架构，并在实现前先消除冲突。

本次文档整理按要求直接删除完全失效、仅剩历史用途或被当前模型完整替代的文档；这种处理方式
只适用于本次任务，不建立后续 ADR 生命周期规则。下面的完整清单只包含当前有效的 Accepted 决策。
[0000-template.md](0000-template.md) 仅用于新建 ADR，不属于当前决策。

## 按主题阅读

| 主题 | 当前入口 |
|---|---|
| 配置发布与存储 | [SettingRegistry](0011-scheduler-setting-registry.md)、[配置事务](0103-storage-owned-configuration-transaction.md)、[当前 Schema](0112-current-schema-only-runtime.md)、[后台发布 Rebase](0155-background-oauth-refresh-configuration-rebase.md) |
| 凭据、认证与代理 | [Credential 生命周期](0003-provider-credential-lifecycle.md)、[Gateway Key](0006-gateway-api-key-auth.md)、[OAuthAccount](0033-database-backed-oauth-accounts.md)、[OAuth 默认出口](0150-oauth-only-global-proxy.md) |
| 协议、Provider 与模型 | [协议桥](0032-optional-openai-protocol-bridge.md)、[Provider identity](0126-versioned-provider-and-transport-identity.md)、[协议 fidelity](0129-queryable-protocol-fidelity-capabilities.md)、[公开模型别名](0153-provider-credential-public-model-alias.md) |
| Transport 与请求面 | [用途/模式隔离](0123-purpose-and-mode-transport-isolation.md)、[Header ownership](0125-credential-owned-request-headers.md)、[响应 Content-Encoding](0127-transport-response-content-coding.md)、[同方言 Accept-Encoding](0135-same-dialect-accept-encoding-pass-through.md)、[缓存连续性](0149-upstream-request-surface-and-cache-continuity.md) |
| 调度、会话与重试 | [QueueTicket](0010-bounded-generation-queue.md)、[RPM](0037-single-optional-rpm-admission.md)、[会话绑定](0062-unified-session-affinity.md)、[RetrySafety](0093-evidence-based-precommit-retry-safety.md)、[预提交拒绝](0136-precontent-rejection-fidelity-and-overload-backoff.md) |
| OAuth 与额度 | [额度持久化](0111-activity-driven-persistent-oauth-quota.md)、[刷新诊断](0116-typed-oauth-refresh-diagnostics.md)、[Credits/健康](0137-codex-credits-and-quota-health.md)、[费率卡](0145-configurable-codex-quota-rate-card.md)、[累计统计](0146-cumulative-codex-quota-statistics.md) |
| 日志与可观测 | [请求遥测](0015-bounded-request-telemetry.md)、[HTTP 系统日志](0051-complete-http-access-logging.md)、[原始交换详情](0081-raw-http-system-log-details.md)、[字节有界队列](0114-byte-bounded-telemetry-ownership.md) |
| Web、部署与更新 | [浏览器 E2E](0022-browser-e2e-contract.md)、[内嵌资源](0027-embedded-web-assets.md)、[Web 错误边界](0085-web-error-recovery-boundaries.md)、[自更新回滚](0089-bounded-self-update-binary-rollback.md) |

## 完整当前清单

| 编号 | 决策 |
|---|---|
| [0003](0003-provider-credential-lifecycle.md) | ADR-0003: ProviderCredential API Key 生命周期 |
| [0006](0006-gateway-api-key-auth.md) | ADR-0006: Gateway API Key 管理与公开入口认证边界 |
| [0009](0009-sse-framing-and-guarded-body.md) | ADR-0009: 同协议 SSE 分帧与 GuardedBody 生命周期 |
| [0010](0010-bounded-generation-queue.md) | ADR-0010: 生成请求的有界 QueueTicket 与快照策略 |
| [0011](0011-scheduler-setting-registry.md) | ADR-0011: Scheduler SettingRegistry 与快照热更新 |
| [0013](0013-precommit-retry-and-runtime-health.md) | ADR-0013: 提交前重试、错误分类与代际健康状态 |
| [0014](0014-single-admin-auth-and-remote-http.md) | ADR-0014: 单管理员认证与远程 HTTP 管理边界 |
| [0015](0015-bounded-request-telemetry.md) | ADR-0015: 有界请求遥测、Attempt 历史与查询边界 |
| [0016](0016-stream-precommit-budget.md) | ADR-0016: SSE 预提交字节与时长预算 |
| [0017](0017-upstream-read-and-stream-idle-timeouts.md) | ADR-0017: 上游读取与流式提交后空闲超时 |
| [0018](0018-proxy-authentication-and-admin-probe.md) | ADR-0018: 代理认证与管理探测边界 |
| [0019](0019-strict-ssrf-local-dns.md) | ADR-0019: 严格 SSRF 的本地 DNS 与固定目标连接 |
| [0020](0020-public-ingress-error-adapters.md) | ADR-0020: 公开入口错误复用协议适配器 |
| [0021](0021-bounded-local-file-logging.md) | ADR-0021: 有界本地文件日志与热更新策略 |
| [0022](0022-browser-e2e-contract.md) | ADR-0022: 真实服务浏览器 E2E 契约 |
| [0024](0024-admin-password-rotation-and-instance-lock.md) | ADR-0024: 管理员密码在线轮换与数据目录单实例锁 |
| [0025](0025-protocol-token-telemetry.md) | ADR-0025: 协议级精确首 Token 与 Token Usage 遥测 |
| [0026](0026-bounded-graceful-shutdown.md) | ADR-0026: 有界优雅停机与进程任务生命周期 |
| [0027](0027-embedded-web-assets.md) | ADR-0027: 内嵌 React 资源与单二进制发布 |
| [0028](0028-credential-model-discovery.md) | ADR-0028: Credential 驱动的模型发现与选择 |
| [0029](0029-provider-base-url-authority.md) | ADR-0029: Provider Base URL 直接决定访问目标 |
| [0032](0032-optional-openai-protocol-bridge.md) | ADR-0032: 可选 OpenAI 协议桥与有效上游协议 |
| [0033](0033-database-backed-oauth-accounts.md) | ADR-0033: SQLite OAuthAccount 与统一路由 |
| [0034](0034-codex-oauth-quota-reset.md) | ADR-0034: Codex OAuth 额度查询与重置次数消费 |
| [0035](0035-upstream-credential-usage-statistics.md) | ADR-0035: 上游 API Key 与 OAuthAccount 本地调用统计 |
| [0036](0036-virtualized-oauth-quota-management.md) | ADR-0036: OAuth 长集合虚拟化、批量额度刷新与失效清理 |
| [0037](0037-single-optional-rpm-admission.md) | ADR-0037: 单一可选 RPM 准入限制 |
| [0038](0038-aggregate-only-balancing-dashboard.md) | ADR-0038: 调度运行态只展示聚合数据 |
| [0039](0039-overview-and-simplified-settings.md) | ADR-0039: 总览承载运行态聚合，设置采用渐进披露 |
| [0040](0040-grok-api-key-provider.md) | ADR-0040：Grok API Key OpenAI 兼容 Provider |
| [0041](0041-grok-oauth-account.md) | ADR-0041：Grok OAuth 作为独立 OAuthAccount 接入订阅数据面 |
| [0043](0043-grok-device-authorization.md) | ADR-0043：Grok OAuth 使用 Device Authorization Grant |
| [0044](0044-provider-oauth-json-import.md) | ADR-0044：Provider 专用 OAuth JSON 批量导入 |
| [0045](0045-grok-oauth-billing-quota.md) | ADR-0045：Grok OAuth Billing 额度查询 |
| [0046](0046-claude-oauth-usage-quota.md) | ADR-0046：Claude OAuth 上游额度查询 |
| [0047](0047-feature-first-crate-layout.md) | ADR-0047：Rust 工作区采用 Feature-First 目录结构 |
| [0048](0048-disabled-oauth-token-keepalive.md) | ADR-0048: Keep disabled OAuth account tokens alive |
| [0050](0050-request-log-client-ip.md) | ADR-0050: RequestLog 保存可信解析后的客户端 IP |
| [0051](0051-complete-http-access-logging.md) | ADR-0051：完整 HTTP 访问系统日志 |
| [0052](0052-credential-usage-time-windows.md) | ADR-0052: 凭据调用统计使用固定时间窗口 |
| [0053](0053-generated-admin-dto-wire-types.md) | ADR-0053: 管理 API 线格式类型由 Rust DTO 生成 |
| [0054](0054-openai-images-api.md) | ADR-0054: OpenAI Images API 与媒体缓冲边界 |
| [0055](0055-flat-overview-request-analytics.md) | ADR-0055: 系统总览采用扁平布局和 RequestLog 调用分析 |
| [0059](0059-codex-remote-compaction.md) | ADR-0059：Codex 远程压缩识别与长时执行预算 |
| [0061](0061-transparent-upstream-error-responses.md) | ADR-0061: 最终上游错误响应透明返回 |
| [0062](0062-unified-session-affinity.md) | ADR-0062: 统一固定会话绑定 |
| [0063](0063-immutable-forward-migrations.md) | ADR-0063: SQLite Migration 历史不可改写并只追加前向脚本 |
| [0064](0064-optional-session-affinity-toggle.md) | ADR-0064: 普通显式 Session 的可选会话粘性开关 |
| [0065](0065-verified-github-release-self-update.md) | ADR-0065: 经校验的 GitHub Release 自动更新 |
| [0066](0066-active-session-overview.md) | ADR-0066: 总览只展示当前策略下的活动显式会话 |
| [0067](0067-portable-responses-replay-identities.md) | ADR-0067: OpenAI Responses 可重放 Item 身份归一化 |
| [0068](0068-local-model-rejection-contract.md) | ADR-0068: 本地模型拒绝使用终局参数错误 |
| [0069](0069-committed-log-change-events.md) | ADR-0069：提交后日志变更事件 |
| [0070](0070-oauth-authentication-and-quota-routing-health.md) | ADR-0070：OAuth 认证失效分类与额度路由健康 |
| [0071](0071-remove-web-setting-reset-actions.md) | ADR-0071: Web 移除 SettingRegistry 恢复默认入口 |
| [0072](0072-trusted-proxy-setting-and-remote-default.md) | ADR-0072: 可信代理进入 SettingRegistry，远程管理默认开启 |
| [0073](0073-explicit-public-model-access-mode.md) | ADR-0073: Explicit public model access mode |
| [0074](0074-plaintext-local-secret-storage.md) | ADR-0074: SQLite 明文 Secret 与部署信任边界 |
| [0075](0075-revision-scoped-runtime-bindings.md) | ADR-0075: Revision-scoped Runtime Binding 与配置发布隔离 |
| [0076](0076-atomic-bridge-continuation-state.md) | ADR-0076: 协议桥 Continuation 状态与路由目标原子归属 |
| [0077](0077-coalesced-health-wake-worker.md) | ADR-0077: 合并健康 deadline 的单一 scheduler wake worker |
| [0078](0078-bounded-batched-oauth-refresh.md) | ADR-0078: 有界并发与机会式分段发布 OAuth 定时刷新 |
| [0079](0079-oauth-quota-rejection-and-provider-egress.md) | ADR-0079：OAuth 额度拒绝与 Provider 出口诊断分离 |
| [0081](0081-raw-http-system-log-details.md) | ADR-0081：系统日志保存原始 HTTP 交换详情 |
| [0082](0082-remove-global-public-request-memory-admission.md) | ADR-0082：取消公开请求全局内存准入 |
| [0083](0083-claude-root-base-url-and-authentication.md) | ADR-0083: Claude 根 Base URL 与认证选择 |
| [0084](0084-tolerant-upstream-timeout-defaults.md) | ADR-0084：面向慢上游的宽松等待默认值 |
| [0085](0085-web-error-recovery-boundaries.md) | ADR-0085：Web 根级与路由级错误恢复边界 |
| [0086](0086-structured-quota-errors-on-http-400.md) | ADR-0086：HTTP 400 的结构化额度错误细化 |
| [0087](0087-interactive-oauth-account-reauthorization.md) | ADR-0087：交互式 OAuth 同账号重新授权 |
| [0088](0088-direct-loopback-admin-boundary.md) | ADR-0088：直接 loopback 管理边界与 IP 地址规范化 |
| [0089](0089-bounded-self-update-binary-rollback.md) | ADR-0089：自更新的有界二进制回滚 |
| [0090](0090-best-effort-file-log-finalization.md) | ADR-0090：文件日志控制面与 best-effort 收尾 |
| [0091](0091-bounded-web-update-confirmation.md) | ADR-0091：Web 自更新确认的有界恢复状态 |
| [0092](0092-bounded-http-access-log-capacity.md) | ADR-0092：HTTP 系统日志独立容量与有界 SQLite 回收 |
| [0093](0093-evidence-based-precommit-retry-safety.md) | ADR-0093: 以可证明的未执行证据判定提交前重试安全性 |
| [0094](0094-health-race-rpm-reservation-rollback.md) | ADR-0094: 健康竞争失败时精确回滚尚未开始的 RPM 预留 |
| [0095](0095-split-authentication-and-routing-health-generations.md) | ADR-0095：拆分认证健康与路由身份健康代际 |
| [0096](0096-tolerant-missing-strict-invalid-forwarded-headers.md) | ADR-0096：转发头缺失可控降级、非法值严格拒绝 |
| [0097](0097-bootstrap-console-before-sqlite.md) | ADR-0097: SQLite 之前安装启动期 Console Tracing |
| [0098](0098-http-409-413-422-error-matrix.md) | ADR-0098：HTTP 409、413、422 上游错误矩阵 |
| [0099](0099-grok-oauth-model-header-capability.md) | ADR-0099：Grok OAuth 模型 Header 的 UTF-8 字节语义 |
| [0100](0100-deterministic-header-projection-priority.md) | ADR-0100：确定性的 Header 投影优先级 |
| [0101](0101-shared-immutable-decoded-request.md) | ADR-0101: 重试共享不可变 DecodedRequest |
| [0102](0102-mutation-footprint-configuration-readback.md) | ADR-0102: 配置 Mutation 按影响面回读 |
| [0103](0103-storage-owned-configuration-transaction.md) | ADR-0103: Storage 独占配置事务能力 |
| [0104](0104-bounded-web-configuration-lifecycle.md) | ADR-0104: 收敛 Web 配置发布生命周期与稳定管理外壳 |
| [0105](0105-bounded-gateway-usage-tracker.md) | ADR-0105: 以 PublishedSnapshot reconcile 约束 Gateway 使用状态 |
| [0106](0106-side-effect-free-oauth-quota-evidence.md) | ADR-0106：OAuth 额度查询只读与拒绝证据收敛 |
| [0107](0107-anchored-keyset-log-pagination.md) | ADR-0107：日志使用带头部锚点的 Keyset 与按页定位 |
| [0108](0108-minimal-public-health-response.md) | ADR-0108：公共健康响应只保留状态与应用版本 |
| [0109](0109-gateway-auth-rejected-log-isolation.md) | ADR-0109：Gateway 鉴权拒绝日志的低优先级容量隔离 |
| [0110](0110-session-creating-wait-handoff.md) | ADR-0110: Session Creating 在候选等待前交还 |
| [0111](0111-activity-driven-persistent-oauth-quota.md) | ADR-0111：按实际使用触发的 OAuth 额度快照持久化 |
| [0112](0112-current-schema-only-runtime.md) | ADR-0112：运行时只面向当前 Schema |
| [0113](0113-shared-raw-json-ingress.md) | ADR-0113：同协议请求共享原始 JSON |
| [0114](0114-byte-bounded-telemetry-ownership.md) | ADR-0114：遥测在途所有权按字节有界 |
| [0115](0115-codex-oauth-responses-request-profile.md) | ADR-0115：Codex OAuth Responses 出站请求 Profile |
| [0116](0116-typed-oauth-refresh-diagnostics.md) | ADR-0116：OAuth Token 刷新使用分阶段安全诊断 |
| [0117](0117-openai-images-chat-completions-bridge.md) | ADR-0117：OpenAI Images 到 Chat Completions 图片上游桥 |
| [0118](0118-precontent-overload-rejection-and-final-stream-outcome.md) | ADR-0118: 首个语义输出前的明确过载拒绝与最终流结果 |
| [0119](0119-cross-platform-transient-memory-reclamation.md) | ADR-0119：跨平台短命大块内存归还 |
| [0120](0120-failure-attributed-candidate-path-balancing.md) | ADR-0120: 按故障归因与候选路径执行负载均衡 |
| [0121](0121-anthropic-precontent-rate-limit-retry.md) | ADR-0121: Anthropic 首个语义输出前的账号限流流式重试 |
| [0122](0122-provider-endpoint-cascade-delete.md) | ADR-0122: Provider Endpoint 确认后的级联删除 |
| [0123](0123-purpose-and-mode-transport-isolation.md) | ADR-0123：按用途与模式隔离 Transport/TLS 状态 |
| [0124](0124-kimi-provider-identity.md) | ADR-0124: Kimi 服务身份与 OpenAI Chat 方言分离 |
| [0125](0125-credential-owned-request-headers.md) | ADR-0125：客户端身份 Header 的重放与账号归属 |
| [0126](0126-versioned-provider-and-transport-identity.md) | ADR-0126: 版本化 Provider 应用身份与通用 Transport profile |
| [0127](0127-transport-response-content-coding.md) | ADR-0127：Transport 统一拥有响应 Content-Encoding |
| [0128](0128-reselect-retry-after-backoff.md) | ADR-0128: Credential reselect 遵守 Retry-After 与语义退避 |
| [0129](0129-queryable-protocol-fidelity-capabilities.md) | ADR-0129: 可查询的协议 fidelity 与 Bridge capability contract |
| [0130](0130-transport-and-stream-conformance-diagnostics.md) | ADR-0130: Transport 与流式 conformance 诊断契约 |
| [0131](0131-official-client-baseline-evidence.md) | ADR-0131: 官方客户端基线的证据与脱敏契约 |
| [0132](0132-provider-scoped-oauth-control-plane-pacing.md) | ADR-0132：按 Provider 排列 OAuth 控制面请求起始时刻 |
| [0133](0133-reject-duplicate-oauth-import-identities.md) | ADR-0133：拒绝可证明重复的 OAuth 导入身份 |
| [0134](0134-interactive-oauth-token-duplicate-guard.md) | ADR-0134：交互式 OAuth 精确 Token 重复保护 |
| [0135](0135-same-dialect-accept-encoding-pass-through.md) | ADR-0135: 同方言 Accept-Encoding 受控透传 |
| [0136](0136-precontent-rejection-fidelity-and-overload-backoff.md) | ADR-0136: 预提交拒绝保真与请求级过载退避 |
| [0137](0137-codex-credits-and-quota-health.md) | ADR-0137：Codex Credits 字段与额度健康 |
| [0145](0145-configurable-codex-quota-rate-card.md) | ADR-0145：可配置的 Codex 额度费率卡 |
| [0146](0146-cumulative-codex-quota-statistics.md) | ADR-0146：Codex 本机额度累计统计 |
| [0147](0147-codex-workspace-member-oauth-identity.md) | ADR-0147：分离 Codex 工作区路由标识与成员 OAuth 身份 |
| [0149](0149-upstream-request-surface-and-cache-continuity.md) | ADR-0149：上游请求面与 prompt cache 连续性 |
| [0150](0150-oauth-only-global-proxy.md) | ADR-0150: OAuth 默认出口与账号级代理选择 |
| [0151](0151-openai-responses-websocket-ingress.md) | ADR-0151：OpenAI Responses WebSocket 入口 |
| [0152](0152-openai-alpha-search-ingress.md) | ADR-0152：OpenAI Alpha Search 入口（`POST /v1/alpha/search`） |
| [0153](0153-provider-credential-public-model-alias.md) | ADR-0153：凭据模型条目的可选公开别名 |
| [0154](0154-codex-memory-prompt-cache-key.md) | ADR-0154：Codex memory 请求派生稳定 prompt_cache_key |
| [0155](0155-background-oauth-refresh-configuration-rebase.md) | ADR-0155: 管理配置跨自动 OAuth 刷新透明 Rebase |
| [0156](0156-remove-redundant-local-request-id-header.md) | ADR-0156: 移除重复的本地请求 ID 响应头 |
| [0157](0157-standard-sk-gateway-key-prefix.md) | ADR-0157：GatewayApiKey 使用标准 `sk-` 前缀 |
