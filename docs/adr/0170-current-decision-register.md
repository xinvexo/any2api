# ADR-0170: 当前架构决策登记册

- 状态：Accepted
- 适用范围：当前架构基线及其维护方式
- 当前事实：[`ARCHITECTURE.md`](../../ARCHITECTURE.md)

## 背景

仓库早期为每个小范围修复和审查意见创建独立 ADR。随着实现收敛，这些文件开始重复当前架构、互相修订，
导致一次行为变更需要同时修改架构正文、多个 ADR、索引和整改台账。已经淘汰的方案也继续占据文档入口，
后来者容易把历史方案误读为当前约束。

## 决策

1. `ARCHITECTURE.md` 是当前行为、边界、不变量、模块职责、协议范围、默认值和部署语义的唯一规范来源。
   代码实现细节以代码为准，但文档中需要维护的架构事实只在这里记录。
2. 本登记册只记录形成当前架构的理由、取舍和少量历史方向；不复制字段表、默认值、接口清单、状态机或
   测试矩阵。需要知道“现在是什么”时只读架构基线，需要知道“为什么这样”时再读本登记册。
3. `docs/adr/README.md` 只做入口、文档所有权说明和当前登记册索引，不维护另一份决策正文。
4. `README.md` 只服务使用者，记录安装、运行、部署和公开入口；`docs/baselines/` 只保存可复核的外部证据。
   `AGENTS.md` 只记录协作、编辑、依赖边界和验证规则，不复制产品架构。
5. 整改台账只保留仍未完成的工作。已经完成的审查过程、基准数字和“不采纳”清单不作为长期架构文档；仍有
   价值的方向收敛到本登记册的“已舍弃方向”。

## 当前取舍的理由摘要

### 产品和数据边界

- 个人、单节点、自托管定位使 SQLite、进程内 Runtime 和单一二进制成为足够且可审计的基础；不引入多租户、
  计费、分布式调度、Redis、PostgreSQL 或消息队列。
- `GatewayApiKey`、`ProviderCredential` 和 `OAuthAccount` 的生命周期与权限不同。它们只在运行时的通用
  `RoutingCredential` 投影处合流，避免复制两套调度、健康和重试实现。
- SQLite 是管理员配置和必要凭据的持久化真相；运行态不恢复。明文 Secret 是明确的本地部署边界，不再
  引入第二套主密钥、加密 Schema 或 Secret 导出系统。

### Provider、协议和网络

- Provider、Protocol 和 Transport 分层，Provider 只描述供应商契约，Protocol 只负责线协议，Transport
  只负责出口和连接；这样新增 Provider 不需要改中央调度器，也避免把网络错误和业务错误混在一起。
- 同方言请求优先保留原始 wire bytes，跨协议才物化桥接结构；这同时保留未知字段并限制大请求的复制成本。
- SSE 的网络 chunk 与协议帧没有一一对应关系，因此按协议增量分帧，并把 Guard、解码状态和未消费字节交给
  响应 Body 持有。首个可接受事件前使用有界预算，可以在提交下游前识别损坏或空流；提交后则保持单一路径，
  避免拼接两条上游流或伪造协议终止。
- 上游认证、请求 Header、Content-Encoding 和重定向由明确的边界拥有者处理。客户端凭据永不直接成为上游
  凭据，专属代理失败不回退，Provider URL 是管理员明确授权的目标。
- Provider identity 和官方客户端观测作为可版本化证据维护，但没有证据时不模拟 TLS/HTTP 特征或随机伪装。

### 调度、会话和流式请求

- RPM 是唯一用户可配置的本地准入限制；`in_flight` 只表达运行态资源生命周期。选择与 RPM 预留原子化，
  QueueTicket 和统一 epoch 负责有限等待，避免隐藏并发限制和丢失唤醒。
- 会话一旦绑定，就固定 Credential、Route Target、模型和协议方言。只有尚未向下游提交且有明确安全证据
  时才允许重试或换路；提交任何响应字节后永久禁止切换。
- SSE 使用预提交预算和一次性 Guard，提交前失败与提交后失败分开处理。未知失败不伪装成成功，也不为了
  计费而继续 Drain 上游。
- 大块短命连续数据不适合与长期小对象共同依赖通用堆的回收时机，但过低的 mmap 阈值会让中型对象承担
  额外映射与迁移成本。现网主路径抽样的 zstd 压缩正文约 0.3–0.6 MiB、解压后 P99 约 1.5 MiB，旧 `256 KiB`
  payload 阈值会为一次普通请求连续建立约 0.5/1/1.5 MiB 映射；HTTP 捕获本身又被限制在 `1 MiB`。
  因此 payload 只在需要的新容量达到 `2 MiB` 后使用匿名映射，已经足够的 mimalloc capacity 不做无收益迁移。
  zstd 另按真实 allocation tier 决策：约 `95,984 B` 交给 mimalloc，streaming `2 MiB` window 对应的
  `2,490,432 B` workspace 继续 direct mmap；两者当前边界虽同为 `2 MiB`，但不共享策略常量。Rust
  通用堆固定使用 mimalloc，不保留系统分配器构建分支，也不启用全局 C allocator override，避免把 SQLite 等
  原生依赖纳入未经验证的替换范围。Linux 现网验证表明，默认透明大页会把 mimalloc 稀疏使用的 arena 以大粒度
  驻留并形成与存活对象无关的稳定 RSS；因此构建统一启用 `no_thp`，以普通页和默认 purge 换取单节点部署更可控
  的常驻内存。当前 mimalloc v3 的该 feature 只移除主动大页建议，不能覆盖宿主机 `always` 策略，因此 Linux
  二进制在 mimalloc 构造器之前同时固定 `allow_thp=false` 并执行 `PR_SET_THP_DISABLE`，Composition Root 再验证
  内核调用；拒绝只在某台机器注入 allocator 环境变量，也拒绝把 supervisor 变成正确性的组成部分。该取舍接受
  极端内存密集负载可能失去部分 TLB 收益，但不把未受控吞吐假设凌驾于已观测的驻留内存问题。Tokio 调度与 Blocking Pool 线程显式声明为
  mimalloc thread-pool thread；页由 mimalloc 的跨线程回收、默认 purge 和线程退出生命周期管理；应用不调用 `mi_collect`，避免把没有全局
  完成语义的线程局部操作包装成进程级回收。拒绝后台强制收集、`force=true`、唤醒空闲线程和按服务器设置
  allocator 参数。总览不展示分配器内部分类；payload 不维护堆/映射/HTTP 捕获当前与峰值原子指标，遥测
  Writer 也只保留字节准入需要的私有总预留，不维护 queued/in-flight/reserved 三项 owned-byte 快照。
  现有空闲期平台压力释放只覆盖原生系统堆；OAuth Token/额度等后台 Worker 与公共请求共用活动 epoch。该策略
  只改善确定性归还，不恢复全局内存准入，也不承诺 RSS 回到冷启动值。停机跟踪与内存回收阻塞使用独立 Guard，
  使长期管理通知流仍参与优雅停机，却不会永久阻塞空闲回收。

### 配置、存储和安全

- 发布采用“事务内构造和校验候选配置，Commit 后 reconcile，再一次性切换 PublishedSnapshot”的顺序，
  保证鉴权、路由和设置使用同一 revision。
- Schema 只追加不可改写的前向 Migration；历史格式转换只在 SQL Migration 或外部导入边界完成，运行时只
  接受当前模型。
- 管理面使用单管理员认证，允许 HTTP 但明确提示风险；公开 Gateway Key 不能登录管理面。日志、DTO、Debug
  和浏览器持久化不得泄露 Provider Secret、OAuth Token、代理密码或原始 Session ID；经认证的 HttpAccessLog
  详情是唯一允许按操作员选择读取原始客户端交换的例外。

### 可观测性、Web 和发布

- RequestLog、HttpAccessLog、活动请求投影和统一管理员 SSE 分别承担历史事实、原始交换、当前进度和失效通知；
  不把实时状态伪装成持久化日志，也不让日志通知承载 Secret 或正文。
- 在购买 Credits 接管窗口前，Codex 本机额度的“已用”选择官方周期内仍保留的 RequestLog 直接求和，而不再由相邻刷新的小百分比反推。
  这样在周期记录完整时，接管前的已用值随本地事实单调增加，且进程重启与漏刷不会产生新的小样本；总量推算保留
  `2%` 门槛，以避免零值和最早期样本直接参与推算；接受较小分母会放大上游百分比量化与异步记账，不引入 EMA、限幅或异常值删除去隐藏
  真实偏差。
- RequestLog 没有 Provider 最终从 included-window 还是购买 Credits 扣款的逐请求证据；官方窗口达到
  `100%` 且真实 Credits 可用后，继续累加整周期本地成本必然把两种资源混在一起。当前选择在首次接管时
  优先冻结同周期接管前的可信容量估算；缺少该基线时冻结首次接管 observation fence 的正数本地周期总和，
  使耗尽窗口继续可见且后续 Credits 不再抬高它。该回退无法追溯剔除 fence 前已发生的 Credits 消耗，但也不
  根据余额差、自然语言或请求时间猜测逐请求扣款来源。旧 estimator 无法恢复接管边界，因此前向 Migration
  保留官方 usage、清空派生状态，并由下一次权威刷新建立冻结值。
- 额度容量是否能够跨刷新比较，应由 Provider 稳定主体而不是路由健康代际决定。账号重新启用、代理切换和
  Token 换代不会把同一个上游主体变成另一份额度；只有 Provider 无法提供稳定主体时，才使用账号与 Token
  代际作保守回退。旧指纹无法在不读取 Secret 的 SQL Migration 中可靠转换，因此升级时只清除可由整周期
  RequestLog 重算的 estimator state，并保留官方额度观测。
- Codex 本机成本先由实际上游模型选择对应费率，再仅以上游最终响应明确确认的 Fast 事实选择 Fast 档；
  请求声明只能证明执行意图，不能证明 Provider 实际按快速档处理。非 Fast 返回和缺失返回均按 Standard，
  避免把未经上游确认的请求意图乘进本机已用额度。
- 管理员实时状态采用单一共享 sampler 和“最新快照”广播，而不是每个页面独立轮询或持久化事件回放；这样
  采样成本不随打开视图增长，断线后仍可由当前快照和 HTTP 事实查询恢复。
- 逻辑请求与上游尝试采用分层统计：Gateway API Key、总览和 Token/请求趋势按一条最终 RequestLog 计数，
  Provider API Key/OAuthAccount 统计按每条 RequestAttempt 归属计数。这样重试后的最终结果不会掩盖中间凭据
  的失败，同时不会把一次客户端请求在总览和网关统计中放大成多条请求。
- Web 面向重复操作和响应式浏览器使用，页面状态、API 调用和 feature 组件按功能归属；架构约束不再复制到
  每个页面或测试说明中。
- Node/pnpm 统筹完整应用的开发、构建和打包，Cargo 保持 Rust-only；正式包是内嵌 Web 的单一二进制，更新器
  只做经校验的固定 Linux AMD64 Release 替换，不替代外部 supervisor。

## 已舍弃方向

以下内容只保留方向性结论，具体旧实现、旧字段和旧测试不再作为文档资产：

| 舍弃方向 | 当前结论 |
|---|---|
| 多租户、余额、计费、支付、Key 销售和分布式调度 | 永久不属于产品边界 |
| 通用 YAML/Secret/数据库导入导出或服务器 OAuth 文件下载 | 只保留 SQLite 当前模型和受支持的 Provider 专用导入边界 |
| 公开请求的全局内存准入、按 TPM/并发/权重的第二套调度限制 | 删除；只保留 RPM、QueueTicket 和明确的单对象容量上限 |
| Responses WebSocket 入口、上游/下游 WebSocket 和跨 Provider 双向桥 | 首版不提供；Responses 走 HTTP JSON/SSE，GET 入口返回 426 |
| 按 Credential 做 prompt-cache 软路由、修改同方言请求面以区分账号 | 删除；请求面保持缓存连续性，粘性只承担固定会话语义 |
| 运行态恢复、请求回放、队列/会话/健康恢复、复杂备份容灾 | 进程重启后从空 Runtime 启动，备份属于部署操作 |
| 旧 OAuth JSON、旧浏览器字段别名、启动期兼容读取和代码内迁移 | 在迁移或导入边界一次性收敛，生产路径只接受当前 Schema |
| 逐条日志事件入口、页码/随机跳页和常驻客户端轮询 | 统一 `/api/admin/events`，用 epoch + Keyset Cursor + 短时合并追赶 |
| 把官方观测当作通用伪装、按自然语言猜额度/账号状态 | 只有可审计字段和固定证据才能影响分类或健康 |

## 维护规则

修改当前行为时只更新 `ARCHITECTURE.md`；若取舍理由或舍弃方向发生变化，再同步本登记册。新增 ADR 只有在
出现新的、独立且尚未能归入本登记册的架构取舍时才必要，不能为每个实现修复或测试补充单独文档。
