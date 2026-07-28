# ADR-0061: 最终上游错误响应透明返回

- 状态：Accepted
- 日期：2026-07-28
- 决策者：maintainer

## 背景

上游 HTTP 状态、正文和允许的响应 Header 是 Provider 的公开错误契约。any2api 的内部分类只服务
重试、OAuth 刷新、健康、冷却和熔断，不能形成第二套客户端错误协议。只有 any2api 自己产生的失败
才使用本地 `PublicError` 与协议兼容消息。

## 决策

- 只有 any2api 自己产生的错误使用 `PublicError` 与入口 `ProtocolAdapter::error_response`。其中包括
  网关鉴权、请求解码、模型/路由、队列、Transport、超时和内部编码失败。
- 真正收到的最终上游非 2xx 绕过 `PublicError` 与协议错误适配器。Runtime 原样返回上游状态码和在
  64 KiB 安全上限内完整收集的正文，只合并 Provider 白名单与通用安全清理后的响应 Header。
- Runtime 不根据分类或入口协议重建、补充或替换上游 `type`、`code`、`message`；该规则同样适用于
  跨协议桥。只有实际结束请求的最终 Attempt 可以返回，被重试或切换掉的响应全部丢弃。
- 错误正文若超限、读取超时或中途断开，仍保留已收到的上游状态与安全 Header，但正文为空；不得
  用固定摘要或新的本地 envelope 冒充上游内容。
- 完整原始正文同时保留 Provider 白名单内的 `Content-Type` 和 `Content-Encoding`，使压缩错误正文
  仍可由客户端按上游语义解码；正文被丢弃为空时同步删除 `Content-Encoding`，禁止留下失配元数据。
- `ProviderDriver::classify_error` 继续产生不可见的机器分类，并可从 Provider 已声明 envelope 提取
  原始 `message`。机器分类只参与运行时决策；原始 message 只进入有界 RequestLog/RequestAttempt
  管理展示，不从任意 JSON 字段猜测，也不把完整正文写入 SQLite。
- 管理请求日志 DTO 与 Web 不再暴露 `error_class`、`retry_safety` 和 Attempt `outcome`。页面按 HTTP
  状态、是否收到上游状态、原始 message、耗时与路由来源展示；message 缺失时保持为空，不生成上游
  摘要。
- 本地预算到期使用 504 和明确的 any2api 本地消息。取消、Transport 失败和本地超时必须在 Attempt
  生命周期中区分，不能因 Future Drop 把超时记录成普通客户端取消。

## 后果

官方客户端和普通 SDK 能看到 Provider 实际返回的状态与错误结构，自定义兼容 Endpoint 的错误字段也
不会被 any2api 丢弃。管理 Web 不再出现与状态码重复、且可能误导来源的分类标签。

内部机器分类仍然保留，因此重试和健康行为不依赖自然语言，但它只能存在于内部决策边界。错误正文
仍有 64 KiB 内存安全上限，超限或不完整时只返回真实状态，不伪造正文。

## 验证

- Runtime 契约覆盖 401、404、429、500 和未知状态，断言状态、正文与允许 Header 原样返回。
- 覆盖 JSON、纯文本、跨协议入口，以及正文超限/超时/中断时不生成替代 envelope。
- 重试契约确认中间 Attempt 正文不会覆盖最终 Attempt。
- 超时契约确认最终为本地 504，Attempt 不再记录为 cancelled。
- 管理 HTTP 与 React 契约确认不再包含/显示内部分类，并显示 Provider 原始 message。
