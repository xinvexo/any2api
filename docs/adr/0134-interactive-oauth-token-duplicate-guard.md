# ADR-0134：交互式 OAuth 精确 Token 重复保护

- 状态：Accepted
- 日期：2026-08-11
- 决策人：maintainer
- 修订：ADR-0087、ADR-0133

## 背景

ADR-0087 让交互式 OAuth 登录按 Provider account ID 或邮箱复用已有
`OAuthAccount`。但部分 Token 响应可能没有可用的 account ID/邮箱；在这种情况下，
重复登录会按“无身份即新建”路径生成第二条本地路由凭据。若新旧材料包含完全相同的
access、refresh 或 ID Token，这已经足以证明它们是同一认证材料，不应继续扩大路由候选。

导入路径已由 ADR-0133 使用短期 Token digest 拒绝这类重复；交互式登录必须使用同一
证明规则，同时保留稳定身份优先和冲突时不猜测的边界。

## 决策

1. 交互式登录在既有发布锁内同时计算两类命中：稳定 Provider 身份命中，以及同一
   Provider 下 access/refresh/ID Token 任一完全相同的 Token-kind 命中。digest 只存在于
   当前发布临界区，不进入 SQLite、日志、DTO、Debug 或浏览器状态。
2. 没有稳定身份时，精确 Token 只命中一条现有账号则复用该 `OAuthAccount` 的重新授权
   路径；命中多条则返回 `oauth_account_identity_conflict`。因此绝不会因为缺少身份字段
   新建第二条相同凭据。
3. 新 Token 的稳定身份与精确 Token 分别命中不同账号时，视为历史配置冲突并
   fail-closed；不按 Token、邮箱或 account ID 的任一弱优先级猜测覆盖。
4. 稳定身份只匹配同 Provider account ID，或双方都没有 account ID 时的规范化邮箱。
   不同 Token 且无法证明稳定身份相同的登录仍允许创建新账号，避免把共享邮箱或相似标签
   错误合并。
5. 所有复用、新建和冲突判断都在一次串行发布临界区中完成；新建或重新授权继续遵守
   SQLite 事务、Runtime reconcile、`PublishedSnapshot` 单次切换和 Token version CAS。

## 后果

- 无稳定身份的重复登录不会再扩大同一官方凭据的路由、刷新、额度或连接隔离对象数量。
- 精确 Token 证据不足以唯一定位时显式返回冲突，管理员可以先清理历史重复记录。
- 不改变 OAuth Provider Endpoint、DIRECT/全局代理、Transport wire profile、Header
  persona 或任何随机化策略；这是认证身份一致性修复，不是隐藏网关或规避 Provider 风控的层。

## 验证

- 纯身份测试覆盖：精确 access/refresh/ID Token 命中、Provider 隔离、稳定身份与 Token
  命中不同账号、多个精确命中和不同 Token 保持独立。
- Runtime 发布测试覆盖：无稳定身份的单一精确命中不会创建新账号；冲突时 revision、
  SQLite、Runtime reconcile epoch 和 PublishedSnapshot 均不变化。
