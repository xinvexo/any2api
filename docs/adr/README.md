# 架构决策记录

ADR 保存一次重要取舍发生时的背景、决定、备选方案和后果。Accepted ADR 的正文保持历史语境；当前系统事实
由 [架构主题文档](../architecture/README.md) 维护。行为改变不回写旧 ADR，新的决定通过后继 ADR 将旧项标为
`Superseded` 并互相链接。

## 状态

- `Proposed`：讨论中，尚不约束实现；
- `Accepted`：决定已采用；
- `Superseded`：已被链接的后继决定替代；
- `Deprecated`：不再推荐，但没有单一替代项；
- `Rejected`：评估后未采用。

除修正链接、拼写或状态外，不改写 Accepted/Superseded ADR 的历史内容。新的独立取舍复制
[模板](0000-template.md)，使用下一个编号。小型修复、字段清单、默认值和完成日志不创建 ADR。

## 索引

| ADR | 状态 | 决策 |
|---|---|---|
| [0170](0170-current-decision-register.md) | Superseded | 曾将理由合并为一个持续维护的登记册 |
| [0171](0171-modular-single-node.md) | Accepted | 单节点模块化单体与产品边界 |
| [0172](0172-provider-protocol-capabilities.md) | Accepted | Provider descriptor/facet 与协议 target profile |
| [0173](0173-runtime-admission-and-stream-commit.md) | Accepted | RPM 准入、粘性和不可逆流式提交边界 |
| [0174](0174-sqlite-security-and-metadata-telemetry.md) | Accepted | SQLite 本地边界、Secret 隔离与 metadata-only HTTP 日志 |
| [0175](0175-measured-memory-isolation.md) | Accepted | 以内存实测门槛决定底层机制去留 |
| [0176](0176-build-release-and-platform-support.md) | Accepted | 完整应用构建、Linux 发布与其他平台支持等级 |
| [0177](0177-semantic-engineering-governance.md) | Accepted | 语义门禁、主题文档和不可变 ADR |
| [0178](0178-evidence-bound-oauth-quota-estimates.md) | Accepted | 基于官方周期与本地事实的 OAuth 额度估计 |

官方客户端观测属于外部证据，不是 ADR，入口在
[docs/baselines/official-clients](../baselines/official-clients/README.md)。
