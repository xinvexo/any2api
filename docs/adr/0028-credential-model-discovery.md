# ADR-0028: Credential 驱动的模型发现与选择

> 状态：Accepted
> 日期：2026-07-23
> 修订：2026-08-03
> 决策者：maintainer

## 背景

管理面不要求用户理解 `ModelRoute`、`RouteTarget`、`fallback tier`、入口协议和上游模型之间的内部关系。用户提供 API Key 和 Provider URL 后，通过该 Credential 的实际模型目录选择，或手工填写已确认的精确模型名。

同一 Endpoint 下不同 API Key 可能拥有不同模型权限，因此模型集合不能只保存在 Endpoint，也不能仅根据某一把 Key 的 `/models` 结果推断其他 Credential。

## 决策

- 每把 `ProviderCredential` 独立保存用户确认的上游模型集合，SQLite 使用 `provider_credential_models(credential_id, upstream_model)`。
- 新建 Credential 初始不发布任何模型。管理面保存 API Key 后，使用该 Credential 当前 generation、实际代理与 Endpoint 请求 `GET /models`。
- 轮换 API Key 时原子清空该 Credential 在轮换前的模型集合与内部路由，避免把被替换 Key 的权限继续套用到新 Key；管理面随后使用新 Key 重新发现并保存模型。
- Provider Driver 负责结构化解析模型目录；Runtime 只在有界响应体和读取超时内收集数据，返回排序去重后的模型 ID，不返回或持久化原始正文。
- `/models` 只是便利的候选目录，不是 Credential 模型集合的权威边界。上游未实现该端点、
  返回空目录或返回无法解析的兼容格式时，管理员可手工添加其已确认可调用的精确上游模型名。
- 用户通过专用模型写端点提交最终确认集合，其中可同时包含目录勾选与手工输入。所有名称
  统一通过 `UpstreamModelName` 校验；不校验其是否出现在最近一次发现结果中，也不持久化名称来源。
  Credential 模型集合、内部 Route/Target 物化、全局 revision 和 PublishedSnapshot 在同一串行配置发布中完成。
- `ModelRoute` 与 `RouteTarget` 保留为数据面内部结构。公开模型名首版固定等于上游模型名；同一协议与模型聚合为一条 Route，同一 Endpoint 聚合为一个 tier-0 Target。
- 候选生成必须再次检查 Credential 是否选择了当前 `upstream_model`。相同 Endpoint 下未选择该模型的 Key 不得参与调度。
- 自动 Route/Target 使用由协议、模型和 Endpoint 派生的稳定 ID，避免无关配置发布改变会话目标身份。
- Route/Target 物化使用同一配置事务内的差异同步：未变化的行保持原位，只插入新增项、更新同一稳定身份的可变字段，并删除候选配置中真正消失的项。不得先清空整张物化表再按相同 ID 插回，因为 `request_attempts.route_target_id ON DELETE SET NULL` 会不可逆地擦除仍有效的历史关联。
- API Key 轮换继续清空该 Credential 的模型集合；差异同步只保留仍由其他 Credential 提供的 Target。不能把“路由 ID 不含 Secret”误解为模型集合不受轮换影响。
- 普通 Web 导航移除独立“模型路由”页面；Provider API Key 编辑流程负责模型发现、手工添加、选择和后续刷新。
  发现正在进行或失败只影响目录状态与“重新拉取”，不禁用手工添加、已选列表编辑或保存。
- Web 只把未保存的发现结果绑定到当前 Endpoint、当前 Credential 和按 DIRECT 继承规则解析出的实际代理版本。全局配置 revision 及无关资源发布不属于该 scope；相关资源版本变化时隐藏旧结果。
- 同一编辑器的探测请求使用单调序号。只有 scope 和序号均仍匹配的最新请求可以结算结果、错误与 loading 状态，迟到请求不得覆盖较新的目录。

## 后果

- 最常见流程缩短为“填写 API Key -> 拉取模型 -> 勾选 -> 保存”；未提供模型目录的兼容上游可走“填写 API Key -> 手工添加模型 -> 保存”。
- 模型权限与真实认证材料保持同一作用域，不会把一把 Key 的权限错误套用给另一把 Key。
- 公开模型名固定等于上游模型名，不提供别名编辑或手工主备 tier。
- 手工添加是管理员对上游能力的显式声明，不证明模型实际可用；后续真实上游错误仍按数据面规则处理。
- `/models` 目录解析成为 Provider 契约的一部分，必须覆盖畸形 JSON、重复 ID、超大正文、读取超时和 Secret 脱敏测试。
- 模型增删、Endpoint 协议变化与 Secret 轮换不再重写无关 Route/Target；历史 Attempt 只在其实际 Target 失效时失去外键关联。
- 无关配置发布不会清空正在使用的模型发现结果；相关 Endpoint、Credential 或实际代理发生变化时不会继续展示旧目录。

## 验证

- Storage 回归必须同时证明：新增模型不改变已有 Target 的 SQLite 行或历史 Attempt 外键；删除模型只清理对应失效 Target；Secret 轮换仍清空该 Credential 的旧模型权限。
- Web 回归必须覆盖全局 revision 变化但相关资源版本不变时结果仍可见，以及同一 scope 的后发请求先完成时迟到旧请求不能覆盖。
