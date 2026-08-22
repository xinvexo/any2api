# 存储、配置与安全

本文是 SQLite 生命周期、配置发布、管理员认证、Secret 和持久化遥测的当前规范。部署操作见
[operations.md](operations.md)，用户可执行的备份和远程访问说明见 [README](../../README.md)。

## SQLite 所有权

一个数据目录对应一个 SQLite 数据库和一个进程实例锁。SQLite 保存：

- 代理、Provider Endpoint、Provider API Key、OAuthAccount、Gateway API Key 和设置覆盖；
- OAuth 模型目录与额度的必要快照；
- Provider 官方客户端版本的最后一次成功快照；
- 有界 RequestLog、RequestAttempt 和 HTTP 系统日志；
- 管理员认证所需的当前持久化材料。

Runtime 的队列、RPM、健康、冷却、粘性、请求进度、事件 epoch 和后台任务状态不持久化。Repository 只暴露
当前领域模型需要的读写能力，业务编排不进入 SQL 层。

## Migration

Migration 是连续编号、只追加、不可改写的前向链，`migrations/checksums.toml` 固定已发布脚本。新进程启动时
在对外监听前把已有数据库升级到当前 Schema；新数据库也运行同一完整链。

生产 Rust 和 TypeScript 只处理当前 Schema。旧格式转换必须在下一条 SQL Migration 或受支持的外部导入边界
完成，不在 Repository 中长期保留双读、旧字段别名或启动期猜测。Migration 测试至少覆盖全新数据库、完整升级
链和本次变更相关的代表性旧状态。

## 配置发布

所有影响鉴权、路由、代理或设置的写入都经过 `ConfigPublisher`：

1. 以期望 `ConfigRevision` 获取串行发布权并验证命令；
2. 在 SQLite 事务中应用 mutation，加载并编译完整候选配置；
3. 候选失败则回滚；提交结果不确定时进程不能继续假装成功；
4. 提交成功后一次性替换 `PublishedSnapshot`，再更新 route admission、依赖配置的进程组件和 scheduler epoch；
5. 管理 API 只在上述步骤完成后返回新 revision。

无变化的 mutation 不提升 revision。普通过期 revision 返回冲突；只有明确标记的自动 OAuth 发布允许安全
rebase。读路径通过快照观察完整 revision，不组合多个 Repository 查询形成临时配置。

## Secret 边界

本地自托管边界允许必要 Provider API Key、OAuth token document 和代理密码以明文存在 SQLite；数据目录权限
和主机访问控制就是保护边界。Gateway token 的具体持久化形式以领域/存储实现为准。

任何 Secret 只在最窄使用点解封，并不得进入：

- tracing、JSONL 文件日志、RequestLog 或 HTTP 系统日志；
- 错误正文、`Debug`、panic 信息和普通管理响应；
- 浏览器 `localStorage`、session storage、URL 或可下载导出；
- 测试 fixture、snapshot、官方客户端 baseline 或生成 DTO。

OAuth JSON 只能通过受支持 Provider 的显式导入边界解析为独立 `OAuthAccount`，不进入普通配置导出，也没有
读取其原始 token document 的管理端点。稳定主体和精确 token 冲突检查在发布锁内完成，防止同一上游身份被
重复建立为多条路由凭据。

## OAuth 目录和额度快照

模型目录按 Provider routing facet 给出的 scope 有界存储，额度按 OAuthAccount 存储带 schema version 的有界
快照。它们是管理员查询与编辑辅助，不进入请求恢复；目录刷新也不自动修改已发布模型选择。

只有 Provider 明确给出成本单位时才计算本地额度估计。估计器在官方周期边界内，按刷新 observation fence
汇总该账号持久化 RequestLog 的规范成本；官方使用率未达到可信门槛时不外推总容量。窗口耗尽且购买 Credits
开始接管后，included-window 值冻结在最后可证明的容量或接管 fence，后续付费消耗不再抬高它。

额度 identity 优先使用 Provider 导出的稳定主体，使 token 刷新或代理切换不会伪造新容量；没有稳定主体时
保守回落到账号与 token generation。订阅身份改变或证据不连续会阻止容量外推。费率卡来自当前 Setting
Registry，Fast 成本只在上游最终响应确认 effective speed tier 时使用。设计理由见
[ADR-0178](../adr/0178-evidence-bound-oauth-quota-estimates.md)。

## 管理员认证与客户端地址

系统只有一个管理员安全域。首次 setup token 只允许 loopback；远程初始化使用显式环境变量。管理员密码以
内存困难哈希保存，轮换使旧会话失效。Gateway API Key 只鉴权公开 `/v1`，不能访问管理 API。

监听地址与 TLS 终止由部署者控制。HTTP 可以运行，但远程管理应放在 TLS 反向代理后。只有管理员配置的可信
代理地址可以影响规范客户端 IP 和 secure-request 判断；不可信 peer 的转发 Header 被忽略，可信代理提供的
歧义值 fail closed。详细部署示例属于 README。

## 持久化遥测

### RequestLog 与 Attempt

RequestLog 保存一条客户端逻辑模型请求的最终结果和规范 usage；RequestAttempt 保存每次实际上游尝试及其
凭据归属。它们服务本地审计和趋势，不形成计费、余额、路由恢复或请求回放系统。写入使用有界队列；容量不足
时丢弃遥测并计数，不能反向阻塞公开请求。

### HTTP 系统日志

HTTP 系统日志永久为 metadata-only。当前记录只包含 request ID、开始时间、配置 revision、规范客户端 IP、
method、path、HTTP 版本、status、duration、response bytes 和 completion outcome。

系统绝不采集或持久化 query、请求 Header、响应 Header、请求 Body 或响应 Body，也不向管理详情返回这些
内容。Migration `0043_remove_raw_http_access_capture.sql` 重建日志和容量表，只复制安全 metadata，物理移除
历史 URI、Header、Body 与交换字节列，并删除 `logs.http_access.max_exchange_bytes` 设置覆盖。

默认过滤本机成功的普通管理/Web 内部流量；公开请求、未知或远程访问、HTTP 错误、Body 错误与取消保留审计
价值。列表使用有界保留与 Keyset Cursor，不执行随机 OFFSET 翻页。

### 实时事件

管理员 SSE 只发送最新运行快照和不含正文的失效通知。SQLite 才是历史列表事实来源；事件 epoch 用于断线后
触发追赶，不提供事件重放。日志清理、保留和容量删除通过 Writer 的有序路径推进相应失效 epoch。

## 浏览器与错误安全

浏览器只持有短期编辑草稿、服务端查询缓存和非敏感显示偏好。管理 DTO 默认使用 Secret-free 投影；任何需要
一次性展示的凭据必须由明确创建/轮换响应拥有，不能通过列表或详情重新读取。

内部错误分类可以影响重试和健康，但返回客户端时不能携带上游凭据或本地存储细节。非成功上游响应的透明性
仍受 Header 安全过滤和 Body 上限约束。
