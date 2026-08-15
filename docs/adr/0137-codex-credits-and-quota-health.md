# ADR-0137：Codex Credits 字段与额度健康

- 状态：Accepted
- 日期：2026-08-11
- 决策者：maintainer
- 相关决策：ADR-0070、ADR-0111、ADR-0145、ADR-0146

## 背景

Codex `/backend-api/wham/usage` 同时返回 rolling rate-limit、工作区 spend control
和购买 Credits。三类信息的单位和路由含义不同：Credits 不是 rate-limit reset credit，
百分比也不是货币余额。管理面需要保留上游的真实 Credits 字段，同时只把本机观测的
美元等值作为展示信息，不能把它变成计费、余额或调度依据。

## 决策

1. Codex Driver 从同一次有界只读 usage 响应解析并校验 `credits.has_credits`、
   `credits.unlimited`、可选非负十进制 `credits.balance`、`spend_control.reached`
   和声明的 `rate_limit_reached_type`。缺失或畸形字段保持未知，不用默认值猜测。
2. 管理 API 原样返回校验后的 Credits 状态和十进制余额。Web 只显示 `Credits` 的
   无限、有限、隐藏但可用或不可用状态；有限值可按当前版本化费率卡换算为标准费率卡
   美元等值，并明确标注为展示换算，不是现金余额。费率配置和 Token 计价由 ADR-0145
   定义，累计本机容量由 ADR-0146 定义。
3. 路由健康按权威字段组合：
   - `spend_control.reached=true`，以及 workspace owner/member credits depleted 或
     usage limit reached，是工作区硬停止，建立临时额度耗尽健康；
   - 普通 `allowed=false`、`limit_reached=true` 或 `rate_limit_reached` 只代表
     included rolling window 不可用；同一快照报告 `credits.unlimited=true` 或
     `credits.has_credits=true` 时，不得据此排除账号，并清除此前仅由 rolling window
     建立的耗尽状态；
   - 百分比、美元等值和本机容量统计不参与路由、RPM、会话、重试或 Gateway Key 权限。
     冲突快照中工作区硬停止优先于 Credits 可用标记。
4. OAuth 数据面继续通过统一活动 Guard 触发已有额度刷新；Guard 不维护第二份 Token
   成本状态。RequestLog 的冻结计价和本机累计统计分别遵守 ADR-0145 与 ADR-0146。
5. Credits 与 rate-limit reset credit 在 API、存储和 Web 中保持独立字段，禁止合并、
   互相换算或用其中一个推断另一个。Quota refresh 失败时保留最近一次安全快照，不以
   失败响应覆盖有效 Credits。

## 边界

- 上游 Credits 是管理员可见的额度字段，但不代表 any2api 的余额、计费或多租户配额。
- 本机美元等值只用于管理展示；RequestLog 被关闭、清理、缺少 usage 或模型费率未知时，
  统计可以保持未知或低估，不能伪造精确余额。
- 未知、自然语言或未经 Provider 契约确认的错误字段不能建立账号限制或出口限制健康。

## 验证

- Provider 测试覆盖 Credits 无限/有限/零/隐藏余额、spend control、reached type、畸形
  数字和未知字段。
- Runtime/HTTP 测试覆盖 Credits 可用覆盖 rolling exhaustion、workspace hard stop 优先、
  百分比和美元等值中性，以及刷新失败保留上一份快照。
- Web 测试覆盖原始 Credits 展示、费率卡换算标识和不把展示值当作余额或权限。
