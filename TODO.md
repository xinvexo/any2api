# any2api 审查整改待办

> 来源：2026-08-03 Claude 全仓分域审查（`app-misc`、`protocol`、`provider`、`rt-config`、`rt-sched2`、`server`、`storage`、`web`）。
> 代码基线：`e2dd42f`。
> 审查原则：`ARCHITECTURE.md` 和现有 ADR 只代表当前实现基线与历史选择，**不能作为设计正确或审查意见错误的证据**。每项意见必须结合实际代码路径、可复现故障、协议事实、测试/基准和收益风险独立判断；确认有优化必要时，即使与现有架构决策冲突，也应先修订架构/ADR，再完成实现。
> 执行约束：从本规则加入起，本表后续分析、修改和验证**禁止使用子代理或多代理委派**，全部由当前主代理串行完成。
> 范围：按用户当前明确选择，不登记应用层加密、明文持久化、签名或 HttpAccessLog 原始交换脱敏任务；这项排除来自用户决策，而不是现有 ADR 的权威性。
> 当前开放 P0：**0**。

## 使用规则

| 状态 | 含义 |
|---|---|
| 待办 | 已由代码、协议或复现证据确认值得实施；若与当前文档冲突，实施前先修订文档 |
| 待验证 | 先补复现、故障注入或基准，证据成立后再实现 |
| 需 ADR | 审查意见有可信优化价值，但会改变当前架构、协议或产品边界；这不是拒绝理由，验证成立后必须先改 `ARCHITECTURE.md`/ADR 再实现 |
| 已完成 | 当前基线已经解决，并保留回归验证 |
| 不采纳 | 仅限用户明确排除、已经由当前代码解决，或独立证据证明属于误报/收益不成立；不得仅以“与现有架构或 ADR 冲突”为理由 |

优先级：P0 为发布阻断，P1 为核心正确性或高损失故障，P2 为重要可靠性/兼容性问题，OPT 为有证据或低风险的优化维护。

所有实现任务共同完成标准：

1. 不使用子代理或多代理委派；当前主代理亲自阅读相关代码、作出判断、修改并验证。
2. Claude 报告是问题线索而不是结论；先用实际代码、协议资料、复现测试、故障注入或基准确认问题及影响。
3. 不预设现有架构、不变量或 ADR 正确。若其阻碍已证实的正确性、可靠性、性能或可维护性改进，先更新 `ARCHITECTURE.md` 和对应 ADR，再实现代码与迁移全部调用点。
4. 先增加能够复现问题的模块级测试；跨模块行为同时增加契约或集成测试。修改既有安全或并发语义时，必须同时覆盖新语义和回归风险。
5. Schema 变化只追加连续前向 Migration，并提供带代表性数据的升级测试；不得修改既有 Migration/checksum。
6. 前端变更运行 typecheck、lint、相关单测和生产构建，并用 `pnpm build:embedded` 同步内嵌产物、`pnpm check:embedded` 校验。
7. 提交前运行受影响范围的 fmt、clippy、test 和架构门禁。

## P1：已确认需优先实施

| ID | 状态 | 模块 | 任务 | 完成标准 | 来源 |
|---|---|---|---|---|---|
| REV-PROTO-001 | 已完成 | Protocol | 修复 Responses → Chat 多轮续接重复追加 `instructions` | continuation 不保存顶层指令；当前轮仅在首部注入一次；重复、替换和省略的三轮模块/HTTP 契约已覆盖 | protocol P1-1 |
| REV-PROTO-002 | 已完成 | Protocol/Runtime | 为 Anthropic、Chat Completions、Images 补齐流终止分类 | 四类生成流均要求协议终止事件；失败事件先透传再按错误结算，截断 EOF 产生 Body error；健康成功延迟到成功终止，HTTP/运行时回归已覆盖 | protocol P1-2 |
| REV-PROTO-003 | 已完成 | Protocol | 禁止缺失 tool-call `index` 时统一落到 `0` | 缺失、负数和非整数 `index` 均在事件处 fail-closed；前提交返回协议错误，提交后 Body 失败并清理 Pending continuation；模块/HTTP 回归已覆盖 | protocol P1-3 |
| REV-PROTO-004 | 已完成 | Protocol/Runtime | 兼容无 `choices` 的 error chunk 与 usage-only chunk | error 投影为官方 Responses `error` 失败事件并保留 code/message/param；对象型纯 usage 尾包安全消费；失败可透传后 Abort Pending continuation，模块/HTTP 回归已覆盖 | protocol P1-4 |
| REV-PROTO-005 | 已完成 | Protocol | buffered 桥接响应采用响应侧向前兼容解析 | 未知 message 扩展字段（含 `annotations`）不再使响应失败；非空 `function_call`、`refusal`、`audio` 等无法无损投影的已知输出仍 fail-closed；请求侧严格校验不变，模块/HTTP 契约已覆盖 | protocol P1-5 |
| REV-STORAGE-001 | 已完成 | Storage | 用差异更新替代全量删除重建 Route/Target | 未变化 Target 保留 SQLite 行与历史 `request_attempts.route_target_id`，只删除真正失效项；复核确认 Secret 轮换必须继续清空旧 Key 的模型权限，但差异同步会保留其他 Credential 仍提供的 Target；模块与 HTTP 契约已覆盖 | storage P1-1；P2-12 的“无用重写”前提不成立 |
| REV-STORAGE-002 | 已完成 | Storage/Migration | 补齐删除路径依赖的外键索引 | Migration 0006 为 Attempt/RequestLog 的 RouteTarget、Credential、Proxy、Endpoint 外键补齐 5 个索引；带 2 MiB 代表数据的升级测试固定全部父表删除 EQP 与数据保留 | storage P1-2 |
| REV-STORAGE-003 | 已完成 | Storage/Migration | 优化系统日志分页 COUNT/摘要过滤 | Migration 0006 增加摘要过滤覆盖索引；COUNT 与分页显式锁定该索引，过滤阶段不读取 Body BLOB；升级测试固定 COUNT covering 与分页查询计划 | storage P1-3 |
| REV-WEB-001 | 已完成 | Web | 日志总数收缩后回写合法页码 | 仅在当前请求页的响应确认越界后异步校正查询状态并重新请求合法页；RequestLog/SystemLog 均覆盖第 3 页收缩到第 1 页后恢复数据的回归测试 | web P1-1 |
| REV-WEB-002 | 已完成 | Web | 增加根级和路由级错误恢复边界 | Provider 外层根边界与 Router `errorElement` 分层兜底，统一中文恢复页仅提供当前 deep link 重载且不暴露异常；Suspense fallback 可见且可访问，Provider/Router 外抛错均有测试 | web P1-3 |

## P2：已确认需实施

| ID | 状态 | 模块 | 任务 | 完成标准 | 来源 |
|---|---|---|---|---|---|
| REV-APP-001 | 已完成 | Updater | 清理中断后遗留的更新临时目录 | 更新器初始化与每次安装前扫描可执行文件同目录，只删除精确 `.any2api-update-<6 位 ASCII 字母数字>`、当前有效 UID 所有且为空或仅含同 UID 普通文件 `release.tar.gz`/`any2api.new` 的残留；近似名、普通文件、符号链接、异 UID、未知内容均不触碰。启动清理 best-effort 告警，安装前清理 I/O 失败明确终止本次安装。Updater 26、App 20+6、更新管理契约 3 项测试及 fmt/clippy/architecture/diff 门禁通过 | app-misc P2-3 |
| REV-APP-002 | 已完成 | File logging | 让文件日志 I/O 故障可见并可恢复 | Writer 每秒核对私有目录与活跃文件身份；准备失败只重试未写入阶段，`write_all` 失败不重放；首次故障立即直写 stderr、持续故障每 60 秒限频、恢复后单次通知。覆盖删除目录后 0700/0600 安全重建、普通文件占位 Fail-Closed 与外部恢复；App 23 项单测、6 项进程测试及 fmt/clippy/architecture/diff 门禁通过 | app-misc P2-4 |
| REV-APP-003 | 已完成 | Bootstrap | 在 SQLite 打开和 Migration 前建立 console tracing | Serve 在环境、Tokio 与 SQLite 前一次性安装 stderr console 和禁用的固定文件层；迁移后只激活 Writer 槽与有效级别，不重装 subscriber/动态替换 layer。损坏 SQLite 真实子进程验证 `phase=application` 与完整错误链且不提前建日志目录；App 24 项单测、7 项进程测试及 fmt/clippy/architecture/diff 门禁通过 | app-misc P2-5 |
| REV-APP-004 | 已完成 | Bootstrap/Storage | 拒绝空 `ANY2API_DATA_DIR` 落到 CWD | 空值按未设置处理并使用默认 `data/`；真实子进程回归确认数据库/实例锁不落到 CWD，目录保持 0700、文件保持 0600。App 20 项模块测试与 6 项进程测试及 fmt/clippy/architecture/diff 门禁通过 | app-misc P2-6 |
| REV-APP-005 | 已完成 | Integration tests | 并发排空子进程 stdout/stderr | `CapturedChild` 在 spawn 后立即用两个线程持续排空 pipe，正常退出、提前失败和 Drop 均 join；确定性超过 256 KiB 的输出回归保留 stdout 尾部与 stderr 诊断。8 项真实进程测试及 fmt/clippy/architecture/diff 门禁通过 | app-misc P2-7 |
| REV-APP-006 | 已完成 | Updater | 下载使用无进展超时而非整个请求总时限 | GitHub Client 固定 10 秒连接/30 秒可重置读取超时；元数据/checksum 保留短总时限，归档移除 300 秒总 deadline。流式本地 HTTP 回归覆盖跨完整读取窗口持续前进成功与停滞 `DownloadFailed`；Updater 28 项测试及 App 编译、fmt/clippy/architecture/diff 门禁通过 | app-misc P2-8 |
| REV-APP-007 | 已完成 | xtask | 严格解析 Allowlist `expires_at` | 使用 `time::Date` 严格解析并要求规范 `YYYY-MM-DD`；非法/不存在/非规范日期被拒绝，今日仍有效、过去失效、未来有效。xtask 9 项测试及 fmt/clippy/architecture/diff 门禁通过 | app-misc IMP-11 |
| REV-APP-008 | 已完成 | App/Updater | 增加 `--version` | `any2api --version` 在环境、数据目录、实例锁和 Tokio 前精确输出构建版本；其他参数 Fail-Fast；Updater 以清空环境、输出上限和 10 秒截止执行 staged 冒烟，错误版本/非零/超限/超时均拒绝且不改当前二进制；进程与模块测试已覆盖 | app-misc IMP-16；ADR-0089 |
| REV-PROTO-006 | 已完成 | Protocol | 补发 `response.reasoning_summary_part.done` | 按官方 Responses 事件契约在 `summary_text.done` 与 `output_item.done` 之间发送携带完整 summary part 的 `part.done`，正常完成不伪造 status；双 delta 生命周期与全流连续序号测试固定。Protocol 64+2 项测试及 fmt/clippy/architecture/diff 门禁通过 | protocol P2-6 |
| REV-PROTO-007 | 已完成 | Protocol | 生成合法的 Responses item ID 前缀 | Bridge 集中生成 `msg_resp_*`、`rs_resp_*`、`fc_resp_*` 类型化身份；buffered 完整 output 回传后原值保留，流式 added/delta/done 与最终 output 共用同一 ID。Protocol 64+2 项测试及 fmt/clippy/architecture/diff 门禁通过 | protocol P2-7 |
| REV-PROTO-008 | 已完成 | Protocol | 明确残缺流式 tool call 的 fail-closed 契约 | 所有工具状态在结算前统一验证非空 id/name；带 `finish_reason` 的终止 chunk 在返回合成事件前失败，`[DONE]` 也不产生局部 done；不跳过、不伪造、不建立 continuation。HTTP 契约固定提交前 502 与提交后 Body 错误/续接丢失。Protocol 66+2、public_sse 17 项测试及 fmt/clippy/architecture/diff 门禁通过 | protocol P2-8 |
| REV-PROTO-009 | 已完成 | Protocol | 拒绝没有 output 的孤立 `function_call` 历史 | Bridge 用预扫描的唯一 output call_id 集合验证当前 input 的每个 call；单个中断 call 和多 call 仅部分有 output 都在编码上游请求前失败，不合成占位结果；HTTP 契约确认 400 且未连接上游。Protocol 67+2、public_json_proxy 20 项测试及 fmt/clippy/architecture/diff 门禁通过 | protocol P2-9 |
| REV-PROTO-010 | 已完成 | Protocol | 统一 multipart `model` 校验值与路由值 | multipart parser 一次完成 UTF-8 + Unicode whitespace trim 并把规范名称存入结构化 payload，Adapter 直接用于路由；ASCII/Unicode 首尾空白、纯空白、重复 model 均覆盖，HTTP 契约确认 Unicode 名称可路由且重复字段不接触上游。Protocol 68+2、public_json_proxy 20 项测试及 fmt/clippy/architecture/diff 门禁通过 | protocol P2-10 |
| REV-PROTO-011 | 已完成 | Protocol | SSE 分帧支持裸 `\r` 行结束符 | 按 WHATWG 行语义把 CRLF pair、LF、裸 CR 分开处理，chunk 尾 CR 等待下一字节/EOF；分帧、payload 与模型改写共享语义。`\r\r`、CRLF、多行 data、单字节切分、无尾空行及真实跨 chunk HTTP 均覆盖。Protocol 68+2、public_sse 17 项测试及 fmt/clippy/architecture/diff 门禁通过 | protocol P2-11 |
| REV-PROTO-012 | 已完成 | Protocol | 将 `finish_reason=content_filter` 映射为 incomplete | buffered 与 SSE 复用唯一 finish-reason 映射：`length` 对应 `max_output_tokens`，`content_filter` 对应同名 reason；两者均生成 `status=incomplete`，SSE 发出 `response.incomplete` 且保持成功协议终止。Protocol 69+2、public_json_proxy 20 项测试及 fmt/clippy/architecture/diff 门禁通过 | protocol P2-12 |
| REV-PROVIDER-001 | 已完成 | Provider/Domain | 补全 409/413/422 和 Claude `request_too_large` 错误矩阵 | 共享基线将 409/413/422 固定为 `InvalidRequest + Ambiguous`，不盲重放非幂等请求且不惩罚健康状态；Claude 413 `request_too_large` fixture 正确分类，当前官方仍存在的 `billing_error` 额度兼容明确保留。最终上游状态/Header/正文透明。Domain 47、Provider 78、Registry 2、public_reliability 37 项测试及 fmt/clippy/architecture/diff 门禁通过；ADR-0098 | provider P2-5、P2-6 |
| REV-PROVIDER-002 | 已完成 | Provider | 正确处理 Grok 非 ASCII upstream model Header | 实测推翻审查报告的 ASCII 前提：项目 `http 1.4.2` 与 xAI 官方 Grok Build 所用 1.4.x 都会接受 UTF-8 高位字节。实现改用 `HeaderValue::from_bytes(model.as_bytes())` 明确按 UTF-8 原始字节确定传输，不增加错误的 ASCII 配置限制；绕过领域类型的控制字节返回 `UnsupportedOAuthModel`，不再误报 `InvalidResponse`，API Key 路径不受影响。Provider 79、Registry 2 项测试通过；ADR-0099 | provider P2-9 |
| REV-PROVIDER-003 | 已完成 | Provider | 让 Header 投影预算具有确定优先级 | 精确白名单数组成为显式高到低优先级，按名称 `get_all` 保留同名插入顺序；前缀按声明顺序分组、组内字典序，彻底移除对 HeaderMap 任意跨名称迭代顺序的依赖。总字节候选超限只跳过该值并继续接纳较小字段，64 值预算耗尽才停止；Codex/Claude/Grok 白名单按协议/绑定语义重排，未获准的 Codex 凭据绑定字段在预算前排除。Provider 81、Registry 2 项测试通过；ADR-0100 | provider P2-10 |
| REV-PROVIDER-004 | 已完成 | Provider | OAuth 刷新拒绝支持仅顶层 `code` | 仅在 HTTP 400/401 且声明字段精确为 `invalid_grant` 时判定永久失效；仅顶层 `code` 已由模块与额度查询 HTTP 契约覆盖，空对象、未知/非字符串 code、message 猜测和暂时状态仍为 Unverified。Provider 73、完整契约 149 项测试及 fmt/clippy/architecture/diff 门禁通过 | provider P2-11 |
| REV-PROVIDER-005 | 已完成 | Provider | 解析 Claude `retry-after-ms` | 按 Anthropic 官方 SDK 行为优先采用合法毫秒值，缺失、负数、NaN/非有限或非法值回落标准 `Retry-After`；两者统一为 `RetryAfterHint` 并限制到 30 天，其他 Provider 仍只读标准 Header。Provider 76 项测试及 fmt/clippy/architecture/diff 门禁通过 | provider IMP-17 |
| REV-PROVIDER-006 | 已完成 | Provider | Grok JWT 的 `sub`/`email` 按字段回落 | 身份优先级按字段独立执行 ID token → access token → 顶层响应；ID token 仅有 email 或仅有 sub 时，另一字段均可从 access token 补齐，Token Debug 仍不暴露原始 JWT。Provider 74 项测试及 fmt/clippy/architecture/diff 门禁通过 | provider IMP-17 |
| REV-RUNTIME-001 | 已完成 | Runtime OAuth | `unique_label` 截断后再次 `trim_end()` | 100 字符边界与同批重名后缀均由模块测试固定；真实批量导入在截断点落于空格时仍一次发布成功，不产生 `LabelNotTrimmed`。Runtime 221 项测试及 fmt/clippy/architecture/diff 门禁通过 | rt-config P2 |
| REV-RUNTIME-002 | 已完成 | Runtime OAuth quota | `EgressProbeCache` 改为 Provider/revision 粒度单飞 | 缓存键改为 `(ProviderKind, ConfigRevision)`，每键使用独立 `OnceCell`；共享表 mutex 只做槽查找、插入和已完成过期项清理，网络 await 全部在锁外。同键 8 个并发只执行一次并复用 30 秒结果；不同 Provider 及同 Provider 不同 revision 的慢探测可同时进入。Runtime quota 23、OAuth quota HTTP 契约 8 项测试及 fmt/clippy/architecture/diff 门禁通过 | rt-config P2 |
| REV-RUNTIME-003 | 已完成 | Runtime telemetry | 修正 `queued`/`dropped` 指标语义 | 内部 queue slot 继续维持原有有界准入，公开指标明确拆为 channel 内 `queued_records`、Writer 已接收未结算 `in_flight_records`、累计 `persisted_records`/`dropped_records`；控制消息不污染记录数，Gateway 批次折叠按原始记录守恒。Writer 正常/失败/超时 abort 并 join 后把所有未获存储终态记录计入 dropped，停机回归固定 2 queued + 1 in-flight = 3 dropped。Runtime 224、Server 56、系统日志契约 2、Web 254 项测试及 fmt/clippy/typecheck/lint/build/embedded/architecture/diff 门禁通过 | rt-config IMP；ADR-0015 |
| REV-RUNTIME-004 | 已完成 | Runtime balancing | 过滤计数按请求而非等待唤醒次数记录 | 普通路由与固定会话选择共用请求级 `RequestFilterRecorder`；每个请求对同一 `RoutingCredentialId`+原因最多累加一次，不同原因仍分别计数。回归覆盖入队立即复查与连续 4 次 epoch 广播后 rate-limit filtered 仍为 1，固定凭据 5 次复查亦为 1；独立请求仍分别累加。Runtime 226、balancing 管理契约 1 项测试及 fmt/clippy/architecture/diff 门禁通过 | rt-sched2 P2-4；ADR-0038 |
| REV-RUNTIME-005 | 已完成 | Runtime retry | 非零指数退避按 Credential 计算 | 未绑定失败先排除已失败路径并立即重新选择，不为 Credential 切换等待旧路径退避；只有绑定请求复用同一 Credential 时才按该 Credential 的请求内尝试次数执行指数退避。模块测试固定 A 的 1s/2s 与首次切换到 B 重置为 1s，契约测试固定总预算仅 1s、退避配置 2s 时仍能立即切换成功。Runtime 227、公开可靠性契约 38 项及 fmt/clippy/architecture/diff 门禁通过 | rt-sched2 IMP-1；ADR-0013 |
| REV-SERVER-001 | 已完成 | Server | zstd 解压接入进程 lifecycle 并限制并发 | `AppState` 共享专用 `ZstdDecoder`，最多 2 个作业进入 Tokio blocking 池；等待许可不返回 429、不预留 Body 字节且可随请求取消。取得许可后压缩输入与 permit 一起移入 `ProcessLifecycle` 的受管 closure，waiter 取消或 Forced 均不会提前释放或绕过停机追踪。测试覆盖有效/损坏/超限输入、并发上限、排队取消、已启动任务取消与强制停机；Server 59、Body 契约 8、公开 JSON/zstd 契约 20、App shutdown 8 项及 fmt/clippy/architecture/diff 门禁通过 | server P2-4；ADR-0026/0082 |
| REV-SERVER-002 | 已完成 | Server | `AdminAuthService` 缺失时 fail-closed | `AdminAuthService` 已成为 `AppState` 必需构造依赖，生产 Composition Root 与全部契约夹具均显式注入；删除可选状态、`with_admin_auth`、loopback 免认证模块及不可达错误码。共享契约夹具使用真实密码/Session/CSRF 且只为直连 loopback 请求注入，远程无会话统一返回 401；专门的 Setup/CSRF 契约仍走真实手工流程。Server 56、App 20+6、完整契约 149 项测试及 fmt/clippy/architecture/diff 门禁通过 | server IMP |
| REV-SERVER-003 | 已完成 | Server | 为未知 HTTP version 提供安全兜底 | 移除 non-exhaustive `http::Version` 上的 panic 分支；未来未知版本在持久化模型扩展前保守记录为 HTTP/1.1，访问日志链路继续可用；五种当前版本映射由模块测试固定。Server 56 项测试及 fmt/clippy/architecture/diff 门禁通过 | server IMP |
| REV-STORAGE-004 | 已完成 | Storage | 将写路径 `assert_eq!` 改为类型化一致性错误 | 7 个仓储写路径的 21 处写后断言统一改为无 `Debug` 数据要求的类型化 `ConfigurationWriteMismatch`，覆盖 revision、Gateway Key、代理、OAuth、Provider Credential/Endpoint、Route 与 Settings 8 类组件；错误只标识组件、不格式化含明文 Secret 的实际/期望配置，候选事务自然回滚。Storage 76、Runtime 配置发布 26 项测试及 fmt/clippy/architecture/diff 门禁通过 | storage P2-5 |
| REV-STORAGE-005 | 已完成 | Storage | 单条坏遥测不再毒化整页 | RequestLog 分页只隔离明确的 `CorruptTelemetry` 行并按查询发出一次 `corrupt_rows` 计数告警，窗口 `total` 仍表示实际持久化行；SQL/事务错误和损坏详情继续失败，配置与 Secret 的 fail-closed 未放宽。模块与真实管理 HTTP 契约覆盖合法兄弟行可读、损坏详情 500；Storage 80、Overview 管理契约 2 项及 fmt/clippy/architecture/diff 门禁通过 | storage P2-8 |
| REV-STORAGE-006 | 已完成 | Storage | 提高清理收敛速度 | RequestLog 写入事务按集中 10,000 行预算裁剪，稳定批次最多 64 条因此持续高于旧 17 req/s 仍不突破上限；历史积压或设置下调通过 `has_more` 与 Writer 内部 `Notify` 继续有界事务，配置发布立即唤醒且不依赖公开请求。Storage 容量 3 项、Runtime 全量 229 项及 fmt/clippy/architecture/diff 门禁通过 | storage P2-11 |
| REV-STORAGE-007 | 已完成 | Storage/Migration | 删除与主键重复的 Attempt 索引 | SQLite 元数据确认显式索引与复合主键自动索引均为 `(request_id, attempt_no)`；冻结 `0001`，追加 `0008_drop_duplicate_request_attempt_index.sql` 只删除冗余 B-tree。带父子数据的 0007→0008 升级回归固定 Attempt 保留、主键唯一性与自动索引查询计划，空库完整链固定最终索引缺失；Storage 81 项及 fmt/clippy/checksum/architecture/diff 门禁通过 | storage P2-7 |
| REV-STORAGE-008 | 已完成 | Storage | 修正 revision 更新 0 行的错误语义 | 条件递增返回 0 行后从同一事务视图读取 actual：过期或超出 SQLite 表示范围的 expected 返回含 actual 的 `RevisionConflict`，只有 actual/expected 同为 `i64::MAX` 才返回 `RevisionOverflow`；可递增但被触发器忽略的异常写入返回 revision `ConfigurationWriteMismatch`。四条路径均固定数据库 revision 不变；Storage 85、Runtime 配置发布 26 项测试及 fmt/clippy/architecture/diff 门禁通过 | storage IMP-13 |
| REV-STORAGE-009 | 已完成 | Domain/Storage/Server/Migration | 统一系统日志 loopback 判定语义 | Domain 提供唯一 `to_canonical`/loopback 语义，Server 可信代理解析、保留策略与 Storage 写入共同复用；前向 `0009` 规范化旧 `::ffff:127.*` 行，外部 mapped 地址保持不变。COUNT 与分页引用同一 SQL 保留谓词常量并继续走摘要覆盖索引，管理直连权限边界不变。Domain 48、Storage 86、Server 59、系统日志契约 3 项及 fmt/clippy/checksum/architecture/diff 门禁通过 | storage P2-10 |
| REV-WEB-003 | 已完成 | Web | 捕获模型保存的 `mutateAsync` rejection | 409 回归先复现 Vitest unhandled rejection，修复后网络/版本冲突只保留抽屉内联错误与本地选择，不关闭编辑器、不发送成功通知；Web 254 项测试、typecheck、lint、production/embedded build、embedded/architecture/diff 门禁通过 | web P2-5 |
| REV-WEB-004 | 已完成 | Web | body scroll lock 使用共享引用计数 | 新增共享 `useBodyScrollLock`，移动导航、SideDrawer、ConfirmDialog 与更新全屏遮罩共用首持有者快照/末持有者恢复的计数器，并保留原始 overflow/padding；Drawer/Dialog、移动导航/Dialog 交错关闭回归固定不提前解锁、不恢复 `hidden`。Web 77 个测试文件 256 项测试、typecheck、lint、production/embedded build、embedded/architecture/diff 门禁通过 | web P2-8 |
| REV-WEB-005 | 已完成 | Web | 限制大 JSON 的语法高亮 | 格式化文本超过 256 Ki 字符或超过 4,096 token 时退化为单一纯文本节点，仍保留完整格式化内容与原文切换；短且 token 密集的输入也受限。64/256/1024 KiB 密集样本由 15,416/61,680/246,720 个 span、约 177/516/2,227 ms 降为 1 个元素、约 16.6/0.5/0.5 ms。Web 77 个测试文件 258 项测试、typecheck、lint、production/embedded build、embedded/architecture/diff 门禁通过 | web P2-9 |
| REV-WEB-006 | 已完成 | Web | Credential 探测状态绑定相关资源版本 | scope 仅绑定当前 Endpoint、Credential 与按 DIRECT 继承规则解析的实际代理版本，不再随全局 revision 或无关代理变化；scope 切换和每次请求共用单调 sequence，迟到请求不能结算结果、错误或 loading。回归覆盖无关 revision 刷新期间在途目录保留、相关资源换版失效及旧请求后完成不覆盖新结果；Web 78 个测试文件 261 项测试、typecheck、lint、production/embedded build、embedded/architecture/diff 门禁通过 | web P2-10 |

## OPT：优化与维护

| ID | 状态 | 模块 | 优化项 | 完成标准 | 来源 |
|---|---|---|---|---|---|
| REV-OPT-001 | 已完成 | Protocol/Runtime | 减少每 Attempt 的完整请求 JSON 深拷贝 | `PlannedRequest` 以 `Arc<DecodedRequest>` 跨 Attempt/OAuth replan 共享不可变入口载荷，`DecodedRequest` 不再实现 Clone；Adapter、Exchange、Bridge 与 multipart 编码改为借用，JSON 通过借用序列化视图替换 model/裁剪 stream，Bridge 移除 messages 与 continuation 的两次完整 Vec clone。重试契约固定两次 wire body 逐字节相同。20 万不同字符串节点、14,000,045-byte 出站 JSON 的 5 次独立进程中位数：峰值 RSS 74,121,216 → 51,527,680 bytes（-30.5%），编码 317 → 312 ms。Protocol 72、Runtime 229、完整契约 154 项及 fmt/clippy/architecture/diff 门禁通过 | rt-sched2 P1-3、protocol IMP-14；ADR-0101 |
| REV-OPT-002 | 已完成 | Protocol | 合成 SSE 事件只序列化一次 | reasoning/文本/工具/错误/终止分支只生成结构化 `SynthesizedEvent`，统一连续注入 `sequence_number` 后才唯一编码为 `AdapterEvent`；回归以编码计数固定 5 个事件恰好 5 次序列化，并逐字节锁定 delta SSE。Protocol 73、完整契约 154 项及 fmt/clippy/architecture/diff 门禁通过 | protocol IMP-13 |
| REV-OPT-003 | 已完成 | Protocol | 用结构化 `Done` 变体识别 `[DONE]` | `SseEventPayload::Done` 已与空帧/心跳分离，桥接路径不再扫描原始字节；SSE 切分属性测试保持通过 | protocol IMP-15 |
| REV-OPT-004 | 已完成 | Runtime stream | 复用 post-commit idle `Sleep` | 首个下游帧交付时仅分配一个 pinned `Sleep`，每个成功上游 chunk 以 `Sleep::reset` 更新绝对 deadline；回归固定两次独立 chunk 间 timer 地址不变，并保留启动时机、成功读取重置、缓冲帧不重置和超时单次结算语义。Runtime 230、虚拟时间 8、public SSE 17 与可靠性契约 1 项及 fmt/clippy/architecture/diff 门禁通过 | rt-sched2 IMP-2 |
| REV-OPT-005 | 已完成 | Runtime routing | 移除重复的 Credential binding Vec | `RoutingCredentials` 仅保留有序投影与 ID 索引，删除会为每个 Binding 额外克隆两个 `Arc` 的第二向量；`PublishedSnapshot::credential_runtimes()` 改为直接借用投影的 `ExactSizeIterator`。回归逐项确认 Runtime ID 与 Credential 投影顺序一致。Runtime 230、完整契约 154 项及 fmt/clippy/architecture/diff 门禁通过 | rt-sched2 IMP-4 |
| REV-OPT-006 | 已完成 | Runtime telemetry | 将 `RequestTelemetry::policy()` 改为纯读 | 请求策略仍从其已捕获的 PublishedSnapshot revision/settings 纯计算，不取得共享写锁或推进 Worker 策略；共享策略更新收窄为私有实现，仅由 `LoggingSettingsReconciler` 在成功发布后调用。故障回归先证明旧 getter 会越过 reconcile 把共享 revision 1 推进到 2，修复后只有显式 reconcile 可改变它。Runtime 231、遥测 16、完整契约 154 项及 fmt/clippy/architecture/diff 门禁通过 | rt-config IMP |
| REV-OPT-007 | 已完成 | Runtime | 清理不可达选择变体和重复 retry 检查 | 删除仅在非空 eligible 集合内调用的 `IndexedSelectAndReserveResult::NoCandidates` 及 Generation 恒假分支，但保留公开 scheduler 的空候选 `NoCandidates` 契约和空 tier 行为；`register_attempt` 删除紧跟 `can_register_attempt` 之后不可达的重复上限检查，边界回归固定 1 次同凭据重试恰好允许 2 次 Attempt。流 Body 取消令牌未修改。Runtime 233、可靠性 38、并发 1、balancing 1 项契约及 fmt/clippy/architecture/diff 门禁通过 | rt-sched2 P2-2、P2-3 |
| REV-OPT-008 | 已完成 | Provider/Domain | 合并三份 retry-safety 表 | `UpstreamErrorKind::default_retry_safety()` 成为唯一 kind 默认表，Provider 共享相容细化后的状态证据合并函数，Claude/Grok 固定拒绝也复用领域默认；408/425 仍以 HTTP 未执行证据覆盖为 `RejectedBeforeExecution`，Codex/Claude/Grok 的 500/503/599 均保持 `Transient + Ambiguous` 且禁止自动重试，公开 503 仅发送一次。Domain 49、Provider 81、完整契约 154 项及 fmt/clippy/architecture/diff 门禁通过 | provider IMP-14 |
| REV-OPT-009 | 已完成 | Provider | 收敛重复公开重导出 | `provider::api` 成为 Registry、trait、错误、Secret、OAuth 类型与辅助函数的唯一公开路径，Provider 根只公开 Composition Root 静态注册所需的 `ClaudeDriver`、`CodexDriver`、`GrokDriver`；App、Runtime 测试与契约夹具全部迁移，架构门禁精确锁定三项根导出并拒绝恢复平行路径。Provider 81、Runtime 233、完整契约 154、xtask 回归 3 项及 fmt/clippy/architecture/diff 门禁通过 | provider IMP-15 |
| REV-OPT-010 | 已完成 | Server logging | 先判定保留规则再构造完整日志对象 | 完成结算先只以 path/client IP/status/outcome 四个标量判定保留；被过滤的本地正常请求不构造完整日志、不复制 Header/Body，需保留时以 `mem::take` 移交字符串、Header 和有界 Body buffer。Vec 指针回归固定 Body 交付不复制；Server 59、完整契约 154、系统日志契约 3 项及 fmt/clippy/architecture/diff 门禁通过 | server P2-1 |
| REV-OPT-011 | 已完成 | Server | 删除不生效的 `DefaultBodyLimit` 层 | 公共 `PublicBody` 显式按操作取得 Runtime 单一 `request_body_limit` 并交给逐块 `collect_body`，成为唯一大小执行点；删除不会被自定义提取器读取的两层 `DefaultBodyLimit` 及无意义的 Router 拆分，管理端 Multipart 限制保持独立。普通 32 MiB、Images Edit 64 MiB、Images Generation 32 MiB 与并发契约 8 项不变；Server 59、完整契约 154 项及 fmt/clippy/architecture/diff 门禁通过 | server P2-5 |
| REV-OPT-012 | 已完成 | Server | 合并重复管理响应/JSON 解析模板 | `/api/admin` 最外层中间件成为成功、提取失败与 fallback 唯一的 `no-store`/`Vary: Cookie` 注入点，删除 Handler JSON helper、认证响应和 `AdminApiError` 的重复 Header；全部管理 JSON Body 改用窄 `AdminJson<T>`，单 revision 与 revision+config-version query 分别使用集中强类型提取器，稳定错误只保留一份。各 feature 继续拥有 DTO、领域转换、Publisher 调用与具体响应，没有通用业务 Handler；Gateway 无逻辑响应转发层一并删除。Server 61、完整契约 154 项及 fmt/clippy/architecture/diff 门禁通过 | server IMP |
| REV-OPT-013 | 已完成 | Server | 客户端地址每请求只解析一次 | 最外层系统日志入口从唯一 PublishedSnapshot 构造不可变 `ClientAddressContext`，同一扩展同时持有快照、规范 TCP peer 与完整成功/失败解析结果；HttpAccessLog 成功时使用逻辑客户端、失败时仅用 peer 审计，Gateway Key 鉴权、管理 Session 鉴权及登录/Setup 复用同一个结果并 Fail-Closed，不再加载新 revision 或重读转发头。底层解析函数收窄为模块私有，生产代码只有 Context 构造点可调用。Server 61、完整契约 154 项及管理可信代理 2、公开可信链 1、系统日志 3 项定向契约与 fmt/clippy/architecture/diff 门禁通过 | server IMP |
| REV-OPT-014 | 已完成 | Server/Web assets | 内嵌资源使用索引并支持条件请求 | 构建期排序清单改为二分路径查找，每项从最终嵌入字节生成 SHA-256 强 ETag；GET/HEAD 支持强、弱、列表与通配 `If-None-Match` 并返回保留 ETag/原 Cache-Control 的无 Body 304，deep link 复用 index 验证器，asset 404/写入 405 不变。Server 63、完整契约 154、内嵌产物 35 文件及 fmt/clippy/architecture/diff 门禁通过 | server IMP |
| REV-OPT-015 | 已完成 | Server | 统一 API 安全响应头 | 合并 Router 的单一全局响应中间件为 Web、`/api/**`、`/v1/**` 的成功、错误、认证拒绝、304、404/405 与生命周期 503 统一覆盖唯一 `X-Content-Type-Options: nosniff`；Web 层只额外负责 CSP/Referrer-Policy，未增加 HTTPS 强制或 HSTS，访问日志仍捕获最终 Header。Server 64、完整契约 154 项及 fmt/clippy/architecture/diff 门禁通过 | server IMP |
| REV-OPT-016 | 已完成 | Storage | 优化凭据/Gateway 用量汇总 SQL | 累计语义仍覆盖完整 RequestLog 保留窗口且未新增计数器；0010 前向迁移为三份既有凭据时间索引追加 `status_code` 覆盖列，Provider/OAuth 改为分支内聚合后 `UNION ALL`，消除双主表扫描和外层临时 B 树，Gateway 聚合不再逐行回表。生产 SQL 直接由升级/EQP 回归复用并验证代表性数据、外键和覆盖计划；20 万行/8 ID 本地基准上游约 90–100ms→10ms、Gateway 约 80ms→10ms。Storage 87、完整契约 154 项及 fmt/clippy/architecture/diff 门禁通过 | storage P2-6 |
| REV-OPT-017 | 已完成 | Storage/Runtime | 减少每次配置 mutation 的重复全量加载和摘要校验 | ADR-0102 固定“事务起点完整加载一次 + 写后按 mutation 影响面回读”：Proxy/Endpoint 会用新引用目标重建相关领域配置，Credential/OAuth/Gateway 等被修改的 Secret 聚合仍从 SQLite 回读并重验摘要，未修改聚合只复用同一事务已验证值；Storage 仍返回完整候选，Runtime 仍执行整份能力校验与快照预编译。10,000 Gateway Key 的 Setting 发布准备 release 基准由双全量 5.498s 降至 2.748s（2.00×）。Storage 90 项 + 手动基准 1 项、Runtime 233、完整契约 154 项及 Workspace fmt/clippy/architecture/diff 门禁通过 | storage IMP-15；ADR-0102 |
| REV-OPT-018 | 已完成 | Storage API | 收窄 `PreparedConfiguration` 持锁事务能力 | ADR-0103 删除公开 `PreparedConfiguration`/`ConfigurationCommit`，活 `sqlx::Transaction` 的 BEGIN、写入、Commit/Rollback 与 Drop 全部留在 Storage；对象安全的泛型事务端口只接收同步 `FnOnce(StoredConfiguration)` 编译器，no-op 不调用、拒绝先回滚、接受先提交，Commit 失败丢弃已编译值，Runtime 只能取得已提交 `PreparedPublishedSnapshot`，无法在持锁期间插入网络 `await`。架构门禁锁定 ConfigPublisher 唯一生产调用点并拒绝旧/具体事务 API 导出；Storage 91 项 + 手动基准 1 项、Runtime 233、xtask 11、完整契约 154 项及 Workspace fmt/clippy/architecture/diff 门禁通过 | storage IMP-16；ADR-0103 |
| REV-OPT-019 | 已完成 | Web charts | 数据刷新使用 `chart.update()` | 共享 `OverviewChart` 将数据与主题生命周期解耦：60 秒查询产生新配置时复用当前 Chart.js 实例，替换 data/options 后调用 `chart.update("none")`，不销毁实例、不重放 550ms 入场动画；主题 Token 变化才按最新配置销毁重建，卸载断开观察并单次清理。新增实例复用、无动画更新、主题重建和卸载回归测试；前端 79 文件 262 项测试、typecheck、lint、生产构建及 35 项内嵌资源同步校验通过 | web P2-6；ADR-0055 修订 |
| REV-OPT-020 | 已完成 | Web overlays | Drawer/Dialog 状态不保存未使用 children | SideDrawer 的退出快照只保留标题、纯文本说明和宽度，编辑器 children 关闭即卸载；ConfirmDialog 只缓存标题、按钮文案、tone 与纯文本说明，结构化 ReactNode 说明关闭即卸载。打开期间父组件即使重建 children/description 节点也不再排入 rAF 或触发快照 state 更新；新增节点更新、退出保留/卸载与 rAF 回归。前端 80 文件 265 项测试、typecheck、lint、生产构建及 35 项内嵌资源同步校验通过 | web P2-7 |
| REV-OPT-021 | 已完成 | Web bundle | Overview 图表按需加载 | Overview 路由先渲染服务/查询状态与统计指标，有有效数据后才通过第二级 `lazy()` 请求双图实现；Chart.js 独立子分块不进入 HTML modulepreload，Suspense 提供两块等高、可访问的布局骨架。相同生产构建下 Overview 路由由 205.05KB / 71.06KB gzip 降至 18.02KB / 5.74KB，图表子分块为 187.74KB / 65.33KB；前端 80 文件 266 项测试、typecheck、lint、生产构建与 40 项内嵌资源同步校验通过 | web IMP-11；ADR-0055 修订 |
| REV-OPT-022 | 已完成 | Web virtual grid | 首次宽度测量使用 layout effect | `VirtualGrid` 在浏览器用 layout effect 同步读取 viewport 宽度，使首个 paint 前从默认 1 列提交为实际 1–3 列；无 `window` 时选择普通 effect。失败回归先固定旧实现首帧为 1 列，修复后 900px 首次同步提交为 3 列；独立 Node renderToString 测试确认无 DOM 访问和 layout-effect 警告。前端 81 文件 268 项测试、typecheck、lint、生产构建与 40 项内嵌资源同步校验通过 | web IMP-12；ADR-0036 修订 |
| REV-OPT-023 | 已完成 | Web | 收敛 Provider/OAuth 重复外壳和请求生命周期模板 | ADR-0104 采用有界抽取：共享 Hook 只统一 Provider Endpoint/Credential、Proxy、Gateway Key、OAuthAccount、Settings 的 revision 单调缓存发布、可选查询失效与失败 active refetch，具体 API、mutation、通知和 OAuth 删除额度清理仍归 feature；删除 5 份重复 cache helper 及 4 份重复测试。Provider 用单一真实 Chrome 在加载/错误/列表内容间切换，非数据态不再渲染 disabled/readOnly 假搜索框；OAuth 的三份 `KindSplitLayout` 合为一份。回归固定加载转正常时 Provider/OAuth 导航 DOM 不替换；未引入参数化万能 mutation 工厂或 Query Boundary。前端 78 文件 269 项测试、typecheck、lint、生产构建与 40 项内嵌资源同步校验通过 | web IMP-13、IMP-14；ADR-0104 |
| REV-OPT-024 | 已完成 | Updater | 将解包/替换提交段移入不可取消 blocking closure | 下载与 checksum 校验后构造拥有临时目录、归档/候选/当前路径和目标版本的 `PreparedInstall`，通过 `UpdateTaskExecutor` 的窄 `spawn_blocking_commit` 端口同步登记到进程 TaskTracker；解包、权限与文件同步、候选冒烟、既有 previous/rename 提交以及成功/失败终态全部在 closure 内完成，成功后同处请求重启。单线程 Tokio 回归证明 blocking 提交不占 worker，Forced 收敛外层 future 后 Tracker 仍保持提交任务直到释放；原有 recovery 测试继续固定 pending/fsync/hard-link/rename/fsync 顺序。Updater 30 项、App 自更新 4 项、管理契约 3 项测试及 fmt、Updater/App clippy、架构门禁通过 | app-misc IMP-15；ADR-0026、ADR-0065、ADR-0089 修订 |
| REV-OPT-025 | 已完成 | Contract tests | 提取统一应用装配 fixture | 新增测试专用 `TestApplication`，集中临时 SQLite、Runtime、Snapshot、Publisher、Composition Root 组件、管理员会话与最小 Web 根，并迁移 19 份 HTTP 契约；配置发布顺序、Telemetry 生命周期、OAuth/Transport mock、自定义管理员认证及静态资源双入口仍由具体测试显式拥有，未引入参数化万能工厂。夹具单测 1 项、完整契约 154 项及 fmt/clippy/architecture/diff 门禁通过 | app-misc IMP-13 |
| REV-OPT-026 | 已完成 | Settings Web | 展示停机设置的最坏总时长 | “优雅停机”随当前有效值或未保存草稿动态显示 `request_grace_period + 6 × finalize_timeout` 的累计等待预算，明确 6 段收尾分别计时且正常停机通常更快；默认值显示 1 分钟，允许上限 `300 + 6 × 60` 显示 11 分钟，非法草稿不展示误导数值。前端 79 文件 271 项测试、typecheck、lint、生产构建、40 项内嵌资源同步校验及 architecture/diff 门禁通过 | app-misc IMP-14 |

## 待验证：先取得证据再决定实现

| ID | 状态 | 模块 | 验证项 | 通过条件/后续动作 | 来源 |
|---|---|---|---|---|---|
| REV-VERIFY-001 | 待验证 | Runtime affinity | continuation 事件热路径的 O(n) 过期扫描 | 用不同 binding/事件规模基准；若达到可观测阈值，移除事件级扫描并复用 sweeper/容量清理 | rt-sched2 P1-2 |
| REV-VERIFY-002 | 待验证 | Runtime scheduler | 单一 epoch 的惊群成本 | 在最大候选和等待者样本下测 O(N×C)；只在证据成立时设计仍保持统一 epoch 的合并优化 | rt-sched2 P1-6 |
| REV-VERIFY-003 | 待验证 | Runtime health | 持健康锁广播 watch 的临界区成本 | 用 contention 基准确认；若成立，将通知安排移到释放内层锁之后 | rt-sched2 P1-5 |
| REV-VERIFY-004 | 待验证 | Runtime OAuth | 定时刷新批次 gate 被最慢账号拖延的在线影响 | 故障注入慢账号，比较整批 gate、分段 gate 与逐账号发布的延迟、一致性和并发风险；若替代方案更优，转入架构修订并实现 | rt-config P2 |
| REV-VERIFY-005 | 待验证 | Runtime/Storage | SQLite commit 已落盘但返回错误的可达性 | 用当前 sqlx/SQLite 故障注入证明；若可达，优先设计 fail-fast，不能带旧快照继续服务 | rt-config P2 |
| REV-VERIFY-006 | 待验证 | Runtime memory | `GatewayUsageTracker` 删除 Key 后的 map 增长 | 长期创建/删除基准；若可观测增长，接入配置 reconcile 淘汰 | rt-config IMP |
| REV-VERIFY-007 | 已完成 | Web | JSON 高亮阈值与 DOM 节点成本 | 使用生产 token 正则与 React/JSDOM 元素构造实测 64/256/1024 KiB 密集 JSON，确认节点数与阻塞线性增长；据此固定 256 Ki 字符快速阈值和 4,096 token 独立预算，并完成 REV-WEB-005 | web P2-9 |
| REV-VERIFY-008 | 已完成 | Server/Deployment | 双栈 `[::]` 下 IPv4-mapped peer 行为 | 确认 `::ffff:*` 不满足原生 IPv4 CIDR/loopback 判断；Server 与 HTTP 契约现以合成 `ConnectInfo` 固定 mapped loopback、mapped 可信代理和 mapped XFF 的规范化行为，作为 ADR-0088/REV-ARCH-003 输入 | server P1-4；ADR-0088 |
| REV-VERIFY-009 | 待验证 | Provider/OAuth | 复核 Grok Free Token、subscription 空值和 Codex 403 出口探测行为 | 结合真实响应 fixture、官方协议和可控集成测试逐项判断；若当前探测造成误判、额外封禁或不可接受延迟，则修订 Provider 决策并实现替代方案 | provider/rt-config 审查 |
| REV-VERIFY-010 | 待验证 | Runtime affinity | 复核 Pending continuation 的 Lease Drop 回收与超时需求 | 用取消、排队、断连和异常 Drop 故障注入确认是否存在泄漏或长期占用；证据成立后设计有界超时，不能靠推测直接修改 | rt-sched2 审查 |

## 需 ADR：架构复核后实施

本节不是“因与旧 ADR 冲突而搁置”。每项都必须独立验证；收益与风险判断成立后，直接改写不合理的旧决策，再实施对应代码。

| ID | 优先级 | 状态 | 决策主题 | 必须回答的问题 | 来源/文档 |
|---|---|---|---|---|---|
| REV-ARCH-001 | P1 | 已完成 | HTTP 400 中明确额度耗尽的分类 | ADR-0086 仅允许已声明 envelope 的精确额度 code/type 细化 400，补齐 OpenAI 当前官方四个 code 并保留兼容值；Claude 普通余额 message 不猜测；实际 Registry 与双 Credential 切换/冷却契约已覆盖 | provider P1-1；ADR-0013、ADR-0070、ADR-0086 |
| REV-ARCH-002 | P1 | 已完成 | OAuth 同一账号重新登录 | ADR-0087：交互式登录按 Provider + 稳定 account ID 匹配，无 ID 时才按双方规范化邮箱回落；唯一匹配原位更新并保留本地 ID/label/RPM/enabled，模型只取旧选择与新目录交集，Token CAS 与配置版本持久化；无匹配生成唯一 label，新旧稳定 ID 不因同邮箱互相覆盖，多匹配返回 409；Domain/Storage/Runtime/Web 单测及三类 HTTP 契约已覆盖 | rt-config P1-2；ADR-0033、ADR-0078、ADR-0087 |
| REV-ARCH-003 | P1 | 已完成 | trusted proxy 下 loopback 安全边界 | ADR-0088：TCP peer 与每个 XFF IP 在 Server 入口统一 `to_canonical()`；仅“规范化 peer 为 loopback 且未进入 trusted-proxy 解析”具有直接本机权限，Setup、`remote_enabled=false`、会话 DTO 与明文提示统一使用该语义；逻辑 IP 继续用于日志/登录限流。Server 单测与管理/公开 HTTP 契约覆盖 mapped IPv4、同机可信反代及伪造 loopback XFF | server P1-2/P1-4；ADR-0014、ADR-0050、ADR-0072、ADR-0088 |
| REV-ARCH-004 | P1 | 已完成 | 自更新旧二进制保留与回滚 | ADR-0089：带格式 pending + 同目录硬链接 previous 在原子替换前持久化；同名冲突 Fail-Closed；`exec` 立即失败、目标版本不符及 listener 确认前启动失败均原子恢复旧程序；listener、必要 Worker 与停机 handler 就绪后才清理，随后由外部 supervisor 负责；明确不逆向回滚 SQLite Migration。Updater 23 项及 App 5 项测试、更新管理契约与工程门禁通过 | app-misc P1-1；ADR-0065、ADR-0089 |
| REV-ARCH-005 | P1 | 已完成 | 更新后的日志收尾失败语义 | ADR-0090：可克隆 `FileLoggingControl` 与 Composition Root 独占 `WorkerGuard` 分离，移除不能代表 flush 成败的 `Arc::try_unwrap` fatal；成功/关键失败两条路径均在最终事件后执行有界 best-effort flush，日志丢弃/I/O/flush 或控制句柄存活不阻断正常退出和 restart；活动请求、受管任务、RequestTelemetry、SQLite 失败仍 fatal，HTTP server error 仍阻止 `result.is_ok()`。日志所有权、outcome、关键失败与真实进程 SIGTERM 回归及工程门禁已通过 | app-misc P1-2；ADR-0021、ADR-0026、ADR-0065、ADR-0090 |
| REV-ARCH-006 | P1 | 已完成 | 更新期间服务长期不可达的 Web 恢复路径 | ADR-0091：精确目标健康是唯一成功；服务端 `failed`/连续三次 `idle` 才是明确失败；活动状态重置不可达窗口，连续 90 秒无活动状态且无目标健康则进入不宣称失败的 `unconfirmed`。该状态清除 tab pending/beforeunload，提供“继续等待/返回”；继续等待只恢复轮询、不重复安装。虚拟时间回归覆盖持续失联、活动重置、idle 失败、Session 恢复和页面解锁；252 项 Web 测试、typecheck、lint、production/embedded build 与架构门禁通过 | web P1-2；ADR-0065、ADR-0091 |
| REV-ARCH-007 | P1 | 已完成 | HttpAccessLog 独立容量预算和 SQLite 回收 | ADR-0092：RequestLog 与 HttpAccessLog 容量解耦，新增独立行数/原始交换字节设置；Migration 0007 回填 `exchange_bytes` 并增加窄覆盖索引；批次事务原子删除最少完整旧记录，热更新/周期清理同步生效；新库启用 incremental auto-vacuum 并按周期/清空后有界回收，旧 NONE 库不做高风险在线全量 VACUUM。Storage 75、Domain 46、Runtime 214、设置契约 8、管理认证契约 2、Web 253 项测试及 fmt/clippy/typecheck/lint/build/embedded/architecture 门禁通过 | storage P1-4；ADR-0051、ADR-0081、ADR-0092 |
| REV-ARCH-008 | P2 | 需 ADR | 日志稳定分页协议 | 是否从 page/total 改为 keyset/cursor；持续写入下如何避免重复漏行并保留用户导航能力 | storage IMP-14；ADR-0051 |
| REV-ARCH-009 | P2 | 需 ADR | 公共 `/api/health` 最小字段 | 必须保留 `application_version` 供更新确认；revision、epoch、请求/任务数哪些移入管理员端点 | server P2-3；ADR-0065 |
| REV-ARCH-010 | P2 | 需 ADR | 未认证 `/v1` 日志冲刷防护 | 在继续审计所有公开请求的前提下，是否增加源 IP 限流、采样或独立容量，避免挤掉真实历史 | server P2-2；ADR-0051 |
| REV-ARCH-011 | P2 | 需 ADR | Session `Creating` 租约与长排队 | 能否把 RPM 等待移出租约而不产生双创建；等待错误应如何准确归因 | rt-sched2 P1-4；ADR-0062 |
| REV-ARCH-012 | P1 | 已完成 | 首个 SSE 事件前失败与 `Transient` 自动重试语义 | ADR-0093：Pending 只是必要条件，不是未执行证据；HTTP 408/425 按标准语义使用 `Transient + RejectedBeforeExecution` 并可在既有预算内切换，5xx、成功响应头后的 buffered body/首个 SSE 事件失败继续保持 `Ambiguous`，421 在 Transport 无法保证新连接前不放宽；不增加 at-least-once 开关。Provider 73、Runtime 214、完整契约 149 项测试及 fmt/clippy/architecture/diff 门禁通过 | protocol/runtime 审查；ADR-0013、ADR-0016、ADR-0093 |
| REV-ARCH-013 | P1 | 已完成 | health 竞争失败后的 RPM 预留处理 | ADR-0094：保留“健康预检查 → 原子 RPM 预留 → Health Guard 获取”的顺序，避免先占 Half-Open 探针再遇 RPM 满额造成自唤醒空转；有限窗口改存唯一预留令牌，仅在 Guard 竞争失败且 Attempt/上游 I/O 尚未开始时于同一 Mutex 下精确删除自己的记录、释放 `in_flight` 并推进 epoch。固定与普通选择共用回滚边界；形成 `SelectedCandidate` 后任何失败仍不归还。Runtime 216、完整契约 149 项及 fmt/clippy/architecture/diff 门禁通过 | rt-sched2 审查；ADR-0013、ADR-0037、ADR-0094 |
| REV-ARCH-014 | P1 | 已完成 | OAuth Token 刷新后的健康状态代际 | ADR-0095：`CredentialGenerationRuntime` 将 `authentication_version` 的 `auth_error` 与 `routing_generation` 的额度/权限/模型冷却拆开；OAuth refresh/同身份重新授权只增加 `token_version`，新建认证健康并复用账号路由健康，重新启用或 API Key/Endpoint 身份变化才整体换代。退役 Token 的迟到 401 只能写旧认证对象；同账号额度信号仍共享。Domain 5、Storage 10、Runtime 218、完整契约 149 项及 fmt/clippy/architecture/diff 门禁通过 | rt-config 审查；ADR-0013、ADR-0033、ADR-0070、ADR-0087、ADR-0095 |
| REV-ARCH-015 | P1 | 已完成 | trusted proxy 转发头缺失、重复和非法值策略 | ADR-0096：可信对端缺少 XFF 时保守使用规范化 TCP 对端，缺少 XFP 时按非安全 HTTP；连接仍标记为经可信代理，不能获得直接 loopback 权限。多行 XFF 按收到顺序合成完整逻辑链再从右向左剥离，未采纳“只取最后一行”；空值/非法 XFF 与重复/非法 XFP 继续 400。管理面与数据面复用同一解析器；Server 55、管理契约 2、公开可信链契约及完整契约回归通过，fmt/clippy/architecture/diff 门禁通过 | server 审查；ADR-0014、ADR-0050、ADR-0072、ADR-0088、ADR-0096 |

## 文档一致性

| ID | 状态 | 任务 | 完成标准 |
|---|---|---|---|
| REV-DOC-001 | 待办 | 对齐原始 HttpAccessLog 的明确例外 | `AGENTS.md` 的普通日志 Secret 禁止规则明确引用 ADR-0081 原始交换例外，避免与 `ARCHITECTURE.md` 相互矛盾；不改变产品行为 |
| REV-DOC-002 | 待办 | 统一系统日志 query 描述 | 修正 `ARCHITECTURE.md` 前部“path 不保存 query”与后部“完整 URI 含 query”的措辞，明确 path/uri 是两个字段 |
| REV-DOC-003 | 待办 | 统一 Release 版本真相来源 | 对齐 README、架构、ADR-0065 与实际 workflow：明确 Cargo package version 是否必须等于 Actions 输入，并补相应 CI 校验或删除错误描述 |

## 已解决、用户明确排除或经独立证据不纳入

| 项目 | 状态 | 结论 |
|---|---|---|
| 非流式请求默认 20 秒硬截断 | 已完成 | `e2dd42f` 与 ADR-0084 已把普通 read/precommit/总预算调整为 300/300/600 秒，并有默认值测试 |
| HttpAccessLog 对 Header/Body/Secret 脱敏或加密 | 不采纳 | 用户在本轮再次明确选择原始明文交换；这是当前用户边界，不以 ADR 正确性为依据 |
| Claude 自定义 Endpoint 改用 `x-api-key` | 不采纳 | 报告把当前分支读反：代码对官方 Origin 使用 `x-api-key`、自定义 Endpoint 使用 Bearer；报告没有提供失败 fixture 或上游契约证明应交换两者，出现反例时重新立项 |
| Release 增加独立签名链 | 不采纳 | 本轮明确排除签名；当前信任边界仍是固定 GitHub Release、TLS 与 SHA-256 |
| 删除 source-size Allowlist/tokei 门禁 | 不采纳 | 违反仓库强制的 401–600 行机器可读 Allowlist 规则 |
| App 强制只能通过 Adapter `api` 导入具体实现 | 不采纳 | 审查未指出实际依赖倒置或测试障碍；Composition Root 必须能引用具体实现，强制间接导入只增加无收益转发层 |
| 未知 Setting key 静默忽略 | 不采纳 | 未知持久化 key 表示版本漂移或配置损坏；静默忽略会形成难以发现的部分生效配置，报告未给出足以抵消该风险的场景 |
| SSE 新连接初始 epoch 不触发刷新 | 不采纳 | 首次立即查询用于覆盖断线到重连之间的窗口；移除后存在可构造的丢失更新路径，节省一次查询不足以抵消正确性风险 |
| 跳过残缺工具调用或伪造占位 tool output | 不采纳 | 跨协议桥必须 fail-closed，禁止静默删除或制造不存在的协议历史 |
| ProviderDriver 大拆分、全局 epoch 改为多套定向队列、永久统计计数器 | 不采纳 | 报告没有提供可复现瓶颈或收益数据，且三项都会显著扩大核心抽象；后续若基准证明具体问题，应拆成独立任务重新立项，而不是引用本结论阻止优化 |
