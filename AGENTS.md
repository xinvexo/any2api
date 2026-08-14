# any2api 项目规则

本文件适用于整个仓库。实现任何功能前，都必须遵守以下规则。

## 1. 架构文档是实现基线

- 涉及跨模块设计、数据模型、协议、调度、代理、鉴权、存储或安全的修改前，必须完整阅读 `ARCHITECTURE.md`。
- `ARCHITECTURE.md` 是当前架构真相来源；实现不能与其中已确认的不变量冲突。
- 用户新增或修改架构要求时，先更新架构文档，再实现代码。
- 重大取舍、架构例外和边界调整写入 `docs/adr/`，不能只存在于聊天或提交信息中。
- `reference/` 仅用于只读参考 CLIProxyAPI、sub2api 和 new-api 的行为，禁止修改或复制其臃肿结构。

## 2. 项目定位与永久边界

- 项目是个人使用、自托管、单节点运行的 AI API 聚合代理。
- 永久不做：用户注册、多租户、套餐、余额、充值、计费、支付、API Key 销售和多节点分布式调度。
- 不引入 Redis、PostgreSQL、消息队列或微服务来解决单节点问题。
- 不提供通用配置、数据库或 Secret 导入导出。
- OAuth2 JSON 只允许作为独立 `OAuthAccount` 明文存入 SQLite；不进入日志、管理响应或浏览器存储，也不提供读取/导出端点。
- 不实现运行态恢复、请求回放、队列恢复、会话恢复或复杂备份容灾。

## 3. 首个正式版本范围

- 后端：Rust；HTTP 框架：Axum/Tokio；前端：React + TypeScript。
- Provider 实现 Codex、Claude、Grok 和 Kimi；Kimi 是独立服务身份，不得借用 Codex/Grok Driver。
- 上游 `ProviderCredential` 只支持 API Key；Codex、Claude 和 Grok 的 OAuth2 账号独立管理，但在运行时与 Provider API Key 编译到同一个路由候选池。Kimi 首版不支持 OAuthAccount。
- 实现：
  - `GET /v1/models`
  - `POST /v1/responses`
  - `POST /v1/responses/compact`
  - `POST /v1/chat/completions`
  - `POST /v1/images/generations`
  - `POST /v1/images/edits`
  - `POST /v1/messages`
  - `POST /v1/messages/count_tokens`
- 首版不实现 `/backend-api/codex/responses`、Codex WebSocket 和 Codex/Claude 双向跨协议路由。
- 首版允许 Responses → Responses、Responses → Chat Completions、Chat Completions → Chat Completions、Images → Images 和 Messages → Messages；不注册其他组合。
- 公开模型名不强制 Provider 前缀，首版固定等于上游模型名，不提供别名编辑。

## 4. 两类 Key 必须严格隔离

- `GatewayApiKey`：客户端访问 any2api 的本地网关凭据。
- `ProviderCredential`：管理员配置的 API Key 上游凭据。
- `OAuthAccount`：SQLite 中独立持久化的 Provider OAuth2 账号；不是 `ProviderCredential`，但可作为同等的上游路由凭据。
- 两者没有绑定、映射、所有权、配额或派生关系。
- `GatewayApiKey` 不得选择、过滤、固定或影响 `ProviderCredential` 或 `OAuthAccount`。
- 客户端认证头在进入 Provider Driver 前必须剥离；只有调度器选中的 Provider API Key 或 OAuth 账号可以注入上游认证。
- 禁止把 Gateway Key、Provider API Key、OAuth Token、代理密码或原始 Session ID 写入普通 tracing/file log、模型 `RequestLog`、错误正文、`Debug`、日志变更 SSE 或浏览器持久化状态。ADR-0081 定义的 `HttpAccessLog` 客户端侧原始 HTTP 交换是唯一日志例外：它按操作员选择原样捕获这些字段，且只允许通过已认证的系统日志详情读取。

## 5. 代理不变量

- 仅支持 `DIRECT`、`HTTP`、`SOCKS5`。
- 内置 `DIRECT` 不可删除、不可禁用。
- ProviderCredential 绑定 HTTP/SOCKS5：使用专属代理；绑定 DIRECT：始终从本机直连，不受全局代理影响。
- 全局代理只作为 OAuth 默认出口。OAuthAccount 的代理选择必须显式区分“跟随 OAuth 全局路由”和“使用指定 Profile”，禁止复用 DIRECT 表达继承。
- OAuthAccount 选择指定 DIRECT 时固定本机直连；选择指定 HTTP/SOCKS5 时固定使用该代理；选择跟随全局时才解析 OAuth 默认出口。
- OAuth 登录必须手动选择“跟随全局”或指定 Profile；该选择写入新建或重新授权后的账号。Token 刷新、额度操作和 OAuth 数据面始终复用账号选择，禁止另设隐式回退路径。
- OAuth 默认出口也是 DIRECT 时，只有选择跟随全局的 OAuth 流量最终从本机直连。
- 专属代理失败必须 Fail-Closed，禁止悄悄回退全局代理或本机直连。
- SOCKS5 默认使用远端 DNS；严格 SSRF 模式下禁止远端 DNS。
- Provider Base URL 只要求是结构合法的 HTTP(S) 地址；管理员填写的 URL 是受信任目标，不再提供或持久化普通 HTTP/内网地址授权开关。
- DIRECT 与严格本地 DNS 模式可以解析并固定目标地址，但不得按公网/私网地址类别拒绝管理员配置的 Endpoint。

## 6. 调度、粘性与流式不变量

- 每个运行时路由凭据（ProviderCredential 或 OAuthAccount）只允许配置一套可选的 `requests_per_minute`；`NULL` 表示不做本地限速，禁止再增加可配置并发或 TPM 限制。
- RPM 使用进程内滚动 60 秒窗口；“选择 Credential”和“预留 RPM 名额”必须是一个原子操作，失败后重新完整选择。
- `RuntimeRegistry` 跨配置代际复用 RPM 窗口和 `in_flight` 观测状态；认证和健康状态按配置 generation 隔离。
- `in_flight` 只用于运行态观测、流式资源生命周期与停机诊断，不参与准入、候选排序或 RPM 名额释放。
- 所有等待使用有界 QueueTicket、统一 epoch 唤醒、超时和取消，禁止丢失唤醒。
- 会话粘性只有一种绑定语义：绑定一旦建立，后续请求固定使用原 Credential、Route Target、上游模型和协议方言，禁止按模式降低绑定强度或重新选择目标。
- 普通显式 Session 未命中时允许首次建立绑定；Codex `previous_response_id` 必须命中已有绑定，未命中返回 `session_binding_lost`，禁止猜测 Credential。
- 会话、RPM 窗口、`in_flight`、排队、冷却和熔断仅保存在内存；进程重启后全部清空。
- `OAuthAccount.enabled` 只控制路由资格；停用账号仍参与到期前 Token 刷新以保持认证存活，只有删除账号才终止定时保活。
- 只有 `Pending` 且 RetrySafety 允许时才能重试或切换上游。
- 一旦向客户端写出 HTTP 响应头或任何字节，永久禁止切换上游。
- 流式 Body 必须持有运行态 Guard 和取消令牌，EOF、错误、断连和 Drop 都只能结算一次；流结束不得归还 RPM 名额。

## 7. 配置发布规则

- SQLite 是管理员配置与 OAuthAccount 的持久化真相来源，不使用巨型 YAML 作为运行时配置。
- 配置发布必须串行：事务内构造候选配置、完整校验和预编译，成功后 Commit，再执行无失败 reconcile 和单次 `ArcSwap<PublishedSnapshot>`。
- 网关鉴权和路由必须位于同一个 PublishedSnapshot revision。
- 管理 API 只有在数据库提交和快照切换完成后才能返回成功。
- OAuth 登录只有在 SQLite 提交、Runtime reconcile 和快照切换完成后才能返回成功；Token 刷新也必须通过版本 CAS 和串行发布整批切换。
- 运行参数通过版本化 `SettingRegistry` 定义；代码保存默认值，SQLite 只保存用户覆盖值。
- Web 必须显示默认值、覆盖值和生效值，并允许修改覆盖值；不得提供“恢复默认”按钮或其他浏览器侧清除覆盖入口。底层管理 API、ConfigPublisher 与存储仍保留删除覆盖记录的能力。
- `models.allowed` 是全局公开模型访问策略：显式模式 `"all"` 表示允许当前 PublishedSnapshot 中的全部公开模型，数组表示只按精确公开模型名放行，其中空数组表示不开放任何模型；它必须随同一 PublishedSnapshot revision 热更新，在路由、RPM 预留和上游 I/O 前执行，并同步过滤 `GET /v1/models`。配置发布后已经没有任何 Route 的名称必须在同一事务中从数组和 SQLite 覆盖值自动移除，裁剪为空后仍保持空数组的禁止全部语义。Gateway Key 不得影响该策略，该策略也不得改写 ProviderCredential 或 OAuthAccount 各自的模型选择。
- 禁止在各模块中散落无法集中查询或覆盖的魔法常量。

## 8. 模块与依赖边界

- 采用模块化单体，不拆微服务。
- `domain` 不依赖 Web、SQLite、HTTP Client 或具体 Provider。
- `protocol` 只处理线协议编解码和兼容错误格式。
- `provider` 只处理供应商能力、Endpoint、认证注入、OAuth JSON Schema、刷新协议和错误分类，不执行网络请求。
- `transport` 只处理 DIRECT/HTTP/SOCKS5、连接池和分阶段网络错误，不知道 Codex/Claude 业务。
- `runtime` 负责编排、调度、粘性、重试、健康状态和调用 Transport。
- `storage` 只处理 SQLite、Repository、Migration 和本地 Secret 持久化；OAuthAccount 使用独立 Repository，禁止复用 ProviderCredential 表。
- `server` 只处理 Axum 路由、中间件、鉴权和 DTO，不承载核心业务规则。
- `app` 是唯一 Composition Root，负责注册和装配具体实现。
- Runtime 只能依赖各 Adapter crate 的稳定 `api` 模块，禁止导入其内部实现。
- 新增 Provider 时，只允许局部增加 Provider 模块、必要协议实现、静态注册和契约测试；禁止修改中央调度器加入不断增长的 Provider `match`。
- ProviderCredential 与 OAuthAccount 的管理模型保持分离；两者只能在通用 `RoutingCredential` 投影处合流，禁止复制第二套调度、RPM、粘性、健康或重试实现。

## 9. 拒绝臃肿文件

- 一个文件只承担一个清晰职责。
- `main.rs`、`lib.rs`、`mod.rs` 只做声明、导出和装配，不放大段业务逻辑。
- 禁止垃圾桶式 `utils.rs`、`common.rs`、`manager.rs`、`service.rs`；公共代码必须按具体领域命名。
- 生产源文件目标不超过 300 行代码。
- 401–600 行必须进入机器可读 Allowlist，包含 path、reason、ADR、owner 和 expires_at。
- 超过 600 行由 CI 拒绝；例外仅限生成代码、静态协议表、测试夹具和 Migration。
- 单个函数建议不超过 80 行；复杂逻辑拆为具名阶段、状态转换和可独立测试的模块。
- 禁止为规避行数而机械拆文件，拆分必须对应真实领域职责。
- React 按 feature 拆分；页面只负责组合，业务状态放 hooks/model，API 调用放 feature/api。
- feature 之间只能通过公开出口依赖，禁止深层导入其他 feature 的内部文件。
- `E:\clashx` 只用于核对前端技术栈，不得复制其 Tauri 桌面布局、固定侧栏、窗口按钮、vibrancy 或巨型页面/CSS。
- Web 必须响应式、支持 URL/deep link、自然滚动、文本选择和键盘可访问性；视觉保持现代、克制、偏 macOS，但不得花哨。
- 样式使用语义 Token 并按职责拆分；重型依赖只在出现真实功能需求时按需引入。
- `app/any2api/web-assets` 是由 Vite 生成并提交的内嵌产物，禁止手工编辑；前端变更使用 `pnpm build:embedded` 同步，提交前使用 `pnpm check:embedded` 校验。
- Rust `build.rs` 只能读取已提交的内嵌资源并生成 `OUT_DIR` 清单，禁止调用 Node/pnpm、联网或修改工作树。

## 10. 安全与持久化

- `0001_initial.sql` 和所有后续编号 Migration、checksum 一经进入仓库即冻结；任何 Schema 变化只允许追加编号连续的前向 Migration，不因项目尚未正式发布而重写或删除历史脚本。
- 每个改变既有 Schema 的 Migration 必须提供带代表性数据的升级测试；生产代码仍只面向完整迁移后的最新 Schema，不保留双轨领域模型或运行时 Schema 分支。
- 默认保留已有数据；若用户明确拒绝兼容某个旧格式，必须由 ADR 记录，并在 Migration 修改任何结构或数据前拒绝非空旧记录，禁止静默删除和兼容代码回流。
- any2api 自身历史版本的兼容与数据转换只能存在于编号 SQL Migration；生产 Rust/TypeScript 只接受当前 Schema、当前 HTTP 契约和当前浏览器状态格式，禁止旧字段别名、双轨读取、启动期重写、代码内迁移和废弃转发层。CLIProxyAPI/Sub2API OAuth 导入是当前明确支持的外部输入协议，必须在导入边界规范化，不能让其结构进入 SQLite 当前 Schema 或运行时读取路径。
- SQLite 只持久化配置、必要凭据、Gateway Key 明文与校验摘要、OAuthAccount 原始 JSON 和可选历史日志。
- RPM 滚动窗口、`in_flight`、等待队列、健康、冷却、熔断、会话和请求进度不得持久化。
- Provider API Key、代理密码、Gateway Key 和 OAuth JSON 都按产品决策明文存入 SQLite。数据目录与其中的数据库、WAL、锁和日志文件是唯一的本地持久化保护边界；除 ADR-0081 的已认证 `HttpAccessLog` 原始交换例外外，Secret 仍禁止进入日志、非必要 DTO、`Debug` 或浏览器持久化状态。
- 管理 DTO 对 Provider Secret 默认只返回指纹或尾号，创建时仅展示一次；`GatewayApiKey` 例外：明文持久化，管理列表始终可查看。
- 远程管理默认开启，但不改变默认 loopback 监听地址；管理面必须使用独立单管理员认证，允许 HTTP 或 HTTPS，公网部署应由 Nginx/Caddy 等反向代理终止 TLS。
- 明文 HTTP 是受支持配置，不能在实现中强制跳转 HTTPS 或拒绝管理请求；Web 必须明确提示密码、Cookie 和 OAuth callback/code 的明文传输风险。
- `GatewayApiKey` 不能登录管理面。
- 所有自定义 URL 必须经过结构化解析并禁用自动重定向；客户端输入不得改变已发布 Provider Endpoint 的 authority，Provider Base URL 不按公网/私网地址类别设门禁。

## 11. 工程与验证要求

- 搜索优先使用 `rg`/`rg --files`。
- 修改文件使用 `apply_patch`，保留用户已有改动，禁止破坏性 Git 操作。
- 修改具体代码前必须完整阅读该代码；跨模块修改前先核对依赖方向。
- 新功能与修复必须在能够证明行为的最低充分层级提供测试；只有行为跨越模块边界、公开协议或真实 I/O 时才增加契约/集成测试。禁止为了完成清单而在模块、Runtime、HTTP 和 Web 多层机械重复同一分支；已有较低层测试能完整证明的纯实现细节不再复制到上层。
- 调度关键路径必须测试：滚动窗口永不超过 RPM、到期可重新准入、无丢失唤醒、运行态 Guard 只结算一次、热更新不错误重置有限 RPM 窗口。
- SSE 必须覆盖任意字节切分、CRLF、多行 data、无尾空行、提交前重试和提交后禁止切换。
- Provider/Protocol 契约测试必须枚举实际 Registry 中的实现，而不是按文件名猜测覆盖率。
- 提交前至少运行与改动相关的 fmt、clippy、test、前端 typecheck/lint/build。
- 不得为了让测试通过而放宽安全边界、删除错误分类或绕过架构不变量。

## 12. 新项目重构原则

- 本项目是新项目，不承担历史 API、内部类型、构造器、数据库模型或测试夹具的兼容包袱。
- 发现架构、领域模型、依赖方向或公开接口不合理时，直接完成正确重构，并同步迁移全部调用点、测试和文档。
- 禁止为了保留错误旧设计而增加兼容构造器、双轨模型、临时适配层、废弃字段或分支式补丁。
- 测试用于验证当前正确架构，不得反过来迫使生产代码保留已经确认不合理的设计。
- 重构仍须保持提交边界清晰、模块职责明确，并一次性通过相关契约和工程门禁。

## 13. 复杂度与防御必须成比例

- 优先实现当前明确需求的最短正确路径；十几行直接代码能够完整满足契约时，不得为了假设场景扩张成数百行防御代码。
- 禁止为尚未出现、无法触发或由内部强类型已经排除的问题叠加重复校验、兼容分支、兜底回退、抽象层、状态机或重试层。
- 额外防御只用于明确的外部不可信输入、已确认的架构不变量、并发与资源生命周期边界，或已经复现并写入测试的故障。
- 测试规模和实现复杂度必须与改动风险、影响范围和失败代价相称；小功能保持小实现、小测试面。
- 如果一个功能必须引入显著复杂度，代码和 ADR 必须能指出对应的真实风险，不能仅以“以后可能需要”作为理由。
- 本规则不允许绕过既有安全边界或并发正确性，但要求这些保护集中在真正的边界上，不在每一层重复堆叠。
