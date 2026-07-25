# ADR-0038: 负载均衡页只展示聚合运行态

- 状态：Partially superseded by ADR-0039
- 日期：2026-07-25
- 决策者：maintainer
- 取代：ADR-0023 中管理 API 与 Web 的逐 Credential/模型运行态目录

> ADR-0039 保留本 ADR 的固定规模聚合 API，但把聚合展示移入总览，把 `scheduler.*` 移入“设置 → 路由策略”，并删除独立负载均衡一级入口。

## 背景

原负载均衡页会在每次轮询中返回并渲染全部 ProviderCredential、OAuthAccount 及其全部模型健康状态。
实例拥有数百或上千账号时，这会形成巨大的页面、重复的账号目录和随账号/模型数量线性增长的浏览器
响应体。即使只在前端虚拟化，服务端仍会每 5 秒序列化并传输完整账号集合，不能解决根本问题。

Provider API Key 与 OAuthAccount 已分别拥有专用管理页面，账号配置、模型与历史请求统计不需要在
负载均衡页复制一份。

## 决策

- 普通 Web 只展示全局汇总、Codex/Claude Provider 汇总、队列状态和 scheduler epoch；ADR-0039
  进一步把这些汇总归入总览，把 `scheduler.*` 归入设置。
- `GET /api/admin/balancing` 只返回聚合数据，不返回逐账号 ID、标签、Endpoint、Proxy、RPM 窗口、
  过滤计数、模型集合或分层健康状态。
- Runtime 在当前 PublishedSnapshot 上单次遍历路由凭据，直接累加全局和 Provider 汇总；不为该
  管理请求构造 Credential×Model 健康快照。
- 账号级配置与历史请求统计继续分别由 Provider 和 OAuth2 登录页面负责。负载均衡页不增加账号
  分页、虚拟列表、搜索或详情抽屉。
- Credential 级选择/过滤原子计数可以继续保留为调度实现与模块测试的一部分，但普通管理契约不
  暴露这些内部明细。

## 备选方案

- 前端虚拟化全部账号：拒绝。只能减少 DOM 节点，无法减少每次轮询的服务端计算与响应体。
- 服务端分页账号详情：拒绝。负载均衡页不是账号管理入口，会继续复制 Provider/OAuth 页面职责。
- 默认折叠、按需展开：拒绝。仍需要传输账号集合，并让页面承担不必要的第二份账号目录。

## 后果

负载均衡响应体规模只随 Provider 种类增长；首版固定为 Codex 与 Claude，因此即使配置 1000 个账号，
浏览器仍只接收固定数量的汇总记录。服务端聚合仍需线性读取当前账号的轻量计数，但不再遍历模型健康
或序列化账号明细。

账号问题的具体定位需要进入 Provider/OAuth 页面或请求日志；总览专注回答“整体是否拥塞、哪个
Provider 正在消耗 RPM、是否排队”，具体策略在设置页修改。

## 验证

- Runtime 测试覆盖空配置和多 Provider 聚合，不产生 Credential/模型明细。
- 管理 HTTP 契约确认响应没有 `credentials` 字段，并验证全局/Provider 汇总与 no-store。
- Web 测试确认页面只展示汇总、队列和刷新错误，不渲染账号标签或模型健康。
