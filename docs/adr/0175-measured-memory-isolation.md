# ADR-0175: 以内存实测门槛决定底层机制去留

- 状态：Accepted
- 日期：2026-08-22
- 当前事实：[运维：内存底层组件](../architecture/operations.md#内存底层组件)
- 替代：[ADR-0170](0170-current-decision-register.md) 的内存部分
- 被替代：无

## 背景

大型请求、压缩 workspace 和长期 Runtime 小对象具有不同分配寿命。仓库曾分别为大块 payload、zstd
workspace 和平台堆回收建立底层 crate，以解决常驻 RSS 问题。无条件保留自定义 allocator 会增加 unsafe、
平台分支和尾延迟风险；代码已经存在不能替代与标准实现的同负载测量。

## 决策

保留 `payload-buffer` 的至少 2 MiB 大块映射路径：256 MiB 代表性释放测试中，它虽约慢 3 倍，但 RSS 能立即
回到基线，普通 `Vec`/mimalloc 不回落。小于门槛的 payload 仍使用普通分配，避免把 mmap 成本扩散到常见请求。

保留 `memory-reclaimer` 作为平台堆压力释放的窄边界。空闲回收只改善可归还性，不形成全局内存准入，也不通过
唤醒所有线程或强制全局收集伪造确定性。

删除自定义 zstd workspace allocator 及独立 crate。它没有测得 RSS 优势，且标准安全 streaming 解码在
64 KiB、1 MiB、2 MiB、8 MiB 样本中分别快约 15%、12%、2%、3%。有界 zstd 解码回归 Server，由标准接口
承担，不再维护专用 unsafe allocator。

## 备选方案

- 所有 payload 始终使用普通 `Vec/Bytes`：最简单，但 256 MiB 释放后 RSS 不回落。
- 所有中型 payload 直接 mmap：增加映射、迁移和系统调用成本。
- 全局替换所有 C/Rust allocator：把 SQLite 等原生依赖纳入更大的未验证故障域。
- 后台周期强制收集：可能制造停顿，并没有跨线程完成语义。
- 保留自定义 zstd allocator：没有 RSS 收益且在全部测量档位更慢。

## 后果

大块 payload 的可归还性得到保留，平台回收仍与业务代码隔离；同时删除一个无收益的 crate 和一条 unsafe
分配路径。代价是大块映射牺牲吞吐，平台回收仍需要原生验证。负载或关键依赖变化后不能引用本次数字代替复核。

## 验证

代表性 payload/zstd 对照基准、空闲后 RSS、P95/P99 延迟、映射次数，以及 Linux/macOS/Windows 底层单元测试。
