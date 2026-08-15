# ADR-0158：手动 OAuth 额度刷新附带上游模型目录

- Status: Accepted
- Date: 2026-08-16

## Context

OAuthAccount 的固定 Provider Profile 只需要数据面基址和协议方言；模型目录必须来自账号可见的上游目录。Provider 会在订阅账号可见范围内新增、下线或重命名模型，因此不能把模型名作为代码配置。每次打开模型编辑器都向上游请求又会把一个普通界面浏览动作变成不可预期的控制面流量。

OAuth 已有显式的手动额度刷新动作。该动作本来就使用当前 Token、账号指定的 OAuth 出口、严格 SSRF、读取超时和 Provider 级控制面节流，适合作为管理员主动同步目录的明确时机。按数据面活跃度触发的自动额度刷新频率最高为每账号 30 秒，不能借此增加模型目录请求。

账号不是目录缓存的正确粒度。例如，十个 Codex Free 与十个 Codex Plus 账号在一次“刷新全部额度”中不应产生二十个相同的 `/models` 请求。另一方面，不能仅按 Provider 盲目共享，因为 Provider 可能按套餐、工作区或其他安全的订阅属性返回不同目录。

## Decision

Provider Driver 从当前 OAuth Token 导出稳定、非敏感的 `directory_scope`。它不是账号 ID、邮箱、Token、代理 ID 或任意 JWT 原文；只能表达 Driver 已经证明会获得同一目录的 Provider 自有订阅分组。Codex 以规范化套餐分组：`free`、`plus_or_pro`、`team_or_business_or_go` 与未知计划的最小 `free` 回退。Claude 与 Grok 的当前 OAuth Profile 未暴露更细套餐目录身份时各自使用一个 Provider 订阅 scope；若将来发现目录依赖更细属性，Driver 必须先拆分 scope，不能在 Runtime 增加 Provider `match`。

`POST /api/admin/oauth/accounts/{id}/quota/refresh` 在成功读取额度后，使用该账号当前 Token 和同一账号出口读取其 `directory_scope` 的模型目录。`POST /api/admin/oauth/quota/refresh` 是“刷新全部额度”的服务端批量操作：它先处理每个账号的额度刷新，然后按 `Provider + directory_scope` 去重，为每一组选择一个成功刷新且账号 ID 最小的代表账号读取目录。因此十个 Free 与十个 Plus Codex 账号最多会产生两次目录读取。单账号与批量操作同时命中同一 scope 时使用进程内 singleflight，避免并发重复读取。

Provider Driver 拥有每个 Provider 的 scope、请求计划、官方身份 Header 和响应解析；Runtime 只负责当前快照、认证材料、代理、Transport、大小/超时边界、OAuth 控制面 pacer 和一次 401 后 Token 刷新重试。

模型目录与额度请求使用同一 OAuth 控制面隔离身份和起始时刻节流，但不经过公开路由、RPM 准入或数据面 `in_flight`。目录响应在固定大小限制内解析为排序去重的模型名，并将模型名和抓取时间写入独立 SQLite 快照，主键为 `Provider + directory_scope`；原始正文、上游 URL 和 Token 不返回给管理面，也不写日志或 SQLite。

手动刷新响应可选携带本次成功的模型目录。目录读取失败不会回滚或伪造已经成功的额度结果：响应仍成功返回额度，目录字段为空，Web 只从独立 SQLite 快照和账号已保存模型读取候选，绝不回退代码内目录。后台按活跃度自动刷新、读取已缓存额度、Token 定时刷新和普通模型抽屉打开均不得发起目录请求。

目录快照只是候选，不是配置事实：它不修改 `OAuthAccount.models`、不替换 `PublishedSnapshot`，也不自动移除不再出现的已选模型。管理员通过已有模型保存操作显式确认集合；只有已确认模型进入 OAuth 路由投影，因此最新目录中经确认的模型可以参与路由。手工模型输入仍然可用。交互式登录必须先完成目录读取，并以该结果初始化显式选择；导入账号使用空选择，直到管理员手动刷新并保存。

## Alternatives

- 每次打开模型编辑器都读取上游：结果更即时，但普通浏览会产生难以预期的控制面请求，也难以与批量操作和账号出口统一节流。
- 在自动额度活动刷新中附带目录：会把高频的数据面活动放大成上游目录轮询。
- 每账号分别读取和存储目录：同一套餐的大量账号会形成无意义的请求放大。
- 仅在浏览器内存保留目录：页面重载后目录消失，管理员无法判断手动刷新是否真的同步成功。
- 持久化上游目录并将其直接发布到路由：会让上游暂态目录变化隐式改变公开路由；本决策只持久化候选快照，保持显式保存边界。

## Consequences

- 交互式登录先读取目录；导入账号或历史账号需先手动刷新额度，模型编辑器才会获得其当前目录身份的最新目录。该目录在进程重启或页面重载后仍可从 SQLite 快照读取；没有成功快照时只显示账号已保存模型。
- 单账号手动刷新最多增加一次有界上游请求；批量刷新按去重后的目录身份数增加请求，而不是按账号数增加。自动刷新频率不变。
- Provider 新增模型仍需管理员确认保存，避免上游目录波动自动扩大公开 API 面。
- 各 Provider 目录协议变化局限于 Driver 计划和解析器，不污染统一调度器。

## Verification

- Provider 单元测试固定每个 OAuth scope、目录请求的路径、认证/身份 Header 和合法/非法响应解析。
- Storage migration 测试覆盖带代表性历史 OAuth/额度数据的升级，并验证目录快照只保存安全字段。
- Runtime 测试证明单账号手动刷新同时请求额度与目录、自动活动刷新只请求额度、批量刷新按 scope 只请求一次目录、目录失败保留成功额度和历史目录、401 至多刷新 Token 并重试一次。
- 管理 API 与 Web 测试证明模型抽屉读取持久化目录、保存仍显式发布且未保存选择不会改变路由。
