# ADR-0172: Provider descriptor、facet 与协议 target profile

- 状态：Accepted
- 日期：2026-08-22
- 当前事实：[Provider、协议与桥接](../architecture/protocol-bridges.md)
- 替代：[ADR-0170](0170-current-decision-register.md) 的 Provider/协议部分
- 被替代：无

## 背景

多个 Provider 可以使用相似的 Responses、Chat、Images 或 Messages wire protocol，但认证、OAuth、端点、
错误和目标字段能力并不相同。按 Provider 复制整套 Bridge 会快速分叉；使用一个宽松兼容类别或按 URL/模型名
猜测又无法给出可靠语义。

## 决策

Provider 通过 `ProviderDescriptor` 声明结构化能力，通过独立 facet 提供实际可选行为；Registry 验证两者一致。
Protocol 拥有共享 Bridge，Provider 只选择 Protocol 定义的 `ProtocolTargetProfile` 来表达目标方言的正交差异。

Target profile 是随 Driver 版本发布的静态契约，不进入用户配置，也不自动学习。新增差异先证明它属于协议目标
能力，再扩展最小 Profile 字段；不创建 Provider 专用 Bridge 或中央 Provider 分支。

## 备选方案

- 一个 `OpenAICompatible` Provider：隐藏认证与行为差异，并把失败推迟到运行时。
- 每个 Provider 一套 Responses → Chat 转换：重复状态机，unary/SSE 容易漂移。
- 把 Profile 存入数据库或由 URL 自动生成：让管理员配置承担实现版本契约，并产生不可审计猜测。
- Driver 暴露一个包含所有 OAuth 方法的巨型 trait：迫使不支持的 Provider 提供空实现。

## 后果

协议转换只有一份，Provider 能力可被 Runtime、管理 API 和测试共同枚举。Provider crate需要依赖稳定的
`protocol::api` Profile 类型，但仍不得执行 Bridge。新增 Profile 字段需要协议契约测试和实际供应商证据。

## 验证

Registry 注册一致性测试、实际实现枚举、Responses → Chat unary/SSE 映射测试，以及最终 loopback wire 契约。
