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
- 未来唯一允许的导入例外是 Provider 专用 OAuth2 JSON；原始 JSON 不持久化、不记录日志，也不能再次导出 Secret。
- 不实现运行态恢复、请求回放、队列恢复、会话恢复或复杂备份容灾。

## 3. 首个正式版本范围

- 后端：Rust；HTTP 框架：Axum/Tokio；前端：React + TypeScript。
- Provider 只实现 Codex 和 Claude。
- 上游 `ProviderCredential` 首版只支持 API Key；OAuth2 放在后续阶段。
- 实现：
  - `GET /v1/models`
  - `POST /v1/responses`
  - `POST /v1/responses/compact`
  - `POST /v1/messages`
  - `POST /v1/messages/count_tokens`
- 首版不实现 `/backend-api/codex/responses`、Codex WebSocket 和 Codex/Claude 双向跨协议路由。
- 首版只允许同协议路由：Responses → Responses，Messages → Messages。
- 公开模型名不强制 `codex/`、`claude/` 前缀；默认等于上游模型名，可配置本地别名。

## 4. 两类 Key 必须严格隔离

- `GatewayApiKey`：客户端访问 any2api 的本地网关凭据。
- `ProviderCredential`：any2api 访问 Codex/Claude 上游的凭据。
- 两者没有绑定、映射、所有权、配额或派生关系。
- `GatewayApiKey` 不得选择、过滤、固定或影响 `ProviderCredential`。
- 客户端认证头在进入 Provider Driver 前必须剥离；只有选中的 `ProviderCredential` 可以注入上游认证。
- 禁止把 Gateway Key、Provider API Key、OAuth Token、代理密码或原始 Session ID 写入日志。

## 5. 代理不变量

- 仅支持 `DIRECT`、`HTTP`、`SOCKS5`。
- 内置 `DIRECT` 不可删除、不可禁用。
- Credential 绑定 HTTP/SOCKS5：使用专属代理。
- Credential 绑定 DIRECT：继承全局代理。
- 全局代理也是 DIRECT：最终本机直连。
- 专属代理失败必须 Fail-Closed，禁止悄悄回退全局代理或本机直连。
- SOCKS5 默认使用远端 DNS；严格 SSRF 模式下禁止远端 DNS。
- Provider URL 默认只允许公网 HTTPS；普通 HTTP 和内网地址必须按 Endpoint 分别显式授权。

## 6. 调度、粘性与流式不变量

- 每个 Credential 的 `max_concurrency` 是硬限制，且必须大于 0。
- 负载比较使用 `in_flight / max_concurrency`，实现时用整数交叉相乘，禁止浮点误差。
- “选择 Credential”和“取得 Permit”必须是一个原子操作；CAS 失败后重新完整选择。
- `RuntimeRegistry` 跨配置代际复用容量状态；认证和健康状态按配置 generation 隔离。
- 所有等待使用有界 QueueTicket、统一 epoch 唤醒、超时和取消，禁止丢失唤醒。
- Codex `previous_response_id` 是硬粘性，绑定 Credential、Route Target、上游模型和协议方言。
- 普通会话是软粘性，支持 `prefer` 和 `strict`。
- 会话、并发、排队、冷却和熔断仅保存在内存；进程重启后全部清空。
- 旧 `previous_response_id` 在重启后没有绑定时返回 `session_binding_lost`，禁止猜测 Credential。
- 只有 `Pending` 且 RetrySafety 允许时才能重试或切换上游。
- 一旦向客户端写出 HTTP 响应头或任何字节，永久禁止切换上游。
- 流式 Body 必须持有 Permit 和取消令牌，EOF、错误、断连和 Drop 都只能释放一次。

## 7. 配置发布规则

- SQLite 是持久化配置真相来源，不使用巨型 YAML 作为运行时配置。
- 配置发布必须串行：事务内构造候选配置、完整校验和预编译，成功后 Commit，再执行无失败 reconcile 和单次 `ArcSwap<PublishedSnapshot>`。
- 网关鉴权和路由必须位于同一个 PublishedSnapshot revision。
- 管理 API 只有在数据库提交和快照切换完成后才能返回成功。
- 运行参数通过版本化 `SettingRegistry` 定义；代码保存默认值，SQLite 只保存用户覆盖值。
- Web 必须显示默认值、覆盖值和生效值，并支持恢复默认。
- 禁止在各模块中散落无法集中查询或覆盖的魔法常量。

## 8. 模块与依赖边界

- 采用模块化单体，不拆微服务。
- `domain` 不依赖 Web、SQLite、HTTP Client 或具体 Provider。
- `protocol` 只处理线协议编解码和兼容错误格式。
- `provider` 只处理供应商能力、Endpoint、认证注入、刷新协议和错误分类，不执行网络请求。
- `transport` 只处理 DIRECT/HTTP/SOCKS5、连接池和分阶段网络错误，不知道 Codex/Claude 业务。
- `runtime` 负责编排、调度、粘性、重试、健康状态和调用 Transport。
- `storage` 只处理 SQLite、Repository、Migration 和 Secret Vault。
- `server` 只处理 Axum 路由、中间件、鉴权和 DTO，不承载核心业务规则。
- `app` 是唯一 Composition Root，负责注册和装配具体实现。
- Runtime 只能依赖各 Adapter crate 的稳定 `api` 模块，禁止导入其内部实现。
- 新增 Provider 时，只允许局部增加 Provider 模块、必要协议实现、静态注册和契约测试；禁止修改中央调度器加入不断增长的 Provider `match`。

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

## 10. 安全与持久化

- SQLite 只持久化配置、必要凭据、Gateway Key 摘要和可选历史日志。
- `in_flight`、等待队列、健康、冷却、熔断、会话和请求进度不得持久化。
- Secret 使用版本化 AEAD 加密；主密钥位于数据库外，缺失或错误时启动失败。
- 管理 DTO 默认只返回 Secret 指纹或尾号，创建时仅显示一次明文。
- 远程管理默认关闭；启用后必须使用独立单管理员认证，允许 HTTP 或 HTTPS，TLS 推荐但不强制。
- 明文 HTTP 是受支持配置，不能在实现中强制跳转 HTTPS 或拒绝管理请求；Web 必须明确提示密码、Cookie 和 OAuth2 JSON 的明文传输风险。
- `GatewayApiKey` 不能登录管理面。
- 所有自定义 URL 必须经过结构化解析、SSRF 检查和重定向限制。

## 11. 工程与验证要求

- 搜索优先使用 `rg`/`rg --files`。
- 修改文件使用 `apply_patch`，保留用户已有改动，禁止破坏性 Git 操作。
- 修改具体代码前必须完整阅读该代码；跨模块修改前先核对依赖方向。
- 新功能必须同时提供模块级测试和对应契约/集成测试。
- 调度关键路径必须测试：永不超过并发上限、无丢失唤醒、Permit 只释放一次、热更新不重置容量状态。
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
