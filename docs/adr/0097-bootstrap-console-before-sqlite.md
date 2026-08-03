# ADR-0097: SQLite 之前安装启动期 Console Tracing

- 状态：Accepted
- 日期：2026-08-03

## 背景

旧启动顺序先执行 `SqliteStore::connect`、Migration 和配置加载，再由 `FileLogging::initialize` 同时安装
console 与文件 tracing layer。SQLite 是启动期最可能需要诊断的边界之一，但此时还没有 subscriber；相关
tracing 事件会被丢弃，最终只有 `anyhow` 从 `main` 输出的非结构化错误。直接提前创建文件日志也不正确，
因为有效文件级别、保留期和容量必须来自迁移后加载的 SQLite 配置，而不是另造一套启动默认状态。

全局 tracing subscriber 只能安装一次。先装临时 subscriber、配置加载后再调用一次 `try_init` 会失败；
维护两套全局 dispatch 或跨阶段转发则为单节点应用引入了不必要的生命周期复杂度。

## 决策

1. 只有有效的 Serve 命令进入启动流程；`--version` 与非法命令保持现有无启动副作用语义。Serve 在读取
   启动环境、获取实例锁、创建 Tokio Runtime 和打开 SQLite 之前，一次性安装全局 subscriber。
2. Subscriber 从开始就包含按 `RUST_LOG` 过滤、明确写入 stderr 的 console layer，以及固定注册但
   独立过滤器禁用、Writer 槽为空的文件 layer。SQLite 连接、Migration 与配置加载期间的事件因此立即
   输出到进程诊断流，但不会进入文件格式化或提前创建日志目录。
3. 配置加载成功后，Composition Root 使用该 revision 的有效 `LoggingSettings` 创建私有分段 Writer、
   有界非阻塞队列、动态文件级别和唯一 `WorkerGuard`，先把 NonBlocking writer 放入既有文件层的槽位，
   再启用其过滤器。此过程不得再次安装 subscriber，也不得在运行中装入未参与注册期初始化的新 layer；
   激活只允许发生一次。
4. 启动流程在把错误交给更新回滚或 `main` 之前，记录一个包含固定 `phase` 和完整错误链的结构化
   `startup failed` 事件。错误字段仍遵守 Secret 禁止进入 tracing 的既有边界。
5. `FileLogging` Drop 先禁用文件过滤器、清空 Writer 槽，再释放唯一 Guard。这样普通停机和启动中途
   失败都不让全局 subscriber 留下指向已停止 worker 的 writer；flush 仍是 ADR-0090 定义的
   best-effort 资源。

## 后果

- Migration、SQLite 打开、配置加载及更早的启动失败在默认 console 上可见，并保留阶段和错误链。
- 文件日志仍严格使用 SQLite 的有效设置；启动早期事件不会补写、缓存或重放到文件。
- Console 与预注册文件层共享一个全局 subscriber，避免重复初始化失败、动态 layer 注册缺失和两套
  subscriber 状态。
- `--version` 和非法参数路径不安装 subscriber，也不创建数据目录或日志目录。

## 验证

- 真实子进程使用损坏的 SQLite 文件触发 Migration 前失败，断言 stderr 包含 tracing 的
  `startup failed`、`phase=application` 和存储初始化错误链，且没有提前创建日志目录。
- 正常真实进程继续完成 SQLite 初始化、挂载文件层、监听与 SIGTERM 收尾，证明后续挂载没有重复安装
  subscriber。
- 日志模块测试覆盖预注册文件层从禁用到激活、事件写入以及清空后不再写入。
- 提交前运行 App 单测/进程测试、fmt、clippy、架构与差异门禁。
