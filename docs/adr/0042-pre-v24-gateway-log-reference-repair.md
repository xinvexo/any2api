# ADR-0042：Migration 24 前归一化悬空 Gateway Key 日志引用

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

一份真实的 Migration 23 数据库包含 `request_logs.gateway_api_key_id` 对已删除 Gateway Key 的悬空引用。该列从创建起声明为 `ON DELETE SET NULL`，但历史版本曾留下不符合这一语义的遥测行。Migration 24 重建 Provider 外键图并在结尾执行完整 `foreign_key_check`，因此正确拒绝了这份数据库，但也导致应用无法启动和完成升级。

Migration 24 已发布且历史 checksum 固定，不能回写其内容。RequestLog 又是可降级历史遥测，不是配置真相；把已不存在的 Gateway Key 引用改为 `NULL` 与原始外键删除语义一致，也不会恢复、猜测或改变任何凭据。

## 决策

1. Storage 在运行正式 Migrator 前读取当前成功应用的 migration 版本。
2. 仅当数据库已经包含 RequestLog（版本不低于 9）且尚未应用 Migration 24 时，执行一次事务化归一化：将找不到父行的非空 `request_logs.gateway_api_key_id` 更新为 `NULL`。
3. 不修改任何历史 Migration 或 checksum，不删除 RequestLog，不修改 Gateway Key、ProviderEndpoint、ProviderCredential、Route、Proxy、OAuthAccount 或 Secret。
4. 不实现通用数据库自动修复。其他外键违反继续由 Migration 24/25 的完整性守卫拒绝，避免掩盖配置损坏。
5. 新增升级测试，以关闭外键后构造真实的悬空 Gateway Key 日志引用，验证升级到最新版本后日志保留、引用归零且 `foreign_key_check` 为空。

## 结果

- 受影响的旧库可以在不丢失请求历史和不改写配置的前提下完成 Migration 24 与后续 Migration。
- 已经应用 Migration 24 的数据库不会进入该兼容路径。
- 迁移前归一化是针对已复现历史缺陷的一次性边界，不扩张为备份、恢复或任意损坏修复系统。

## 未选择方案

- 修改 Migration 24：会破坏不可变 migration 历史和 checksum 契约。
- 删除全部请求日志：虽然日志可降级，但没有必要丢弃仍然有效的历史数据。
- 自动清理所有外键错误：可能静默改变配置真相并掩盖严重损坏。
- 要求用户手工修改 SQLite：不可测试、容易误删数据，也无法让后续相同升级自动安全完成。
