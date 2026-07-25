# ADR-0047：Rust 工作区采用 Feature-First 目录结构

- 状态：Accepted
- 日期：2026-07-26
- 决策人：项目维护者

## 背景

随着 OAuthAccount、ProviderCredential、GatewayApiKey、代理、配置发布和请求遥测逐步实现，多个 crate 的 `src/` 根目录积累了大量按技术后缀命名的平铺文件。例如同一实体的 `*_rows.rs`、`*_repository.rs`、`*_writes.rs`、`*_dto.rs`、`*_handlers.rs` 和 `*_tests.rs` 相互远离。应用 crate 也把启动环境、具体 Adapter、日志和进程装配平铺在根目录；`xtask` 同时存在 `architecture.rs` 与 `architecture/`。文件虽然仍未超过体积门禁，但定位一个功能需要扫描整个 crate，也使所有权边界和可见性越来越模糊。

项目是模块化单体；整理目录不能借机制造新的横向“公共层”、反转 crate 依赖，或用兼容转发模块长期保留错误路径。

## 决策

1. Rust Workspace 源码采用 feature-first 目录。Domain 按 Gateway Key、OAuth Account、Provider、Proxy、Routing 和 Telemetry 分类；Storage 按持久化聚合根分类；Server Admin 按 HTTP feature 分类；Runtime 按配置发布、路由、凭据维护、OAuth、遥测和生命周期分类；Transport 按 client、connection、proxy 与 resolution 分类。
2. 同一 feature 的 entity/configuration、row/repository/write、DTO/handler/error 与测试放在相邻目录；Transport 测试也跟随所验证的能力放置。`mod.rs` 只声明子模块、做最小重导出或组装 feature 路由，不承载大段业务逻辑。
3. 单文件且职责独立的模块不机械创建同名目录。目录必须表示多个文件共同维护的真实领域边界。
4. crate 之间继续只使用稳定 `api` 出口。内部模块路径一次性迁移到最终结构，不保留旧路径兼容别名、双轨模块或转发文件；本项目尚无需要维持的内部路径兼容负担。
5. 共享抽象必须来自至少两个真实调用方的相同语义。仅仅拥有相似后缀不是抽象依据；禁止引入 `common.rs`、`utils.rs`、万能 repository、万能 handler 或只有一层调用的 facade。
6. 目录迁移不得改变 SQLite Schema/Migration、HTTP 路由、Provider 注册、Secret 边界、调度规则或公开 DTO。行为变更必须另立 ADR 和独立提交边界。
7. `app/any2api` 按 `bootstrap`、`logging` 与 `shutdown` 分类。`bootstrap` 是唯一应用装配域，包含环境配置、实例锁、具体 Registry/Adapter 注册和启动流程；它不能承载被装配 crate 的核心业务规则。`build.rs`、`main.rs` 与 `lib.rs` 保持工具链约定入口并维持最小内容。
8. `xtask` 按命令域分类。`architecture-check` 内的 crate 依赖、Migration 历史和源文件体积检查保持独立；只被体积检查使用的 Allowlist 解析归入 `source_size`，不提升为无实际复用的全局工具层。
9. 已形成多条独立工作流的 feature 继续按工作流收敛：Runtime OAuth 分为 `login`、`import`、`quota` 与 `refresh`，Server OAuth 分为 `account`、`login`、`import` 与 `quota`。请求日志按仓储编排、写入和 Row 映射拆分；Responses-to-Chat 请求转换按输入、选项和工具拆分；Reqwest Transport 按执行、Client 构造和失败分类拆分。OAuth 刷新进一步分离扫描调度与单账号 CAS/singleflight 执行；请求遥测分离请求生命周期与最终记录组装；Provider Credential DTO 分离读取响应与变更请求。
10. 承担明确职责的核心类型保留既有公开名称，但内部文件使用 `config_publisher.rs`、`coordinator.rs`、`executor.rs`、`authentication.rs` 等职责名称，不再使用含义不明的 `service.rs`。测试通过正常目录模块与实现相邻，禁止继续用 `#[path = "...tests.rs"]` 维持不一致的物理布局。
11. 源文件体积 Allowlist 只记录当前仍需偿还的例外。每个条目必须对应 `architecture-check` 实际扫描且处于 401–600 代码行区间的文件；目录整理导致文件删除、移动或降到阈值以下时必须同步移除条目，门禁主动拒绝失效条目。
12. Storage 的完整 `StoredConfiguration` 装配属于 `configuration` feature；Proxy 只加载、解析和写入自己的聚合根。管理 API 的通用错误 envelope 保持集中，但 Proxy/Credential 测试等功能专属 Runtime 错误映射放回对应 Server feature。

## 备选方案

- 只按 `entity/`、`repository/`、`handler/` 等技术层分类：拒绝。一个业务功能仍会散布在多个远端目录，查找成本没有实质下降。
- 在 `lib.rs` 使用大量 `#[path = ...]` 把文件物理归档但保留旧模块图：拒绝。文件系统与 Rust 所有权边界会继续不一致。
- 为每个单文件模块建立同名目录：拒绝。这只是把平铺从文件变成目录，没有形成真实聚合。
- 提取通用 CRUD/repository/handler 框架：拒绝。当前各聚合根的事务、版本、Secret 和发布语义不同，强行统一会隐藏不变量。

## 后果

功能相关文件可以从一个目录完整发现，测试与实现保持邻近，crate 根目录只呈现一级领域地图。应用装配、日志、停机和架构工具检查也具有明确所有权。迁移会一次性修改内部模块路径，但不改变跨 crate 稳定 API 或运行行为。新增文件必须先选择所属 feature；找不到所属边界通常意味着职责仍未定义清楚，而不是应当放回根目录。Allowlist 不再积累已经完成的重构债务，但路径移动和文件拆分必须同步更新对应条目。

## 验证

- `cargo fmt --all --check` 与 workspace Clippy/测试证明模块图和行为未改变。
- `cargo xtask architecture-check` 继续验证依赖方向、文件体积和模块入口门禁。
- `xtask` 单元测试覆盖 Allowlist 条目对应文件缺失和已低于例外阈值两种失效状态。
- `rg --files crates/*/src app/any2api/src xtask/src` 人工核对根目录只保留稳定入口、跨 feature 基础类型和一级 feature。
- Storage、Runtime、Provider 与 Server 契约测试继续枚举真实 Registry/HTTP/Repository 实现，不按旧文件路径猜测覆盖率。
