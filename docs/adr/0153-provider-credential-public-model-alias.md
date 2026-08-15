# ADR-0153：凭据模型条目的可选公开别名

- 状态：Accepted
- 日期：2026-08-15
- 决策者：maintainer
- 修订：`AGENTS.md` §3 公开模型名边界、`ARCHITECTURE.md` §9.4 凭据模型集合、§9.5 内部
  ModelRoute、§11.2 Images 命名例外、§11.4 公开模型命名

## 问题

部分 API Key 中转上游对同一底层模型使用自有名称：例如 `gpt-5.6-sol` 在某中转被命名为
`gpt-5.6-sol-ganen`。现状公开模型名固定等于上游模型名（恒等物化），因此该上游只能以
`gpt-5.6-sol-ganen` 发布公开模型；而客户端（如 Codex CLI）只请求标准模型名，自定义名称
无法触达，该上游也无法加入 `gpt-5.6-sol` 标准名下的负载均衡池。

全局映射不能解决该问题：命名是单个上游的属性，同一公开名在不同上游对应不同真实名；
出向全局改写会把错误名称发给使用标准名的其他上游，入向归一化则永远不会命中（客户端
根本不发送自定义名）。`route_targets.upstream_model` 在 Schema 与域模型中本就独立于
`public_model` 并贯穿候选选择与出站编码，唯一强制恒等的是 `from_credentials` 物化与
凭据模型管理面。

## 决策

1. `provider_credential_models` 条目新增可选 `public_model` 别名（迁移 0033）：
   `ALTER TABLE ADD COLUMN`，`CHECK` 约束 trim、1..=255 字符且不等于 `upstream_model`
   （等值别名在域层归一为空，DB 拒绝冗余表示），并新增唯一表达式索引
   `(credential_id, COALESCE(public_model, upstream_model))` 保证同一凭据别名后的
   公开名唯一。域类型 `ProviderCredentialModel { upstream_model, public_model? }` 取代
   裸 `UpstreamModelName` 列表，管理链路（DTO → Publisher → ConfigCommand →
   ConfigurationMutation → Storage）全程携带该类型，不保留字符串数组双轨。
2. Route 物化 `ModelRouteConfiguration::from_credentials` 改为按
   `(ingress_protocol, 有效公开名)` 分组；Target 的 `upstream_model` 使用条目的上游
   真实名。`derived_target_id` 的确定性身份串显式加入 `upstream_model`——恒等物化下该
   信息经由 route_id 隐式传递，别名使其独立变量化，必须进入身份串才能满足
   「模型或 Endpoint 变化时生成新 ID」的既有不变量（差异同步的 upsert 按身份字段守卫，
   同 ID 换模型会被拒绝）。
3. 物化期一致性校验：同一 Endpoint 内 `(公开名 → 上游名)` 与 `(上游名 → 公开名)` 必须
   双向唯一，跨凭据冲突拒绝发布并返回包含 Endpoint 名与两个冲突名称的可读错误。该约束
   保证同一 Endpoint 对同一公开模型的出站请求体与凭据选择无关（ADR-0149 请求面原则），
   也避免同一上游名以两个公开名重复发布。
4. OAuth 账号目录保持恒等命名：OAuth 候选按公开名并入同名 Route，上游名即公开名。
   响应侧既有的已知 `model` 字段回写机制（JSON 与 SSE 拼接改写）按入口公开名工作，
   别名生效后自动恢复公开名，协议层零改动。Images 方言同样适用统一别名规则，撤销原
   「图片不做别名」的措辞——机制统一后维持例外反而需要专门校验代码。
5. 管理面：`PUT /api/admin/provider-credentials/{id}/models` 的 `models` 从字符串数组
   改为 `{upstream_model, public_model?}` 条目数组，凭据列表 DTO 同步；Web 在
   「选择上游模型」对话框为勾选条目提供可选「公开名称」输入，默认留空表示与上游一致。

## 边界与非目标

- 不支持 wildcard、前缀规则、别名链或 Route 到 Route 的引用；别名仍是精确、区分大小写
  的单层映射。
- 不提供手工 Route/Target/tier 编辑，不暴露调度内部结构。
- OAuth 账号目录不提供别名；固定目录来自 Provider Driver。
- `models.allowed`、请求日志、用量统计与会话粘性语义不变，均继续以公开名/既有身份字段
  为准。

## 后果

- 自定义命名上游加入标准公开名的候选池：客户端零改动，`/v1/models` 只列公开名，
  自定义上游名不再作为公开模型出现。
- 一次性迁移代价：target id 身份串变化，升级后首次触发物化的配置变更会以新 ID 重建全部
  `route_targets`；`request_attempts.route_target_id` 经由既有 `ON DELETE SET NULL` 与
  历史遥测解耦，会话粘性为内存态不受影响。
- `domain` 新增条目类型与两类冲突校验；`storage` 扩展行读写与迁移；`runtime` 仅投影
  上游名（调度、健康、RPM、粘性零改动）；`protocol`、`transport`、`provider` 零改动；
  `server` 与 Web 更新模型条目 DTO 与别名编辑。
