# ADR-0150: OAuth 默认出口与账号级代理选择

- 状态：Accepted
- 日期：2026-08-14
- 决策者：maintainer
- 修订：取代此前 ADR 中“ProviderCredential 的 DIRECT 继承全局”和“OAuthAccount 固定 DIRECT”的代理解析语义

## 背景

现有配置把 `ProviderCredential` 的 `DIRECT` 绑定解释为继承全局代理。因此管理员为了 OAuth 登录或订阅账号选择出口时，会同时改变所有绑定 DIRECT 的 API Key 上游、模型探测和数据面流量。API Key 已经拥有逐 Credential 的显式代理绑定，这种继承把两套独立控制面耦合在一起，也让 DIRECT 不再表示可直接判断的本机出口。

OAuthAccount 已有独立 `proxy_profile_id`，但原始 Schema 把它固定为 `DIRECT`，运行时也忽略该字段。管理员既需要一条方便统一调整的 OAuth 默认出口，也需要让特定账号手动选择任意 Profile，包括明确的本机 DIRECT。若继续用 DIRECT 表示“继承全局”，同一个值会在 ProviderCredential 和 OAuthAccount 中产生相反含义，因此必须把继承状态独立建模。登录、刷新、额度和数据面必须共享同一套 OAuth 专用解析规则，并保持可审计且 fail-closed。

## 决策

1. 全局代理只作为 OAuth 默认出口，不再影响任何 `ProviderCredential`、API Key 模型探测或 API Key 数据面请求。
2. `ProviderCredential` 始终按自身 `proxy_profile_id` 解析。绑定 `DIRECT` 表示本机直连；绑定 HTTP/SOCKS5 表示使用该 Credential 的专属代理。
3. OAuth 代理选择是明确的和类型化的：`Global` 表示跟随 OAuth 默认出口，`Profile(id)` 表示严格使用该 Profile。`Profile(DIRECT)` 固定本机直连，绝不表示继承。SQLite 用 nullable `oauth_accounts.proxy_profile_id` 保存该语义：NULL 是 `Global`，非 NULL 是 `Profile(id)`。
4. 交互式 OAuth 登录由管理员先选择 `Global` 或具体 Profile。每个登录网络阶段都按该选择解析；成功后无论新建还是重新授权已有账号，都把本次显式选择写入最终账号。Provider JSON 导入默认保存 `Global`。
5. API Key 或 OAuth 的显式代理失败继续 fail-closed，禁止回退 OAuth 默认出口或本机直连。OAuth 默认出口失败也禁止为继承它的登录、刷新、额度或数据面另设隐式直连路径。
6. 配置编译时，ProviderCredential 直接读取其绑定 Profile；OAuthAccount 使用 OAuth 专用解析函数处理 `Global` 或 `Profile(id)`。两类结果仍投影到同一 `RoutingCredential`，调度器不增加凭据来源分支。
7. 修改 OAuth 默认出口只能改变选择 `Global` 的 OAuth 登录 session 与 OAuthAccount；不得改变 API Key 或选择具体 Profile 的 OAuthAccount。修改账号选择必须增加其配置与路由健康代际，并随同一串行配置发布切换。
8. SQLite 的 `proxy_settings.global_proxy_profile_id` 与管理 API 的 `global_proxy_id` 保留；追加前向 Migration 把既有 OAuthAccount 的 DIRECT 标记转换为 NULL，并把该列改为 nullable 外键，不修改冻结的历史 Migration。管理 Web 将全局选择明确标为 OAuth 默认出口，并允许登录和账号编辑选择 `Global` 或具体 Profile。

## 后果

- 管理员调整 OAuth 出口不会再意外迁移 API Key 流量。
- API Key 若需要 HTTP/SOCKS5，必须在对应 Credential 上显式绑定该 Profile；DIRECT 始终可预测为本机直连。
- OAuth 账号可以统一继承默认出口，也可以按账号隔离线路或明确固定本机直连；登录、刷新、额度和数据面仍共享同一解析语义，保留一致的严格 SSRF、连接池隔离、故障归因和无回退语义。
- 这是新项目当前契约的直接修正，不保留 API Key DIRECT 继承或 OAuth DIRECT 继承的双重语义；Migration 显式把既有 OAuthAccount 转换为 `Global`。

## 验证

- Domain/Runtime 测试证明在 OAuth 默认出口为 HTTP/SOCKS5 时，DIRECT API Key 与 `Profile(DIRECT)` OAuthAccount 都固定本机直连，只有 `Global` OAuthAccount 使用默认出口，其他显式 OAuthAccount 使用自身代理。
- 修改全局代理后，API Key 模型探测 scope 不变化；其显式绑定 Profile 换代后 scope 才变化。
- Migration 升级测试保留代表性 OAuthAccount 与子表数据，并证明升级后可以写入具体代理绑定且外键完整。
- Web 将选中 Profile 标记为 OAuth 默认出口，Provider Credential 列表不再显示 DIRECT 继承，并在 OAuth 登录与账号编辑中提供代理选择。
- OAuth 登录、刷新、额度与数据面测试证明 `Global` 与 `Profile(id)` 都使用同一 OAuth 解析规则，且失败不隐式回退。
