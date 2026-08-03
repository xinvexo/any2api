# ADR-0104: 收敛 Web 配置发布生命周期与稳定管理外壳

- 状态：Accepted
- 日期：2026-08-03
- 决策者：maintainer

## 背景

Provider Endpoint、Provider Credential、Proxy、Gateway API Key、OAuthAccount 和 Settings
在 mutation 成功后都按 `configRevision` 发布 React Query 缓存，并在失败后重新读取 active query。
此前每个 feature 各自复制“拒绝较旧 revision、写缓存、失效相关查询、失败后 refetch”模板，连完全相同的
revision 比较也各有一份文件和测试。

Provider 与 OAuth 页面还分别为首次加载、无数据错误和正常数据重复书写 `KindSplitLayout`。Provider
为了保持几何位置，在错误分支额外渲染了不可输入的假搜索框；OAuth 则把同一 toolbar 和 Provider 导航
复制三次。重复代码使按钮禁用条件、可访问名称和恢复动作容易只修改其中一条分支。

这些页面的具体 mutation 并不完全相同：OAuth 删除还要清除额度缓存，Provider Credential 只失效
Endpoint 列表，Settings 同时维护本地草稿。把全部 mutation 或全部 Query 状态塞进一个通用工厂/Boundary
会把差异变成回调和布尔参数，并没有形成稳定领域抽象。

## 决策

1. `shared/api` 提供有界的配置 mutation 生命周期 Hook，只负责：
   - 以 `configRevision` 单调选择最新配置，同 revision 接受服务端新响应；
   - 把完整配置写入一个明确的 Query key；
   - 成功后按调用方声明的可选 key 执行查询失效；
   - 失败后只 refetch 调用方声明的 active query 范围。
2. 各 feature 继续声明自己的 `useMutation`、输入类型、API 函数、pending 汇总、用户通知与特殊成功清理；
   共享 Hook 不接收任意 mutation 字典，也不解释业务错误。
3. 删除 Provider、Credential、Proxy、Gateway 与 Settings 各自的 revision 比较副本，以共享纯函数和一组
   契约测试固定相等/更新/陈旧三种情况。OAuth 使用同一选择规则。
4. Provider 与 OAuth 各自只保留一份导航/toolbar 外壳；首次加载、无数据错误、陈旧数据警告和正常内容
   只在内容槽内切换。Provider 搜索框只有真实列表可搜索时才出现，不用 disabled/readOnly 假控件占位。
5. 不新增通用 `QueryStateBoundary`。加载文案、错误恢复按钮、陈旧数据警告和列表骨架仍由 feature 决定，
   直到至少两个调用点具有相同视觉、恢复动作和数据契约。

## 后果

revision 顺序和失败恢复只有一个实现，配置 feature 仍保有清晰的业务 mutation。稳定外壳不因查询阶段
切换而复制或漂移，加载/错误状态也不会暴露无功能搜索框。共享 Hook 的参数必须保持在缓存 key、失效 key
和 refetch key 三个查询职责内；若未来需要业务回调或 mutation 编排，应留在 feature，而不是继续扩大 Hook。

## 验证

- 共享纯函数覆盖无当前值、相同 revision、新 revision 和陈旧 revision；Hook 测试覆盖缓存发布、可选失效
  与失败 active refetch。
- Provider/OAuth 组件测试覆盖首次加载、无数据错误、陈旧数据与正常列表共用同一外壳；Provider 非数据态
  不渲染假搜索框。
- 相关 React 测试、typecheck、ESLint、生产构建与内嵌资源一致性检查作为门禁。
