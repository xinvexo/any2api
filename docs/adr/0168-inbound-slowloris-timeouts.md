# ADR-0168：入站请求头与请求体空闲超时

- 状态：Accepted
- 日期：2026-08-18
- 修订：2026-08-18
- 决策者：maintainer

## 背景

入站连接数上限只能限制已接受的 TCP 连接数量，不能限制连接在 HTTP 请求头阶段或请求体阶段的停留时间。
客户端如果持续发送少量 Header 字节，或在两个 Body 数据块之间无限等待，就能长期占用连接许可、请求生命周期和访问日志
Body 捕获资源；这会拖慢正常请求，也会让优雅停机等待无界。

## 决策

- 新增 `network.request_header_timeout`，默认 30 秒，启动时装配。首个 HTTP 请求在握手、协议识别或请求头完整解析前必须在预算内建立；HTTP/1 后续 keep-alive 请求使用 Hyper 的 Header 读取超时。超时发生在 Axum Request 形成前，直接关闭连接，不生成协议错误正文。
- 新增 `network.request_body_idle_timeout`，默认 60 秒，启动时装配。公共接口、管理 JSON、multipart 上传和未知路由统一包装请求 Body；每次收到一个数据帧都会重置计时，连续空闲超过预算则返回 typed Body timeout error 并停止消费，禁止进入 Handler 或上游请求。它是 idle timeout，不是整个上传的 wall-clock deadline。
- 两个设置都属于 `restart_required`。SQLite 只保存覆盖值，当前监听器、HTTP 连接构造器和 Body middleware 在进程启动时捕获同一份 `SettingsConfiguration`；热更新不会悄悄改变已建立连接的超时语义。
- `network.max_connections` 仍只统计存活 TCP 连接；内核 backlog、HTTP/2 多路复用和请求级生命周期不复用该计数。请求取消、客户端断开和 Forced shutdown 必须通过 Body Drop/连接任务收尾释放所有资源。

## 后果

慢速或失联客户端会在有界时间内释放入站连接许可，管理员不需要依赖反向代理才能获得基本 slowloris 防护。非常慢但合法的上传必须在每个数据帧之间持续发送，部署者可通过设置覆盖值调大预算并重启。HTTP/2 后续 stream 的协议级 HEADERS 由 h2/Hyper 管理；首个请求握手预算和统一 Body idle wrapper 仍覆盖最容易造成连接长期占用的阶段。

## 验证

- App server 测试使用原始 TCP 覆盖无字节、分段 HTTP/1 Header 和慢速首个请求，在 Header deadline 后连接关闭；正常请求与已有优雅停机测试继续通过。
- Server middleware 测试覆盖空闲 Body 超时、每个数据帧重置计时，以及公共、管理 JSON 和 multipart 路径共享同一 wrapper。
- Settings Domain/管理契约验证两个键的默认值、范围、`restart_required` 元数据和覆盖编译。
