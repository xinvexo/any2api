# ADR-0147：分离 Codex 工作区路由标识与成员 OAuth 身份

- 状态：Accepted
- 日期：2026-08-13
- 决策人：maintainer
- 修订：ADR-0033、ADR-0087、ADR-0133、ADR-0134

## 背景

Codex Token 的 `https://api.openai.com/auth.chatgpt_account_id` 表示登录时选择的个人账户或工作区，数据面需要把它作为 `chatgpt-account-id` Header 发送。Team 工作区内不同成员会共享这个值。成员主体由同一 claim namespace 下的 `chatgpt_user_id` 表示，旧形态使用 `user_id`。

旧实现把 `OAuthTokenMaterial.account_id` 同时用于出站路由和本地稳定身份。结果是同一 Team 工作区的第二个成员在交互式登录时被误判为第一条记录的重新授权并覆盖 Token；JSON 导入则被误报为重复账号。工作区相同不能证明成员相同。

## 决策

1. `OAuthTokenMaterial.account_id` 继续保存 Codex 工作区路由标识，Codex 数据面继续用它构造 `chatgpt-account-id`；不改变 Endpoint、模型、额度或请求协议行为。
2. 稳定 OAuth 主体改由 `ProviderDriver` 投影。投影值只保存域分隔 SHA-256 指纹，是短期、不透明、可比较且脱敏 `Debug` 的内部类型；成员 ID、邮箱与工作区 ID 原值不经该类型进入 SQLite、DTO、日志、错误正文或浏览器状态。
3. Codex Driver 从已规范化 Token 文档的 ID Token 提取 `chatgpt_user_id`，兼容 `user_id`；外部导入文档在 ID Token 缺失 claim 时还可从 access token 提取。存在成员主体时，以 `(可选 chatgpt_account_id, member_id)` 组成稳定身份：同一工作区的不同成员保持独立，同一成员选择不同工作区也保持独立。
4. Codex 缺少成员主体 claim 时，只能回落到规范化邮箱；不得单独使用 `chatgpt_account_id` 做账号去重。若连邮箱也缺失，只允许 ADR-0134 的同类型精确 Token 证据证明重复。
5. Claude、Grok 及后续未覆写该投影的 Provider 保持原规则：非空 Provider account ID 优先，无 account ID 时回落规范化邮箱。Provider 隔离与精确 Token digest 规则不变。
6. 交互式重新授权、JSON 导入与额度估算的凭据代际指纹共用同一 Driver 投影。Codex 成员主体或工作区变化必须使旧额度统计失效，普通 Token 轮换不能清空同一成员的统计。
7. 交互式重新授权和 JSON 导入继续在串行发布锁内比较当前快照与同批输入。重新授权响应若没有新 ID Token，保留已有 ID Token 作为后续主体证据；新 Token 版本仍替换 access/refresh 材料并遵守 CAS。
8. 已经被旧行为覆盖掉的另一成员 Token 无法从现有一条记录恢复，不做启动期猜测或数据重写。升级后管理员重新登录缺失成员即可创建独立账号；被覆盖记录的旧额度统计会在下一次权威额度观测时因主体指纹变化而清空。

## 后果

- 同一 Team 工作区可以导入和路由多个不同成员账号，且各自拥有独立的本地 OAuthAccount、RPM、健康、额度与粘性状态。
- 同一成员重新登录仍能命中原记录，不产生重复候选；工作区 Header 行为保持不变。
- 不新增持久化字段或 Migration。成员身份在发布临界区内从现有 Token 材料动态提取。
- 无成员 claim 的老旧 Token 不再把共享工作区当成强身份；这可能允许无法证明相同的材料创建新记录，但优先避免覆盖另一个真实成员。

## 验证

- Provider 测试覆盖同一工作区不同 `chatgpt_user_id` 不同、同一成员刷新后相同、不同工作区同一成员不同，以及 `user_id` 兼容和邮箱回落。
- Runtime 导入测试覆盖同一 Team 工作区两个成员均可创建，同一成员重复导入仍原子拒绝。
- Runtime 交互式登录测试覆盖同一 Team 工作区第二个成员新建、同一成员重新授权复用原 `OAuthAccountId`，且出站仍使用各自工作区 ID。
