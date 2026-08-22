# ADR-0170: 合并当前决策理由

- 状态：Superseded
- 日期：2026-08-19
- 被替代日期：2026-08-22
- 被替代：[ADR-0171](0171-modular-single-node.md)、[ADR-0172](0172-provider-protocol-capabilities.md)、
  [ADR-0173](0173-runtime-admission-and-stream-commit.md)、[ADR-0174](0174-sqlite-security-and-metadata-telemetry.md)、
  [ADR-0175](0175-measured-memory-isolation.md)、[ADR-0176](0176-build-release-and-platform-support.md)、
  [ADR-0177](0177-semantic-engineering-governance.md)、
  [ADR-0178](0178-evidence-bound-oauth-quota-estimates.md)

## 背景

仓库早期存在许多重复当前实现的小型决策文档。为了快速收敛，它们曾被合并成一个持续维护的登记册，并以一份
大型架构基线描述所有当前事实。该方式减少了当时的重复文件，但随后让无关主题共享生命周期，也使小型实现
变化频繁改动同一个文档。

## 当时的决定

- 当前事实集中维护，理由和舍弃方向进入一个登记册；
- README 面向使用者，官方客户端 baseline 保存外部证据；
- 已完成的整改过程不作为长期完成日志；
- 个人单节点定位、模块化单体和 SQLite 作为总体边界；
- Provider、Protocol、Transport 分离，显式能力优先于兼容猜测；
- RPM、粘性、类型化重试和流式提交边界共享一套 Runtime；
- 配置通过完整候选和 PublishedSnapshot 原子发布；
- Secret 受本地数据目录和最窄使用边界保护；
- 管理 Web 使用服务端事实、响应式浏览器布局和统一实时连接；
- 完整应用由同一构建生命周期组合 Rust 与 Web，发布为单一二进制。

## 保留的取舍理由

### 产品和模块

个人、自托管、单节点使 SQLite、进程内 Runtime 与单一二进制足够且易审计。Gateway Key、Provider API Key
和 OAuthAccount 生命周期不同，只在运行时 `RoutingCredential` 投影处合流，从而复用调度、健康与重试。
分层 Provider/Protocol/Transport 避免把供应商认证、线协议和网络失败混进中央执行器。

### 路由和流式

RPM 是面向管理员的本地准入限制；`in_flight` 表示资源生命周期。会话绑定完整候选，只有尚未向下游提交且
存在明确安全证据时才能重试或换路。网络 chunk 与 SSE 帧不同，因此协议必须增量解析；提交后保持单一路径，
不拼接不同上游响应。

### 存储和配置

SQLite 保存配置和必要凭据，运行态不恢复。配置候选在事务中编译，提交后以完整 revision 发布。Schema 通过
不可改写的前向 Migration 演进，兼容转换停留在 Migration 或外部导入边界。

### 内存与运维

大块短命 payload、zstd workspace 与长期小对象具有不同回收特征，因此底层分配策略以观测到的 RSS 和尾延迟
为依据，并隔离 unsafe 平台实现。Node tooling 组合完整 Web/Rust 应用，Cargo 保持 Rust-only；Linux AMD64
是正式发布边界，更新器不替代外部 supervisor 或数据库恢复。

### 可观测性和 Web

逻辑请求和上游 Attempt 分层统计，实时快照不伪装成持久化历史。浏览器使用后端能力与配置 revision，不自行
推断路由。实时状态通过共享管理员连接分发，历史列表通过 SQLite Cursor 恢复。

### OAuth 额度估计

本地额度估计以官方周期边界和持久化 RequestLog 成本为依据，不从相邻刷新差值、余额自然语言或短暂小百分比
猜测消耗。容量外推需要最低官方使用率；购买 Credits 接管耗尽窗口后冻结 included-window 估计，避免混入后续
付费消耗。稳定 Provider 主体用于跨 token 刷新保持同一容量身份，Fast 成本只使用上游最终确认的执行档位。

## 当时舍弃的方向

| 方向 | 结论 |
|---|---|
| 多租户、计费、支付、Key 销售与分布式调度 | 不属于产品边界 |
| Redis、PostgreSQL、消息队列与微服务 | 不用于解决单节点问题 |
| 通用配置、Secret、数据库导入导出 | 只保留当前 SQLite 模型和受支持 Provider 导入 |
| TPM、并发、权重等第二套公开准入策略 | 只保留 RPM 与明确的对象容量上限 |
| WebSocket 或跨 Provider 双向万能桥 | 当前公开入口使用 HTTP JSON/SSE 与显式 Bridge |
| 用 URL、模型名或自然语言猜测 Provider 能力 | 只有注册契约和可审计证据可影响能力 |
| prompt-cache 软路由或修改同方言请求区分账号 | 请求面保持连续，粘性只承担显式会话语义 |
| 请求、队列、会话、健康和事件回放 | 进程启动后从空 Runtime 建立 |
| 长期兼容读取旧 Schema/浏览器格式 | 在 Migration 或导入边界一次性收敛 |

## 被替代原因

一个登记册无法给不同决策独立状态，也会把当前事实、理由和维护规则重新耦合。后继 ADR 将以上理由按决策拆分，
当前事实移入少量主题文档。本文件停止维护，仅作为 2026-08-19 合并阶段的历史索引。
