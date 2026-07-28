# ADR-0058：首版前正确性重基线

- 状态：Accepted
- 日期：2026-07-28
- 取代：ADR-0006 的 Gateway Key 生成与删除模型、ADR-0042、ADR-0049 的空列表语义

## 背景

首个正式版本尚未发布，项目不承担旧数据库、旧管理 API、旧内部类型或旧测试夹具的兼容义务。审查发现
若继续沿用开发期模型，会留下白名单 Fail-Open、OAuth Device session 丢失、管理员 Session 无界增长、
Gateway Key 双轨状态和大量只服务于废弃 Schema 的 Migration 代码。
Release 只需要由管理员手动输入版本号，完成对应版本打包并创建 Tag，不扩张 CI 架构。

## 决策

1. `models.allowed` 改为显式 `ModelAccessPolicy::All | Only(set)`。JSON `null` 表示 `All`，数组表示
   `Only`，其中空数组表示拒绝全部。Route 变化裁剪到空集合时保持 `Only(empty)`，不扩大权限。
2. Grok Device 轮询使用带代际的 `DevicePollLease`。Store 在网络 I/O 期间保留容量占位；Lease 的正常
   完成、错误和 Drop 都通过单一状态转换结算。Pending、SlowDown 和非终止错误恢复 session，授权拒绝、
   Provider 过期和成功激活终止 session。
3. 管理员会话由专用 `AdminSessionStore` 管理，固定上限 32。签发和认证前全局清理过期记录，满额时
   淘汰最早签发的会话；密码轮换仍原子撤销全部旧会话并只签发当前会话。
4. Gateway Key 只由服务端从 32 个 CSPRNG 字节生成，格式为 `a2k_v1_` 加 URL-safe Base64 无填充正文。
   创建和轮换不接收客户端 Secret。删除使用 `DELETE`，领域、SQLite、DTO 和 Web 不再存在 `revoked_at`、
   revoke 状态、旧路由或错误别名。
5. 仓库在首版前重置为单个规范 `0001_initial.sql`。删除开发期增量 Migration、pre-v24 修复器和升级
   夹具；开发数据库直接重建。正式版本发布后才冻结 Migration 历史。
6. Release 使用 `workflow_dispatch` 接收版本号，校验 Cargo 版本后打包，并创建对应 `v<version>` Tag/Release。
   不为这项需求改造普通 CI、引入可复用工作流或复制额外门禁。

## 结果

- 权限策略不再依赖空值哨兵，配置裁剪只能保持或收紧权限。
- OAuth 轮询取消和临时故障不会销毁仍有效会话，64 个 session 上限覆盖轮询中的会话。
- 管理认证内存使用有明确上界。
- Gateway Key、管理 API 和数据库只保留一个当前模型。
- Schema 可从一个可读基线直接审查，测试只验证当前结构与不变量。
- Action 保持单一、直接的手动版本打包与 Tag 发布路径。

## 被拒绝的方案

- 用空数组同时表示允许全部和拒绝全部：无法在裁剪后 Fail-Closed。
- 在 Device poll 出错分支逐个手工插回 session：请求取消仍会绕过，且无法可靠计入容量。
- 给旧 Gateway Key 字段、路由和 Migration 留兼容别名：项目没有相应数据或客户端兼容要求。
- 为手动版本发布重构整套 CI：超出需求，增加维护面和发布耦合。
