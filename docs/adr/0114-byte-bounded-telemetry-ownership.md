# ADR-0114：遥测在途所有权按字节有界

- 状态：Accepted
- 日期：2026-08-04
- 决策者：maintainer
- 修订：ADR-0015、ADR-0109

## 背景

RequestTelemetry 当前只按记录数限制逻辑队列，默认允许 4,096 条。普通 RequestLog 很小，但
HttpAccessLog 可以拥有请求和响应各 1 MiB Body，以及两侧 Header、URI 与容器容量。SQLite Writer 卡顿时，
同一个“记录槽”可能代表几百字节，也可能代表超过 2 MiB；因此条数有界并不等于内存有界，默认配置的
理论大 Body 所有权可达到数 GiB。

Writer 从 channel 接收后还会把最多 64 条移动到批次。现有 queue slot 在接收时释放，所以只给 channel
增加字节计数仍会遗漏 Writer 已接收但尚未获得 SQLite 终态的对象。该内存属于可丢失遥测，不应阻塞或拒绝
数据面，也不属于 ADR-0082 禁止的公开请求全局内存准入。

## 决策

1. SettingRegistry 新增热更新整数 `logs.telemetry_queue_max_bytes`：默认 `64 MiB`，允许范围
   `4 MiB..=4 GiB`。原 `logs.telemetry_queue_capacity` 继续限制小记录和 channel 元数据数量；两项必须同时
   满足才接受数据事件。
2. 每个 TelemetryEvent 在入队前计算一次 owned bytes。计算包含事件/Box、String 与 Vec 的实际 capacity、
   HttpAccessLog Header/Body 容器、CompletedRequestLog Attempt 容器及其有界文本；不按 Body 理论最大值
   预留，也不把共享 Repository、channel 或 Runtime 全局对象重复计入每条记录。
3. 准入使用原子计数和失败回滚同时预留逻辑 slot 与 owned bytes。任一边界不足时立即丢弃该遥测并增加
   dropped record，公开请求继续执行，不等待、不返回 429，也不降低 Credential 并发。
4. owned bytes 从成功入队开始一直保留到 SQLite 成功或失败的终态。Writer 接收时只把字节指标从 queued
   转为 in-flight，不释放总字节预留；批次、控制命令前 flush 和存储 await 全部包含在同一所有权生命周期。
   send 失败、存储失败、正常持久化、Writer 停止和 shutdown abort 都只释放一次。
5. Gateway 鉴权拒绝除了现有四分之一 queue slot 子容量，还使用总字节预算四分之一向下取整且最小为一
   字节的子容量。该子计数同样保持到存储终态，廉价未认证大 Body 不能占满正常日志的全部字节预算。
6. 清理等控制事件不拥有大记录，继续通过有序 channel 和控制 slot 执行；它们不计数据记录或 owned bytes。
   热更新下调不会删除已经由 Writer 持有的对象，新事件在当前所有权下降到新上限以内前自然被丢弃。

## 后果

- Writer 停顿时，遥测大对象在 channel 与批次中的合计所有权具有可查询设置定义的硬上界，不再由
  `4,096 × 单条最大捕获` 决定。
- 默认 64 MiB 预算只保护可丢历史，不限制请求 Body、协议解析、Transport、SSE 或连接数量，因此不会复活
  ADR-0082 已删除的进程级公开请求准入。
- `capacity()` 统计会略高于当前有效长度，能够覆盖 Vec/String 已经向分配器申请但暂未使用的空间；结构体
  对齐和分配器元数据仍有很小固定误差，条数上限继续约束该部分。

## 验证

- Domain 测试固定设置默认值、范围、管理元数据和 Web 展示。
- Runtime 测试以不同大小 HttpAccessLog 验证字节不足立即丢弃、queued → in-flight 不释放总所有权、成功/
  失败终态释放、Gateway 鉴权拒绝四分之一子预算，以及 shutdown abort 后记录与字节计数归零。
- 并发测试让 Storage 阻塞并持续提交接近 2 MiB 的日志，证明接受的 queued + in-flight owned bytes 永不超过
  请求捕获 revision 的预算，同时公开数据面调用不等待 Writer。
