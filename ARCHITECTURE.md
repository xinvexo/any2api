# any2api 架构地图

本文只描述系统形状、模块边界和跨主题不变量。协议、调度、存储、运维与 Web 的当前事实分别维护在
[docs/architecture](docs/architecture/README.md)；已经作出的设计取舍及状态维护在
[docs/adr](docs/adr/README.md)。字段、默认值和已注册实现等机器可枚举事实以代码 Registry、Migration 和
生成契约为准，不在文档中维护第二份清单。

## 1. 产品边界

any2api 是个人使用、自托管、单进程、单节点的 AI API 聚合代理。它把管理员持有的 Provider API Key 和
受支持的 OAuth 账号汇入统一路由池，对客户端提供带 Gateway API Key 鉴权的 AI API，并提供单管理员 Web
控制面。

产品不承担以下职责：

- 用户注册、多租户、套餐、余额、计费、支付或 API Key 销售；
- 多节点调度、分布式状态、请求回放和队列恢复；
- Redis、PostgreSQL、消息队列或微服务部署；
- 通用数据库、Secret 或运行态导入导出；
- 用未知 Provider 行为、URL 或模型名猜测兼容能力。

SQLite、进程内 Runtime 和单一应用二进制是有意选择的部署边界，而不是等待替换的临时方案。

## 2. 系统上下文

```text
AI client
   │  Gateway API Key
   ▼
server ──► runtime ──► protocol/provider ──► transport ──► upstream Provider
   │           │                 │
   │           └──── PublishedSnapshot / runtime state
   │
   ├── admin API ◄──── embedded React Web
   │        │
   │        └──── ConfigPublisher ──► storage(SQLite)
   │
   └── metadata telemetry ──────────► storage(SQLite)

app/any2api = process bootstrap + concrete dependency assembly
```

数据面处理公开模型请求；控制面管理代理、Provider、凭据、OAuth、Gateway Key 和设置。两者读取同一已发布
配置 revision，但管理 DTO、浏览器状态和日志不能成为 Secret 的旁路。

## 3. Workspace 模块

| 模块 | 职责 | 不拥有的职责 |
|---|---|---|
| `crates/domain` | 领域类型、验证规则、配置值和稳定标识 | HTTP、SQLite、Provider 实现、网络请求 |
| `crates/payload-buffer` | 大块请求/响应数据的受控连续缓冲 | 协议语义和调度 |
| `crates/protocol` | 公开/上游线协议、桥接、增量 SSE 转换、目标协议 Profile | Provider 认证和网络 I/O |
| `crates/provider` | Provider descriptor、可选能力 facet、认证材料投影、端点和错误契约 | 发起网络请求、中央调度 |
| `crates/transport` | DIRECT/HTTP/SOCKS5、TLS/HTTP、地址解析、超时和内容编码 | Provider 业务语义 |
| `crates/runtime` | 配置发布、候选路由、RPM、健康、粘性、重试、OAuth 编排、请求生命周期 | Axum DTO 和 SQL 实现 |
| `crates/storage` | SQLite Repository、事务、Migration 和持久化行映射 | Runtime 策略和 HTTP |
| `crates/server` | Axum 路由、鉴权、中间件、管理/公开 DTO、Web 资源服务 | 核心路由决策和直接 SQL |
| `crates/memory-reclaimer` | 平台原生堆压力释放的隔离实现 | 业务内存策略 |
| `crates/updater` | 官方 Release 元数据、下载、验证和文件替换原语 | 进程装配和数据库回滚 |
| `app/any2api` | 唯一 Composition Root、启动、日志、停机和更新编排 | 可复用领域逻辑 |
| `tests/contract` | 跨 crate、公开 HTTP、真实 SQLite/loopback I/O 契约 | 生产实现 |
| `xtask` | 少量仓库级语义检查 | 产品运行时 |

模块边界以职责和依赖方向为主，不以文件数量或文件长度定义。可以合并只做转发的薄层，也可以在状态机、
协议阶段或 unsafe 边界确有独立所有权时拆分。

## 4. 依赖方向

主要生产依赖方向如下；`app/any2api` 只在最外层装配具体实现：

```text
domain                 payload-buffer
  ▲                         ▲
  ├──── transport           ├──── protocol
  ├──── storage             ├──── provider
  └──────────────┬──────────┘        │
                 └──── runtime ◄─────┘
                         ▲
                         │
                       server ◄──── updater
                         ▲
                         │
                    app/any2api
```

`provider` 可以依赖 `protocol::api` 中的稳定 Profile 类型来声明目标协议能力，但不能执行 Bridge；Runtime
只通过 `provider::api`、`protocol::api`、`transport::api` 和 `storage::api` 使用实现。新增 Provider 或
Protocol 通过局部实现、Registry 注册和契约测试进入系统，不在中央请求执行器累积按 Provider 分支。

Web、SQLite、Axum、HTTP Client 和具体 Provider 不得反向进入 `domain`。unsafe 平台代码保留在专用底层
crate，其安全封装之外的 Workspace 继续遵守 `unsafe_code = "forbid"`。

## 5. 跨主题不变量

以下不变量跨越多个主题，其余细节由主题文档拥有：

1. **明确能力，不猜测。** Provider、凭据种类、操作、协议方言和目标 Profile 都来自已注册实现与持久化
   配置，不由 URL、模型名或自然语言响应推断。
2. **客户端鉴权与上游认证隔离。** Gateway API Key 只能鉴权公开入口，绝不直接成为上游凭据；Provider
   Secret 只在需要发起上游请求的最窄边界解封。
3. **一次请求只有一个提交边界。** 下游尚未提交时可以依据类型化安全证据重试；提交任何响应字节后不能
   换上游、拼接响应或把流内失败改写成成功。
4. **配置 revision 原子可见。** 配置候选必须在 SQLite 事务中构造并校验；成功返回前，持久化提交、完整
   `PublishedSnapshot` 切换和相关 Runtime reconcile 必须全部完成。读请求只观察完整 revision。
5. **持久化与运行态分离。** SQLite 保存配置、必要凭据和有界历史；RPM 窗口、队列、健康、冷却、粘性、
   请求进度和后台任务状态在进程启动时从空状态建立。
6. **Secret 不进入观察面。** Token、密码、Cookie、原始 Session ID 和带凭据的请求内容不得进入普通日志、
   遥测、错误正文、Debug、管理响应、测试快照或浏览器持久化。
7. **代理和授权目标 fail closed。** 专属代理失败不回退到其他出口；Provider Base URL、可信反向代理和
   转发客户端地址都必须由管理员显式授权并经过验证。
8. **单节点保持简单。** 有界队列、SQLite Writer 和进程内广播解决本节点问题；不为假设中的集群引入恢复
   协议或外部协调服务。

## 6. 主题规范

| 主题 | 当前事实的规范位置 |
|---|---|
| Provider、Operation、协议 Bridge、目标 Profile | [protocol-bridges.md](docs/architecture/protocol-bridges.md) |
| 请求阶段、路由、RPM、粘性、重试、SSE | [routing-and-streaming.md](docs/architecture/routing-and-streaming.md) |
| SQLite、配置发布、Migration、Secret、认证、遥测 | [storage-and-security.md](docs/architecture/storage-and-security.md) |
| 构建、平台支持、部署、停机、自更新、内存组件 | [operations.md](docs/architecture/operations.md) |
| React 所有权、服务端状态、实时事件和交互边界 | [web.md](docs/architecture/web.md) |

公开安装步骤、环境变量、反向代理示例和 API 路径属于 [README.md](README.md)。设置 key、类型、默认值、
约束和 apply mode 的规范来源是 `crates/domain/src/settings/definitions/registry.rs` 及其注册定义；文档不复制
设置表。管理 TypeScript DTO 由 Rust 导出流程生成，生成文件不是独立设计来源。

## 7. 关键数据流

### 7.1 公开请求

1. `server` 建立 request ID、规范客户端地址和生命周期 Guard，并完成 Gateway 鉴权。
2. `runtime` 从一个 `PublishedSnapshot` 解析公开操作、模型、协议和路由要求。
3. 路由层选择并预留一个候选；Provider 提供目标能力与认证契约，Protocol 计划 wire 转换，Transport 发起
   实际网络请求。
4. 在下游提交前，类型化错误可进入有限重试；提交后，选定 Attempt 独占余下响应生命周期。
5. 完成、取消或 Body 错误统一结算 Guard，并异步写入有界遥测。

### 7.2 配置变更

1. 管理 API 把已验证 DTO 转成 `ConfigCommand`，携带期望 revision。
2. `ConfigPublisher` 串行化变更，在 Storage 事务中生成并编译完整候选配置。
3. 事务成功后发布完整快照，更新路由准入并 reconcile 依赖配置的进程组件。
4. scheduler epoch 和管理事件使等待者与 Web 视图观察新 revision；任一步失败都不能返回虚假的成功。

### 7.3 实时管理面

Runtime 只广播“最新状态”和不含正文的失效事件。历史列表仍以 SQLite Keyset Cursor 为事实来源；Web 在事件
后做有界追赶，而不是把 SSE 当作持久化事件流或恢复日志。

## 8. 扩展规则

### 新 Provider

- 在 `provider` 中实现基础 Driver 和实际拥有的可选 facet；descriptor 必须与 facet 一致。
- 在 Composition Root 注册，并由 Registry 契约测试枚举；不要修改中央调度器加入 Provider `match`。
- 只为有证据的上游差异选择或新增 `ProtocolTargetProfile` 字段；不要复制整套 Bridge。

### 新协议或 Bridge

- 在 `protocol` 定义 Operation、请求/响应和流式转换的明确所有权。
- 同方言优先保持 wire 信息；跨方言必须显式声明有损或不支持的能力。
- 为任意字节切分、CRLF、多行 SSE、无尾空行、提交前失败和提交后失败提供相应层级的测试。

### Schema 或配置

- Schema 只追加下一条不可变 Migration；旧格式转换在 Migration 或受支持导入边界完成。
- 设置进入现有 Registry，并由一个定义拥有 key、类型、默认值、约束和 apply mode；不要在文档或 Web 再建
  一份默认值表。
- 任何影响路由或鉴权的管理写入都必须经过 `ConfigPublisher`。

## 9. 验证与治理

测试按故障面分层：纯验证/转换使用单元测试，Registry 使用枚举契约测试，最终 path/header/auth 使用少量
loopback HTTP 契约，SQLite 使用 Migration 和 Repository 测试，跨页面关键流程才进入浏览器 E2E。
相同断言不因模块层级重复堆叠。

`cargo xtask architecture-check` 只保留三类仓库语义检查：Workspace 依赖边界、Migration 编号与 checksum、
官方客户端基线的 schema 与脱敏。代码形状、导出文本和实现数量由编译器、Registry 测试及评审判断。

CI 的普通 PR 验证以 Linux 为必需平台；macOS/Windows 原生检查在主分支和定时任务运行，并允许作为非发布
平台独立报告。完整应用构建只需要在 Linux 组合一次当前 Rust 与 Web 源码。

## 10. 决策记录

ADR 是创建后保持历史语境的决策记录，不是当前规范的镜像。新决策使用
[ADR 模板](docs/adr/0000-template.md)，通过 `Accepted`、`Superseded`、`Deprecated` 或 `Rejected` 表明状态；
被替代的 ADR 保留并链接后继项。当前行为发生变化时更新拥有该事实的主题文档，只有取舍本身变化时才创建或
替代 ADR。
