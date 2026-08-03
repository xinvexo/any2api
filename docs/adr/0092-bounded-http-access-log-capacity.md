# ADR-0092：HTTP 系统日志独立容量与有界 SQLite 回收

- 状态：Accepted
- 日期：2026-08-03
- 决策人：项目维护者
- 修订：ADR-0015、ADR-0051、ADR-0081

## 背景

`RequestLog` 每行只有路由与用量摘要，`HttpAccessLog` 却可以保存请求/响应各 1 MiB Body 以及两侧原始
Header。现有实现把 `logs.request.max_rows` 同时用于两个顶层表，默认 200,000 行；该上限对 RequestLog
合理，却允许系统日志仅按 Body 理论值增长到数百 GiB。`logs.file.max_total_size` 只控制 JSONL 目录，不能
约束 SQLite，名称又容易让管理员误以为全部日志已有总容量边界。

当前 bundled SQLite 没有启用 `SQLITE_DEFAULT_AUTOVACUUM`，`SqliteStore` 也未在建表前选择模式，因此
数据库默认 `auto_vacuum=NONE`。删除记录只把页面放入 freelist，后续写入可以复用，但主文件不会缩小。
SQLite 官方同时规定：既有含表数据库从 `NONE` 切换为 `INCREMENTAL` 必须执行完整 `VACUUM`；该操作重建
整个数据库，并可能需要约原库两倍的额外磁盘空间。对一个正因原始交换日志而异常膨胀的库，在启动迁移中
强制执行它会把容量问题变成启动阻断。

仅增加字节预算也不完整：没有行数上限时，大量无 Body/小 Body 记录仍可让元数据和索引无界增长。反过来，
仅拆出行数上限无法控制大小相差数个数量级的记录。因此两条边界必须同时存在，但不需要引入第二个数据库、
永久统计服务或复杂压缩格式。

## 决策

1. `logs.request.enabled`、`logs.request.retention` 与遥测队列继续由两类 SQLite 日志共用；容量不再共用。
   `logs.request.max_rows` 只约束 RequestLog。SettingRegistry 新增：

   | 设置 | 默认值 | 允许范围 |
   |---|---:|---:|
   | `logs.http_access.max_rows` | `200000` | `1..=10000000` |
   | `logs.http_access.max_exchange_bytes` | `256 MiB` | `1 MiB..=64 GiB` |

   两项均热更新，并由 Web“日志”设置展示默认值、覆盖值和生效值。
2. 前向 Migration 为 `http_access_logs` 增加 `exchange_bytes`。它精确等于该行已序列化的请求 Header、请求
   Body、响应 Header 和响应 Body BLOB 字节之和；迁移前没有交换详情的记录为零。独立行数上限负责约束
   URI、摘要、行与索引等固定/元数据开销，因此字节设置不伪装成 SQLite 文件大小硬配额。
3. 每个 HttpAccessLog 批次在同一 `BEGIN IMMEDIATE` 事务内先插入，再通过只包含时间、ID 与
   `exchange_bytes` 的保留索引计算容量。若任一上限超出，按 `started_at_ms, request_id` 从最旧记录开始删除
   最少数量的完整行，提交后的表同时满足行数与交换字节预算。单条交换本身超过预算时该记录最终不会保留；
   禁止只截短已经按 ADR-0081 捕获的某一侧 Body、删 Header 或保留无法解释的半条详情。
4. 每分钟保留任务仍分批删除过期记录，并再次执行独立容量裁剪，使没有新流量时的设置下调也会生效。批次
   插入导致的容量删除、周期清理和手动清理只要实际删除行，就推进 `system_logs_changed`；只写入被通知抑制
   的系统日志列表记录但同时驱逐旧行时，也必须通知页面重新读取。
5. 新建 SQLite 文件在任何表创建前设置 `auto_vacuum=INCREMENTAL`。遥测 Writer 每个 60 秒保留周期都在
   普通删除事务提交后执行一次最多约 16 MiB 页面的 `PRAGMA incremental_vacuum(N)`；手动清空后立即执行
   同一有界步骤，余下 freelist 由后续周期继续回收。回收失败只记录告警，不回滚已经成功提交的日志删除，
   也不阻塞公开请求。
6. 已有 `auto_vacuum=NONE` 数据库不在在线启动或 Migration 中隐式执行完整 `VACUUM`。新容量策略仍立即
   生效，删除页会被后续 SQLite 写入复用，从而停止按原始交换历史峰值继续增长；若管理员必须让旧主文件
   立即缩小，应在服务停止并确认有足够临时磁盘空间后显式执行一次
   `PRAGMA auto_vacuum=INCREMENTAL; VACUUM;`。运行中管理 API 不新增无界全库重写操作。
7. 物理数据库文件还包含 RequestLog、配置、索引、页面碎片与 WAL，不能用交换字节设置承诺逐字节相等的
   文件大小。增量回收只释放完整 freelist 页面，不做全库重排；这是对单节点本地日志容量足够且风险更小的
   边界。

## 后果

- 大 Body 系统日志不再能借用 RequestLog 的高行数上限无界占用 SQLite；小记录也受独立行数上限约束。
- 容量压力只淘汰完整最旧记录，单条详情的原始交换语义保持可解释。
- 新数据库会逐步把已释放页面返还文件系统；旧数据库先复用历史 freelist，避免一次高风险在线重写。旧库若
  需要回收已经形成的峰值文件，仍有一次明确的离线维护成本。
- 容量检查和求和只扫描窄覆盖索引，不读取最高 2 MiB 的详情 BLOB；普通请求继续只做非阻塞入队。

## 验证

- Domain/管理契约/Web 测试覆盖两个新设置的默认值、范围、热更新元数据、中文标签和字节单位。
- Migration 升级测试用迁移前的有详情与无详情代表记录验证 `exchange_bytes` 回填、行保留和窄索引列。
- Storage 测试覆盖行数裁剪、交换字节裁剪、超预算单行、时间清理与手动清理；查询计划证明容量统计使用
  覆盖索引而不读取 Body。
- Runtime 测试固定当前策略分别传递 RequestLog 与 HttpAccessLog 容量，并验证容量驱逐会推进变更 epoch。
- SQLite 连接测试验证新库为 `INCREMENTAL`、既有 `NONE` 库不会被隐式完整重写，以及有界回收会减少
  freelist/page count；工程门禁继续冻结既有 Migration checksum。
