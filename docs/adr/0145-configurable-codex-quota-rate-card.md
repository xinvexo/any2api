# ADR-0145：可配置的 Codex 额度费率卡

- 状态：Accepted
- 日期：2026-08-13
- 决策者：maintainer
- 影响范围：SettingRegistry、PublishedSnapshot、Codex OAuth RequestLog 计价、额度管理 DTO 与 Web 展示
- 修订：ADR-0137 的固定 Credits/USD 比例与 Provider 内模型费率；ADR-0144 的固定 UI 换算
- 后续修订：ADR-0146 删除容量 `confidence`、离散度和历史费率卡集合；本 ADR 继续定义当前费率配置、RequestLog 冻结计价和美元等值换算

## 问题

Codex 本地额度估算依赖两类会变化的数据：模型 Token 到 Codex Credits 的费率，以及 Credits 到美元的
展示汇率。当前两者分别硬编码在 Provider Rust 源码和 Web TypeScript 中。官方调整费率或新增模型时，
必须修改代码、重新构建并同时保持两处常量一致；前端也无法知道产生历史样本时使用的完整费率版本。

## 决策

1. SettingRegistry 新增 `oauth.codex.rate_card`，类型为受严格 Schema 校验的 `codex_rate_card`。编译默认值
   继续表达当前已知费率，SQLite 只保存用户覆盖值。Web 从通用系统设置中排除该项，通过主导航独立的
   “额度费率”页面和 `/quota-rates` deep link 编辑同一个覆盖值，不建立页面私有配置。
2. 费率卡包含非空版本 ID、正整数 `credits_per_usd`，以及按精确上游模型名索引的标准/快速档费率。
   每个档位直接配置每百万 Token 的输入、缓存输入和输出 nano-Credits；不按模型名称相似度回退。任何
   费率或展示汇率变化都必须使用新的 ID；同一 ID 内容变化由发布校验拒绝，避免日志中的版本标识失真。
   Web 使用紧凑模型行、standard/fast 档位、开关和定宽数值输入，不展示原始 JSON，也不把输入控件拉伸到
   页面全宽。模型名称只允许从下拉候选选择；管理设置 DTO 把 PublishedSnapshot 中 Codex OAuth 已选择或
   发现的精确上游模型名作为动态建议返回，Web 再与默认、当前费率卡的模型合并，确保既有配置始终可见且无需
   前端硬编码。该候选只是编辑建议，不改变费率卡对精确模型名的 Schema 校验。费率字段按 Credits/百万
   Token 展示并在提交边界精确换算为 nano-Credits；版本 ID 属于内部审计字段，不显示也不允许手工编辑，
   页面在每次语义修改保存时自动产生不同于当前值的新 ID。
3. 候选配置发布时完整解析并校验费率卡，再随 PublishedSnapshot 原子切换。公开请求只读取请求捕获的
   snapshot。Attempt 准备时把匹配的费率值和卡片 ID 冻结进 RequestLog；后续覆盖值变化不重算历史日志。
4. OAuth 额度响应携带当前 snapshot 的 `credits_per_usd` 和费率卡 ID。真实 Credits 与容量估算都使用同一
   响应内的展示汇率换算，前端不再保留定价常量。缓存额度 GET 使用读取时的当前配置，因此显示汇率修改后
   立即生效；历史 RequestLog 的 Token→Credits 计价仍由各日志已冻结的卡片 ID 说明。
5. Web 只展示当前费率换算后的确定容量与累计区间数，不展示证据、置信度、样本质量、最近区间、近似符号
   或历史费率卡 ID；这些字段已由 ADR-0146 从当前契约删除。

## 边界

- 费率配置只服务本机观测与美元等值展示，不形成计费、余额、套餐或 Gateway Key 配额。
- `credits_per_usd` 变化只改变展示换算，不改变 estimator 的规范 Credits 容量和既有累计。
- Token→Credits 费率变化只影响新请求。历史 RequestLog 保留冻结的费率卡 ID；累计容量只保存已经换算好的
  Credits 和官方百分比总量，不再复制历史费率卡集合。
- 无匹配模型或档位的请求保持未计价，禁止使用默认模型或最近费率猜测。

## 验证

- Domain 测试覆盖默认费率等价、严格 Schema、数值边界、Token 成本计算和设置值往返。
- Runtime 发布测试覆盖同 ID 改变内容被拒绝、换用新 ID 后原 PublishedSnapshot 不变。
- Server/Web 测试覆盖额度响应中的展示汇率、真实 Credits 与本机容量共用换算、结构化费率编辑、自动版本换代
  和不含置信状态的容量展示。
