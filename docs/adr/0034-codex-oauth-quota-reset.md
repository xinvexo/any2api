# ADR-0034: Codex OAuth 额度查询与重置次数消费

- 状态：Accepted
- 日期：2026-07-24
- 决策者：maintainer

## 背景

Codex OAuth 账号的 ChatGPT 后端提供 5 小时/周限流窗口以及 `rate_limit_reset_credit`。现有 OAuthAccount 已具备登录、持久化、自动 Token 刷新和统一调度，但管理页的“刷新”只重新读取本地账号列表，无法查询上游额度，也不能在账号拥有可用重置次数时消费一次 credit。Token refresh 与 quota refresh 是两种不同能力，不能继续使用同一个模糊语义。

额度重置会消耗稀缺且不可撤销的上游 credit。浏览器中的旧计数不能作为执行依据，原始额度响应也不能扩张 OAuth JSON 明文持久化例外或泄露 Token。

## 决策

- 仅 Codex OAuthAccount 支持该能力；Claude 明确返回不支持，不增加伪实现。
- 管理 API 使用 `GET /api/admin/oauth/accounts/{id}/quota` 查询，使用 `POST /api/admin/oauth/accounts/{id}/quota/reset` 消耗一次重置次数；两者都受单管理员鉴权和 `no-store` 约束。
- Codex Driver 固定构造 `/backend-api/wham/usage`、`/backend-api/wham/rate-limit-reset-credits` 与 `/consume` 请求，注入 Bearer、`chatgpt-account-id` 和 Codex quota 所需固定头，并把受限响应解析成安全类型。Provider 不执行网络请求。
- Runtime 使用 OAuthAccount 固定 DIRECT 绑定解析出的全局代理和当前严格 SSRF 设置；不允许专属代理、重定向或隐式直连回退。响应正文按固定上限读取，错误正文不进入日志或管理响应。
- 查询遇到 401 时沿用 OAuth per-account refresh singleflight，最多刷新并重试一次。额度详情查询失败只允许回退到同次 usage 响应中明确给出的 reset credit 数据；缺失时保持未知，禁止猜测为可用。
- 重置按 OAuthAccount 串行。每次 POST 在持锁后重新执行额度查询，仅当最新 `available_count > 0` 才生成 UUID v4 `redeem_request_id` 并调用 consume；不相信客户端提交的次数。
- consume 成功且响应确认至少重置一个窗口后，清除该账号当前运行代际的 credential/model 临时冷却并推进 scheduler epoch。认证错误、Endpoint/Proxy 状态和其他账号不受影响。
- 管理 DTO 只返回主/次窗口、可用次数、credit 到期时间、抓取时间和已重置窗口数。额度快照不写 OAuth Provider JSON、SQLite、PublishedSnapshot、RequestLog、文件日志或浏览器持久存储。
- Web 先显式查询并展示额度；只有查询成功且可用次数大于 0 时启用重置，确认框明确提示会消耗一次，成功后立即重新查询。

## 备选方案

- 只在前端检查已查询次数：拒绝。浏览器状态可能过期或被伪造，无法保护不可逆 credit。
- 把额度快照写入 OAuthAccount JSON/SQLite：拒绝。它是短生命周期上游观测，不属于配置真相，也会扩大明文 OAuth JSON 和恢复语义。
- 把 Codex URL/JSON 写进 Server Handler：拒绝。Provider 协议必须留在 Driver，Runtime 才负责 Transport 编排。
- consume 成功后重建全部运行代际：拒绝。重置只改变上游额度窗口，集中清除当前账号的临时冷却即可。

## 后果

管理面会为一次额度查询发送一到两个上游请求；重置会先复核额度再发送一次不可逆 consume，并在前端再查询最新状态。该开销只由显式管理员操作触发，不进入公开数据面。Provider 接口增加可选 OAuth quota 能力，但新增 Provider 不需要修改中央调度器。

## 验证

- Provider 单元测试覆盖固定 URL/认证头、usage 主次窗口、reset credit 多种响应形状、非 Codex credit 过滤和 consume 响应校验。
- Runtime 测试覆盖 DIRECT/全局代理、严格 SSRF、正文上限、401 单次刷新、无次数拒绝、并发重置串行和成功后的临时冷却清理。
- 管理契约测试覆盖鉴权、Codex 查询/重置、Claude 不支持、DTO 脱敏和 Token 不出现在响应。
- Web 测试覆盖额度展示、零次数禁用、确认消费、成功后重新查询和错误状态。
