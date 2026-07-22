# ADR-0028: Credential 驱动的模型发现与选择

> 状态：Accepted
> 日期：2026-07-23
> 决策者：maintainer

## 背景

现有管理面要求用户理解 `ModelRoute`、`RouteTarget`、`fallback tier`、入口协议和上游模型之间的关系。对于个人自托管实例，这把内部调度结构暴露成了主配置流程；用户已经提供 API Key 和 Provider URL，却仍需手工重复填写上游模型名并拼装路由。

同一 Endpoint 下不同 API Key 可能拥有不同模型权限，因此模型集合不能只保存在 Endpoint，也不能仅根据某一把 Key 的 `/models` 结果推断其他 Credential。

## 决策

- 每把 `ProviderCredential` 独立保存用户确认的上游模型集合，SQLite 使用 `provider_credential_models(credential_id, upstream_model)`。
- 新建 Credential 初始不发布任何模型。管理面保存 API Key 后，使用该 Credential 当前 generation、实际代理与 Endpoint 请求 `GET /models`。
- 轮换 API Key 时原子清空该 Credential 的旧模型集合与内部路由，避免把旧 Key 的权限继续套用到新 Key；管理面随后使用新 Key 重新发现并保存模型。
- Provider Driver 负责结构化解析模型目录；Runtime 只在有界响应体和读取超时内收集数据，返回排序去重后的模型 ID，不返回或持久化原始正文。
- 用户通过专用模型写端点提交勾选集合。Credential 模型集合、内部 Route/Target 物化、全局 revision 和 PublishedSnapshot 在同一串行配置发布中完成。
- `ModelRoute` 与 `RouteTarget` 保留为数据面内部结构。公开模型名首版固定等于上游模型名；同一协议与模型聚合为一条 Route，同一 Endpoint 聚合为一个 tier-0 Target。
- 候选生成必须再次检查 Credential 是否选择了当前 `upstream_model`。相同 Endpoint 下未选择该模型的 Key 不得参与调度。
- 自动 Route/Target 使用由协议、模型和 Endpoint 派生的稳定 ID，避免无关配置发布改变会话目标身份。
- 普通 Web 导航移除独立“模型路由”页面；Provider API Key 编辑流程负责模型发现、选择和后续刷新。
- 旧手工 Route 在迁移时按 Endpoint 展开为各 Credential 的模型选择；下一次模型集合写入会规范化为自动物化结构。本项目不保留别名、手工 tier 与旧管理 API 的兼容承诺。

## 后果

- 最常见流程缩短为“填写 API Key -> 拉取模型 -> 勾选 -> 保存”。
- 模型权限与真实认证材料保持同一作用域，不会把一把 Key 的权限错误套用给另一把 Key。
- 首版暂不提供公开模型别名和手工主备 tier。后续若确有需求，应设计为独立高级策略，而不是重新暴露内部聚合对象。
- `/models` 目录解析成为 Provider 契约的一部分，必须覆盖畸形 JSON、重复 ID、超大正文、读取超时和 Secret 脱敏测试。
