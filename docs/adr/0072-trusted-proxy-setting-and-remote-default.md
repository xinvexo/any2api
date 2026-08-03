# ADR-0072: 可信代理进入 SettingRegistry，远程管理默认开启

- 状态：Accepted
- 日期：2026-07-30
- 决策者：maintainer

## 背景

可信反向代理此前只能通过启动环境变量 `ANY2API_TRUSTED_PROXY_CIDRS` 配置，管理员无法从系统设置
查看或修改生效值，修改后还必须重启进程。与此同时，`admin.remote_enabled` 的编译默认值为关闭；部署
已经通过同机 Nginx/Caddy 提供管理页面时，首次登录前还需额外进入服务器修改设置，形成不必要的启动
障碍。

可信代理是客户端来源解析的安全边界，不能因为进入 Web 设置就退化为直接相信任意转发头，也不能与
旧环境变量形成两个互相覆盖的真相来源。

## 决策

- `admin.remote_enabled` 的编译默认值改为 `true`。它只决定非 loopback 客户端能否访问管理员登录和
  管理 API，不打开端口，也不修改监听地址；`ANY2API_BIND` 的默认值仍为 `127.0.0.1:3210`。
- 设置项面向用户解释为：“允许其他设备打开管理页面并登录。关闭后，只有运行 any2api 的这台设备
  可以访问管理功能。”不在主说明中使用 loopback、API 或 bind 等实现术语。
- 新增热更新设置 `network.trusted_proxy_cidrs`，类型为 `string_list`，编译默认值为 `[]`。空列表表示
  未使用受信任反向代理：忽略所有客户端提供的转发头，直接使用 TCP 对端地址。
- 列表接受单个 IPv4/IPv6 地址或 CIDR。单个地址规范化为 `/32` 或 `/128`，CIDR 截断到网络地址，
  最终排序并去重。Web 在“基础 → 远程管理”直接提供逐行或逗号分隔的输入控件，并提示未使用
  Nginx、Caddy 等反向代理时留空。
- SQLite 覆盖值与代码默认值成为可信代理列表的唯一配置来源；删除启动环境变量
  `ANY2API_TRUSTED_PROXY_CIDRS`，不保留双轨兼容或优先级规则。
- 管理鉴权、Gateway Key 鉴权、RequestLog 与 HttpAccessLog 在每个请求开始时捕获一次
  PublishedSnapshot，并使用同一 revision 的可信代理列表和其他访问策略。管理设置提交只有在 SQLite
  Commit 与快照切换完成后才返回成功，新请求随后立即采用新列表。
- 只有 TCP 对端命中可信列表时才解析 `X-Forwarded-For` 和 `X-Forwarded-Proto`；从 TCP 对端开始按 XFF
  右到左剥离连续可信代理。XFF 的多行值按完整逻辑列表合并；XFF 缺失时回退 TCP 对端，XFP 缺失时
  按不安全 HTTP 处理。空值/非法 XFF 和重复/非法 XFP 继续 Fail-Closed，完整边界由 ADR-0096 修订。
- 远程访问默认开启不改变首次密码初始化：按照 ADR-0088，Setup API 只接受未进入 trusted-proxy
  解析的直接 loopback TCP 连接并要求一次性 Setup Token；解析后的 XFF loopback 不授予本机权限。
  远程首次部署使用 `ANY2API_ADMIN_PASSWORD` 初始化管理员密码。
- 本决策部分取代 ADR-0014 与 ADR-0050 中关于远程管理默认关闭以及可信代理只能由启动环境变量配置的
  内容，不改变其余认证、CSRF、Cookie、明文 HTTP 警告和日志地址语义。

## 后果

- 同机反向代理可以在默认 loopback 监听下直接展示登录页；管理员登录后可在 Web 完成可信代理配置，
  不再为该设置重启服务。
- 直接暴露非 loopback 监听地址仍是部署者的显式选择。远程管理端点受管理员密码、登录失败窗口、
  会话与 CSRF 保护；通过明文 HTTP 使用时继续显示持续风险警告。
- 可信代理配置错误会降低来源精度、共享登录限流桶，非法转发头仍会使当前代理连接被拒绝。Web 必须
  说明仅填写实际反向代理的地址或网段，不能把客户端网络误配为可信代理。
- 没有反向代理的部署无需配置该项，也不应为了“读取真实 IP”而信任任意公网网段。

## 验证

- Domain 测试覆盖远程管理默认开启，以及可信代理 IP/CIDR 的解析、规范化、排序、去重和非法值拒绝。
- 管理设置契约覆盖默认值、覆盖值、热更新 revision 与 `options=null` 的自由字符串列表响应。
- Server 单元测试继续覆盖直连忽略伪造头、可信多跳/多行链、缺失头的保守降级和非法转发头 Fail-Closed。
- 管理与公开 HTTP 契约通过 SettingRegistry 写入可信代理列表，验证保存后无需重启即可影响来源解析、
  HTTPS 判断和 RequestLog 客户端地址。
- Web 契约、草稿与组件测试覆盖自由字符串列表解析、基础页展示、友好说明和批量保存。
