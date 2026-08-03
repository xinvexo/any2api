# ADR-0096：转发头缺失可控降级、非法值严格拒绝

- 状态：Accepted
- 日期：2026-08-03
- 决策人：项目维护者
- 修订：ADR-0014、ADR-0050、ADR-0072

## 背景

`network.trusted_proxy_cidrs` 命中的 TCP 对端原先必须同时提供“恰好一行”`X-Forwarded-For`（XFF）与 `X-Forwarded-Proto`（XFP），任一缺失、重复或非法都会让管理面和 `/v1` 全部返回 400。反向代理仅漏配 XFP 是常见且可安全降级的部署错误，却会造成整个实例不可用。

另一方面，简单接受任意重复值也不正确。XFF 是有顺序的地址列表，多行字段在代理追加场景中共同构成一条链；只取最后一行会丢弃更早的真实客户端与代理跳。XFP 的多值语义则无法在当前模型中可靠证明哪个协议属于外部客户端，错误地选择 `https` 会设置 `Secure` Cookie 并隐藏明文传输警告。

直接 loopback 权限已经由 ADR-0088 固定为“规范化 TCP 对端是 loopback 且请求未进入 trusted-proxy 解析”。因此在可信代理缺少来源头时回退到 TCP 对端不会扩大 Setup 或 `admin.remote_enabled=false` 的权限边界。

## 决策

1. 只有规范化 TCP 对端命中当前 PublishedSnapshot 的可信代理 CIDR 时才读取 XFF/XFP；未命中时仍完全忽略客户端提供的转发头。
2. 可信代理缺少 XFF 时，客户端地址降级为规范化 TCP 对端。连接仍标记为 `through_trusted_proxy=true`，绝不能因此获得直接 loopback 权限。RequestLog/HttpAccessLog 与登录限流会使用代理地址，明确接受可观测性降低和共享限流桶这一运维代价。
3. XFF 出现时，把所有物理字段行按收到的顺序视为一个逻辑逗号列表；每行仍可包含多个逗号分隔地址。全部元素必须非空、可解析并先 `to_canonical()`，随后从 TCP 对端开始按完整链从右向左剥离连续可信代理。不得只取第一行、最后一行或跳过非法元素。
4. 可信代理缺少 XFP 时降级为 `secure=false`。这会保留明文传输警告且不会给会话 Cookie 增加 `Secure`，比无证据宣称 HTTPS 更保守，同时不阻断 API 数据面。
5. XFP 出现时必须恰好一个物理字段值，且精确为 `http` 或 `https`。重复行、逗号多值、空值、非 UTF-8 或其他协议全部拒绝；当前实现不猜测 leftmost/rightmost 协议。
6. 已出现但非法的 XFF/XFP 在管理面和公开面继续返回现有稳定的 invalid-forwarded-headers 400。缺失与非法不能混为一类：只有“完全缺失”允许上述降级，空字段属于非法。
7. 每个请求由最外层入口捕获一次 PublishedSnapshot，并只调用一次可信代理解析器。该快照、规范化 TCP peer 与完整 `Result<ClientConnection, ClientAddressError>` 共同组成不可变请求扩展；管理鉴权、公开鉴权、登录/Setup、RequestLog 与 HttpAccessLog 都必须复用它，不按路由重新解析 Header、加载新快照或引入两套宽严策略。最外层 HttpAccessLog 在解析失败时仍可用同一上下文中的规范 TCP peer 记录这次本地 400；管理与公开鉴权必须复用同一个错误 Fail-Closed，禁止把日志 fallback 当成成功来源，也不把原始转发头写入 RequestLog。
8. 本决策不自动信任 `Forwarded`、`CF-Connecting-IP` 或其他供应商头，也不改变可信 CIDR 配置、直接 loopback 权限或远程管理开关。

## 后果

- Nginx/Caddy 漏配 XFP 或 XFF 不再让整个网关下线；缺失 XFP 会显式表现为非安全连接。
- 合法的多行 XFF 追加链可以准确解析，客户端预置字段不会因“只取最后一行”而静默丢失链路证据。
- 畸形地址与有歧义的 XFP 仍 fail-closed，不会伪造来源、Secure Cookie 或安全提示。
- 缺失 XFF 时多个客户端共享代理 IP 的登录失败窗口；管理员应补齐代理配置来恢复准确来源，而不是由 any2api 猜测地址。

## 验证

- Server 单元测试覆盖 XFF/XFP 同时缺失、分别缺失、多行 XFF 完整合并、伪造左侧地址、空值/非法 IP、重复和非法 XFP；请求上下文回归固定同一捕获快照以及成功/失败结果同时供日志与鉴权读取。
- 管理 HTTP 契约验证可信代理缺少头时仍可访问但保持 `through_trusted_proxy` 与明文警告，多行 XFF + 单值 HTTPS 正确签发 Secure Cookie，非法 XFP 仍返回 400。
- 公开请求继续复用同一请求扩展，既有 RequestLog 契约验证可信链地址持久化和非法头 400；源码门禁确认生产请求路径只有最外层上下文构造点调用解析器。
