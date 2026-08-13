# ADR-0148：基于 prompt_cache_key 的缓存局部性软路由

- 状态：Accepted
- 日期：2026-08-13
- 决策者：maintainer
- 影响范围：Protocol Decode、RuntimeRegistry、未绑定候选选择、Attempt 成功与失败生命周期
- 补充：ADR-0062、ADR-0064 的会话粘性语义；第 12 章的稳定轮询与 RPM 原子预留

## 问题

关闭 `affinity.enabled` 后，携带相同 `prompt_cache_key` 的 Codex Responses 请求会按普通稳定轮询进入不同
Route Target、ProviderCredential 或 OAuthAccount。实际观测表明，同一缓存键和同一正文前缀切换凭据后可能
从高缓存命中直接变为零命中；发生 403 后虽然健康冷却能暂时避开失败路径，冷却到期仍会再次选到该路径。

冷却解决的是失败抑制，不保存上游缓存所在路径；强制开启会话粘性又会把 Credential 不可用变成绑定错误，
并违背管理员关闭普通粘性的选择。需要一个独立、可回退的缓存局部性机制。

## 决策

1. Protocol Decode 对 OpenAI Responses 与 Chat Completions JSON 请求提取非空、有界的
   `prompt_cache_key`。该值是瞬态敏感路由材料，不加入 `Debug`、日志、DTO、SQLite 或浏览器状态。
2. 稳定 `RuntimeRegistry` 持有进程级随机 HMAC-SHA256 键和有界内存表。映射键的作用域包含协议方言、
   操作、`ModelRouteId` 与原始 `prompt_cache_key`；表中只保存不可逆摘要和上次成功的完整候选身份。
3. 映射最多保存 16,384 条，采用 30 分钟滑动过期和最近访问淘汰。活跃键每次命中延长有效期；进程重启
   清空全部记录，不实现恢复、导出或跨节点同步。
4. 未绑定选择命中映射时，在普通 fallback tier/轮询前对目标做一次无等待尝试。该尝试复用现有健康检查、
   exclusions 与 RPM 原子预留；成功不推进普通轮询游标。目标不可用、RPM 已满或已被本次重试排除时，
   立即执行原有完整调度，不等待提示目标，也不排除任何其他候选。
5. 缓存提示可以跨 fallback tier 优先上次成功路径。这是必要行为：否则上层候选冷却结束后仍会覆盖已经形成
   缓存的后备路径。该优先级只影响具有同一显式缓存键的请求，其他请求继续遵守原 tier 顺序。
6. buffered Attempt 只在响应完整解码、续接状态与绑定提交全部成功后记录目标；流式 Attempt 只在成功终止
   后记录。客户端取消不建立新提示，也不删除已有提示，因为取消不能证明上游路径失效。
7. 带候选身份的传输、协议或预提交失败，以及认证、权限、额度、限流、模型/操作不可用和瞬态上游失败，
   会比较并删除仍指向该候选的提示。普通 InvalidRequest、any2api 本地后处理失败与客户端取消不删除提示，
   因为它们不能证明路径失效。比较删除避免旧失败并发删除后来已由另一成功请求更新的映射。配置代际变化后，
   无法匹配当前完整候选身份的记录在读取时失效。

## 边界

- 这不是 Session Affinity：不创建 Binding、不等待固定 Credential、不返回
  `session_binding_lost`，也不改变 `affinity.enabled`。
- 这不是缓存命中承诺。上游仍可独立淘汰缓存；本机制只避免 any2api 在已经观察到成功路径后主动打散它。
- 没有 `prompt_cache_key` 的请求保持原稳定轮询。Continuation 始终使用已有强绑定，不读取软提示。
- 不新增设置、数据库 Schema、管理 API 或持久化运行态。容量和过期常量集中在单一 Runtime 模块。

## 验证

- Protocol 测试覆盖合法键提取、非字符串/空值不参与局部性以及 Debug 不泄露原始值。
- Runtime 单元测试覆盖同作用域命中、作用域隔离、滑动过期、比较删除和有界淘汰。
- 候选选择测试覆盖提示目标优先、提示目标不可用立即回到普通调度、exclusion 生效和普通轮询游标不被推进。
- Completion 生命周期测试覆盖只有最终成功才写入、候选失败比较删除、取消或未结算不改写已有提示。
