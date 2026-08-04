# ADR-0034: Codex OAuth 额度查询与重置次数消费

- 状态：Accepted
- 日期：2026-07-24
- 决策者：maintainer
- 部分修订：ADR-0111

## 背景

Codex OAuth 账号的 ChatGPT 后端提供 5 小时/周限流窗口以及 `rate_limit_reset_credit`。管理面需要显式查询上游额度，并在账号拥有可用重置次数时消费一次 credit。Token refresh 与 quota refresh 是两种不同能力，必须保持独立语义。

额度重置会消耗稀缺且不可撤销的上游 credit。浏览器中此前读取的计数不能作为执行依据，原始额度响应也不能扩张 OAuth JSON 明文持久化例外或泄露 Token。

## 决策

- 额度重置能力仅适用于 Codex OAuthAccount；Claude 与 Grok 只提供各自的只读额度查询，不实现重置。
- 管理 API 使用 `GET /api/admin/oauth/accounts/{id}/quota` 读取最后一次安全快照，使用 `POST /api/admin/oauth/accounts/{id}/quota/refresh` 查询上游并持久化，使用 `POST /api/admin/oauth/accounts/{id}/quota/reset` 消耗一次重置次数；三者都受单管理员鉴权和 `no-store` 约束。
- Codex Driver 固定构造 `/backend-api/wham/usage`、`/backend-api/wham/rate-limit-reset-credits` 与 `/consume` 请求，注入 Bearer、`chatgpt-account-id` 和 Codex quota 所需固定头，并把受限响应解析成安全类型。Provider 不执行网络请求。
- Runtime 使用 OAuthAccount 固定 DIRECT 绑定解析出的全局代理和当前严格 SSRF 设置；不允许专属代理、重定向或隐式直连回退。响应正文按固定上限读取，错误正文不进入日志或管理响应。
- 查询遇到 401 时沿用 OAuth per-account refresh singleflight，最多刷新并重试一次。额度详情查询失败只允许回退到同次 usage 响应中明确给出的 reset credit 数据；缺失时保持未知，禁止猜测为可用。
- 重置按 OAuthAccount 串行。每次 POST 在持锁后重新执行额度查询，仅当最新 `available_count > 0` 才生成 UUID v4 `redeem_request_id` 并调用 consume；不相信客户端提交的次数。
- consume 成功且响应确认至少重置一个窗口后，清除该账号当前运行代际的 credential/model 临时冷却并推进 scheduler epoch。认证错误、Endpoint/Proxy 状态和其他账号不受影响。
- 管理 DTO 使用通用窗口列表，只返回安全窗口、可用次数、credit 到期时间、抓取时间和已重置窗口数。最后一次成功快照按 ADR-0111 写入独立 SQLite 表，但不写 OAuth Provider JSON、PublishedSnapshot、RequestLog、文件日志或浏览器持久存储，也不用于恢复路由健康。
- Web 先读取持久化快照并允许显式刷新；只有最新成功查询确认可用次数大于 0 时启用重置，确认框明确提示会消耗一次，成功后删除重置前快照并立即重新查询。

## 备选方案

- 只在前端检查已查询次数：拒绝。浏览器状态可能过期或被伪造，无法保护不可逆 credit。
- 把额度快照写入 OAuthAccount JSON 或配置快照：拒绝。ADR-0111 只允许写入独立、版本化的安全展示表，不扩大 OAuth JSON，也不恢复运行态。
- 把 Codex URL/JSON 写进 Server Handler：拒绝。Provider 协议必须留在 Driver，Runtime 才负责 Transport 编排。
- consume 成功后重建全部运行代际：拒绝。重置只改变上游额度窗口，集中清除当前账号的临时冷却即可。

## 后果

管理面 POST refresh 会发送一到两个上游请求；重置会先复核额度再发送一次不可逆 consume，并在前端再查询最新状态。真实使用后的活动刷新也可异步触发同一只读查询，但按账号合并且不阻塞公开数据面。Provider 接口增加可选 OAuth quota 能力，但新增 Provider 不需要修改中央调度器。

## 验证

- Provider 单元测试覆盖固定 URL/认证头、usage 主次窗口、reset credit 多种响应形状、非 Codex credit 过滤和 consume 响应校验。
- Runtime 测试覆盖 DIRECT/全局代理、严格 SSRF、正文上限、401 单次刷新、无次数拒绝、并发重置串行和成功后的临时冷却清理。
- 管理契约测试覆盖鉴权、Codex 查询/重置、Claude/Grok 无重置能力、DTO 脱敏和 Token 不出现在响应。
- Web 测试覆盖额度展示、零次数禁用、确认消费、成功后重新查询和错误状态。
