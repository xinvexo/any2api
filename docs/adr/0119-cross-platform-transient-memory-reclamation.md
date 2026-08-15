# ADR-0119：跨平台短命大块内存归还

- 状态：Accepted
- 日期：2026-08-05
- 决策者：maintainer
- 修订：ADR-0082、ADR-0114

## 背景

any2api 冷启动后 RSS 较低，但客户端并发发送大 JSON、接收 buffered 响应并启用 HTTP 系统日志后，空闲
RSS 会长期停留在峰值附近。隔离压测已经排除仍在处理的请求、遥测队列积压、旧版本进程和线程数量：日志
关闭时也会复现，日志开启只会进一步放大峰值。

macOS `heap -s` 在压测结束后只发现约 2--4 MiB 仍存活的 malloc 对象，但 `vmmap` 显示超过 120 MiB
已经为空的 malloc region 仍有驻留页；Linux 同期表现为高位 `RssAnon`/`AnonHugePages` 且没有 `LazyFree`。
请求、响应、协议 JSON 和两侧各 1 MiB 日志前缀虽然都已按 Rust 所有权释放，通用分配器仍会为了 arena、
size class 和后续复用保留脏页。因此这不是全量日志仍被内存持有，也不是单一 Vec 泄漏，而是长生命周期
代理把大块短命数据与小型长期对象放入同一通用堆后产生的跨平台高水位与碎片问题。

替换全局分配器的 A/B 结果不具备跨平台一致性：mimalloc 和 snmalloc 在 Linux 或 macOS 至少一端提高了
基线、峰值或空闲 RSS。只调用 Linux `malloc_trim` 又不能解决 macOS 上仍被碎片占住的 region。按机器设置
并发或内存阈值则会恢复 ADR-0082 已删除的错误准入模型。

## 决策

1. 保留平台系统分配器，不提供 allocator、arena、THP、进程 RSS 或按服务器规格调整的运行参数。
2. 新增独立 `payload-buffer` 基础 crate。公共请求多块聚合、zstd 解压、Images multipart 重编码、
   Provider/Protocol identity JSON 编码、buffered 上游成功响应和 HTTP 请求/响应 Body 前缀捕获统一
   使用它：小于 `256 KiB` 时使用普通 `Vec`；达到该阈值后把现有内容移动到匿名私有映射，后续扩容
   继续替换映射。它同时提供普通增量追加和 `std::io::Write` 边界，使解码器、压缩器与序列化器可以直接
   写入最终存储。最终 `Bytes` 直接拥有 Vec 或映射，最后一个所有者 Drop 时大映射立即由操作系统解除，
   不依赖通用堆清理 arena；已经由网络栈提供的单块共享 `Bytes` 仍保持零拷贝路径。
3. 现有单请求硬上限保持唯一大小边界。`Content-Length`/`size_hint` 只有在不超过该边界时才能作为容量
   提示，实际累计仍在每次写入前用 checked arithmetic 验证；提示超限时忽略提示并按实际字节读取。禁止按
   `32 MiB`/`64 MiB` 理论上限预留，禁止新增全局字节预算、并发 Semaphore、等待队列或本地 429。
4. Payload 冻结结果同时携带真实 owned allocation bytes：Vec 使用 capacity，映射使用映射长度。HTTP Body
   捕获把该所有权直接移动到 move-only 的 `HttpAccessLog`，Writer 再把批次按值交给 Storage，SQLite 绑定
   只借用同一字节，不再深拷贝完整前缀；ADR-0114 的遥测准入继续按该真实分配量计费。捕获扩容失败时
   停止继续捕获并把日志标记为截断，不能让可丢失日志改写或中断代理 Body。
5. 新增独立 `memory-reclaimer` crate 作为唯一原生 FFI 边界。Composition Root 每 30 秒检查一次：只有自
   上次回收后发生过 HTTP 请求且检查时活动请求为零，才在 blocking 线程调用一次平台能力；没有新活动时
   不重复空转回收。
   - Linux GNU：`malloc_trim(0)`；
   - macOS：`malloc_zone_pressure_relief(NULL, 0)`；
   - Windows：`HeapSetInformation(NULL, HeapOptimizeResources, ...)`，参数使用 version 1、flags 0 的
     `HEAP_OPTIMIZE_RESOURCES_INFORMATION`；
   - 其他目标：安全 no-op。
6. Workspace 继续全局 `forbid(unsafe_code)`。只有 `memory-reclaimer` 不继承该 forbid，并在 crate 级
   `deny(unsafe_code)` 下对三个短小平台调用逐处写明 Safety、局部允许；它不暴露指针或 unsafe API。
7. SQLite 写池继续固定一条连接并由 60 秒周期遥测维护复用。读池仍允许突发时扩展到 8 条，但显式设置
   60 秒 idle timeout，使长期闲置的读连接及其辅助线程/缓存退出；这属于池生命周期，不是请求准入或
   服务器规格限制。写池使用相同 idle timeout 的原生 Linux A/B 会与周期维护形成关闭/重连抖动，且没有
   降低最终 RSS，因此不保留该改动。
8. CI 除 Linux 全套门禁外，使用 macOS 与 Windows 原生 runner 实际运行 `payload-buffer` 与
   `memory-reclaimer` 基础测试，再完整链接 release 二进制，防止平台符号、cfg、原生调用或匿名映射支持
   回归。RSS 结论使用隔离进程和固定工作负载验证，不把一次性 benchmark 脚本提交为产品运行逻辑。
9. 同协议 buffered JSON 响应走 raw direct decode：完整语法仍使用 `IgnoredAny` 校验，token usage、Responses
   Continuation ID 和顶层 model 只从 wire bytes 借用扫描，不 materialize 完整 `serde_json::Value`。只有
   Protocol Bridge 确实需要结构转换时才使用原有 structured decode；Bridge 完成后继续由 ingress adapter
   编码转换结果。该分流由 `ProtocolExchange` 决定，Provider、Runtime 和平台代码不得各自猜测。
10. `payload-buffer` 是不携带协议、Provider、调度或 HTTP 语义的基础所有权原语，因此 `protocol` 与
    `provider` 可以直接依赖它完成各自序列化边界；该依赖不得反向引入 Runtime 规则。所有调用方继续服从
    自己已有的输入/输出硬上限，不由该 crate 新增按机器规格、进程 RSS 或并发量变化的限制。

## 后果

- 大请求、入口 zstd 解压、Provider/multipart 正文改写、buffered 响应和原始 HTTP 捕获不再把主要短命工作集
  留给通用堆；日志仍会增加必要的实时工作集和 SQLite 磁盘占用，但不再被误认为永久存活的全量内存日志。
- 直通 JSON 响应不会再为输出文本、图片 base64 或未知大字段建立第二棵堆对象树；语法错误、usage 统计、
  Continuation 身份和公开模型名改写语义保持不变。跨协议转换仍承担必要的结构化分配。
- 映射创建和跨阈值搬移比纯 Vec 多少量系统调用；只有较大、多块聚合对象承担该代价，小请求和单块共享
  `Bytes` 保持原路径。
- 空闲回收是对通用堆碎片的补充，不保证 RSS 精确回到冷启动数值。运行时线程栈、连接池、SQLite 页缓存、
  协议小对象和代码页仍形成稳定基线；操作系统也可以选择延后统计更新。
- musl 等没有对应显式压力释放 API 的平台仍受益于匿名映射的确定性 Drop，reclaimer 保持 no-op，不通过
  不可靠符号探测或私有 API 扩大 unsafe 面。
- 本决策不限制合法并发，不按机器容量猜测 OOM，也不改变 RPM、路由、重试、日志保留或公开 HTTP 契约。

## 验证

- `payload-buffer` 单元测试覆盖阈值两侧、从堆迁移到映射、`Write` 增量路径、错误容量提示、实际长度硬
  上限、冻结后内容和 owned allocation bytes。
- Server/Runtime 测试覆盖任意分块聚合、单块零拷贝、超限错误、上游读取错误元数据，以及日志捕获分配
  失败的 fail-open/截断语义；Domain/Runtime 遥测测试继续证明字节预算不低估映射所有权。
- Lifecycle/Composition Root 测试证明无请求活动、仍有活动请求和同一 activity epoch 不回收，新的活动在
  空闲检查时只触发一次。
- Storage 测试固定读池 idle timeout 与写池单连接不变量。
- Protocol 测试证明 direct decode 校验完整 JSON、只保留 wire body、从 raw body 提取 usage/Continuation ID，
  而 Bridge decode 仍提供完整结构并通过既有转换契约；入口 zstd、identity JSON 与 multipart 的大输出
  测试证明序列化结果正确且最终所有权进入映射。
- Linux AMD64 GNU、macOS ARM64 与 Windows x86_64 构建完整链接；Linux/macOS 隔离压测使用相同 16 并发、
  每侧约 2 MiB 的请求/响应，并分别验证系统日志关闭和开启后的峰值与空闲 RSS。

2026-08-05 的最终隔离结果如下；空闲列在请求结束 90 秒后采样，遥测 queued/in-flight 均为 0：

| 平台 | 原始 HTTP 日志 | 冷态 RSS | 峰值 RSS | 90 秒空闲 RSS |
|---|---:|---:|---:|---:|
| Linux AMD64 GNU | 关闭 | 35.4 MiB | 78.7 MiB | 37.5 MiB |
| Linux AMD64 GNU | 开启 | 35.0 MiB | 109.0 MiB | 49.4 MiB |
| macOS ARM64 | 关闭 | 40.9 MiB | 79.2 MiB | 34.8 MiB |
| macOS ARM64 | 开启 | 40.8 MiB | 96.9 MiB | 43.1 MiB |

Linux 数据来自原生 x86_64 GNU 进程而非跨架构 QEMU；同一工作负载在修改前的 90 秒空闲 RSS 为：日志
关闭约 104.4 MiB、开启约 111.9--118.5 MiB。日志开启的最终数据库仍持久化 16 行、32.02 MiB 原始
exchange，证明磁盘日志保留与 live RSS 已解耦。关闭/开启日志时，进程线程数分别从冷态 8/7 条降到
90 秒时 5 条，证明 Tokio blocking 与池辅助线程会按已有 idle 生命周期退出；固定 Runtime Worker 和必要
后台任务构成稳定基线。macOS 两组在 90 秒越过读池退休点后同样回到冷态附近，日志开启组的 SQLite 也
完整保留 16 行、32.02 MiB exchange。
