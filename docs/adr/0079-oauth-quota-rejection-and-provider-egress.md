# ADR-0079：OAuth 额度拒绝与 Provider 出口诊断分离

- 状态：Accepted
- 日期：2026-07-31
- 决策者：maintainer
- 修订：ADR-0036、ADR-0070

## 背景

OAuth 额度请求过去把所有 `403 Forbidden` 直接映射为账号访问受限。这个状态码也可能由 Provider 的区域策略、
IP 信誉或边缘访问策略产生；当 OAuthAccount 固定继承全局代理时，实际故障可能只是当前网络出口不被 OpenAI
接受。把二者合并会向管理员错误宣称账号被封禁，并可能诱导其停用或删除仍然有效的账号。

现有代理测试只访问 `https://example.com/`，只能证明通用 DNS、代理握手、TLS 和响应头可达，不能证明同一
出口符合某个 Provider 的访问策略。账号认证、账号限制和出口限制需要各自独立、可验证的证据。

## 决策

1. 除继续进入既有 Token 刷新链的 `401` 外，OAuth 额度非成功响应由 Provider Driver 读取有界响应，并从
   声明的结构化字段分类为“明确账号限制”“明确 Provider 出口拒绝”或“未分类”。禁止仅凭 `403` 或
   自然语言消息声明账号受限。
2. Codex 首批只读取顶层 `code` 及 `error.code/type`。`unsupported_country_region_territory` 表示出口拒绝；
   `account_deactivated`、`account_suspended` 和 `account_disabled` 表示账号限制。Grok 只在声明字段中识别
   `unauthorized:blocked-user` 为账号限制。未知码、字段位置不符、相互冲突的声明码和畸形正文保持未分类。
3. Codex 额度请求出现未分类 `403` 时，Runtime 执行 Driver 声明的无认证
   `GET https://chatgpt.com/backend-api/wham/usage`。它必须使用同一 PublishedSnapshot 中 OAuthAccount 实际
   继承的全局代理、代理认证和严格 SSRF 设置，不携带 Authorization、`chatgpt-account-id` 或其他账号材料，
   也不回退其他代理或本机直连。
4. Codex 探测的 `2xx` 或 `401` 表示出口已经到达 Provider 认证边界；`403` 表示无账号请求仍被拒绝，因此
   归类为当前 Provider 出口拒绝。其他 HTTP 状态、网络错误、超时、响应超限和无法识别的结果保持未知，
   不覆盖原始中性上游错误。
5. 出口探测按 `(Provider, 配置 revision)` 使用独立的进程内单飞槽和 30 秒缓存。共享槽表的锁只覆盖
   查找、插入与已完成过期项清理；网络探测在释放表锁后由该键自己的单飞槽执行。同键并发只执行一次，
   不同 Provider 或不同 revision 不互相等待。配置发布自然使用新键；缓存不持久化、不进入
   PublishedSnapshot。探测作为当前额度诊断的附属请求，不另占一个 RPM 名额。
6. 探测结果不更新 Proxy、Endpoint 或 Credential 健康。HTTP 策略拒绝不是代理握手故障；通用
   `example.com` 代理测试继续只表示公网连通性。
7. 管理 API 分别返回 `oauth_account_restricted`、`oauth_provider_egress_restricted` 和
   `oauth_quota_upstream_failed`。Web 文案必须分别描述账号限制、当前网络/全局代理出口拒绝和未知上游失败，
   不得使用“限制或封禁”覆盖多个原因。
8. “删除失效账号”仍只接受 `oauth_account_authentication_failed`。账号限制、出口拒绝、未分类 `403`、
   刷新无法确认和其他额度错误都不是删除候选。

## 后果

- 全局代理出口被 OpenAI 拒绝时，管理员会得到可操作的代理诊断，不再看到账号封禁误报。
- 明确账号限制仍保留独立诊断，但必须有 Provider 声明的结构化证据。
- 多账号批量额度刷新不会为同一 Provider 和配置代际重复制造出口探测洪峰。
- 一个 Provider 的慢探测不会串行阻塞其他 Provider 或新配置 revision 的额度诊断。
- Provider 新增或调整拒绝码时只修改自身 Driver 与契约测试，Runtime 不增加 Provider 分支。

## 验证

- Provider 测试覆盖账号码、区域码、未知 `403`、畸形正文和禁止递归扫描。
- Runtime 测试覆盖同一全局代理、无账号认证 Header、`401` 可达、`403` 出口拒绝、未知结果、同键缓存/
  单飞，以及不同 Provider/revision 在慢探测下仍可并行。
- HTTP/Web 测试覆盖三个稳定错误码和文案，并确认账号限制与出口拒绝都不会进入删除集合。
