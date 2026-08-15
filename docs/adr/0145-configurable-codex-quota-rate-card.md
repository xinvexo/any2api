# ADR-0145：可配置的 Codex 额度费率卡

- 状态：Accepted
- 日期：2026-08-13
- 决策者：maintainer
- 影响范围：SettingRegistry、PublishedSnapshot、Codex OAuth RequestLog 计价、额度 DTO 与 Web
- 相关决策：ADR-0137、ADR-0146

## 背景

本机额度统计需要把请求 Token 换算为 Credits，并把 Credits 换算为管理面展示用的美元等值。
两类费率都可能变化，且历史 RequestLog 必须保留当时使用的费率语义。费率只服务单节点
观测和展示，不构成计费、余额或 Gateway Key 配额。

## 决策

1. SettingRegistry 提供 `oauth.codex.rate_card`，类型为严格校验的 `codex_rate_card`。
   编译默认值表达当前已知费率，SQLite 只保存用户覆盖值。Web 通过独立的“额度费率”页面
   编辑同一个 SettingRegistry 覆盖值，不建立页面私有配置或浏览器持久化副本。
2. 费率卡包含非空版本 ID、正整数 `credits_per_usd`，以及按精确上游模型名索引的
   standard/fast 档位输入、缓存输入和输出 nano-Credits。未知模型或档位保持未计价，
   禁止按名称相似度、最近模型或默认值猜价。
3. 费率卡内容和版本 ID 一起参与发布校验。相同 ID 的内容变化被拒绝；语义修改必须生成
   新 ID。候选模型可由 PublishedSnapshot 中已选择或发现的精确上游名称提供给 Web，候选
   只是编辑建议，不改变 Schema 的精确匹配规则。
4. 配置发布成功后费率卡与其他设置随同一 PublishedSnapshot 原子切换。公开请求在 Attempt
   准备阶段冻结匹配的费率和卡片 ID；RequestLog 保存冻结后的 nano-Credits、费率卡 ID 和
   计价档位，后续覆盖值变化不得重算历史行。
5. OAuth 额度响应携带当前 `credits_per_usd` 与费率卡 ID。真实 Credits 的美元等值和管理面
   本机容量展示使用这一次响应的当前展示汇率；前端不保存定价常量。改变展示汇率只影响
   读取时的显示，不改变 ADR-0146 的规范 Credits 累计。
6. Web 只展示确定容量、累计区间数和当前美元换算，不展示已经从当前契约移除的置信度、离散度、
   样本质量、最近区间或历史费率集合，也不把结果标成上游余额。

## 边界

- Token→Credits 费率变化只影响新请求；历史 RequestLog 的冻结卡片 ID 是其唯一计价依据。
- `credits_per_usd` 变化只改变管理展示换算，不改变已经累计的 Credits 或路由行为。
- 没有匹配费率、完整 usage 或合法数值时，记录从本机成本中排除并保持可诊断的未计价计数；
  不阻断同一窗口的其他可计价记录。

## 验证

- Domain/Storage 测试覆盖严格 Schema、数值边界、默认费率、版本 ID 冲突和设置往返。
- Runtime 测试覆盖请求时冻结费率、历史日志不随新卡重算、未知模型不猜价。
- HTTP/Web 测试覆盖费率编辑、快照原子切换、Credits 与容量共用当前展示汇率，以及不显示
  置信状态或伪造余额。
