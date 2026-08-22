# 架构主题文档

[ARCHITECTURE.md](../../ARCHITECTURE.md) 提供系统地图和跨主题不变量；本目录维护可随实现更新的当前事实。
每项事实只放在一个主题中，其他文档使用链接。设计理由及历史状态见 [ADR 索引](../adr/README.md)。

| 主题 | 内容 |
|---|---|
| [协议与桥接](protocol-bridges.md) | Provider 能力、协议 Operation、Bridge、目标 Profile |
| [路由与流式](routing-and-streaming.md) | 请求阶段、RPM、粘性、重试、健康和 SSE 生命周期 |
| [存储与安全](storage-and-security.md) | SQLite、配置发布、Migration、Secret、认证和遥测 |
| [运维](operations.md) | 构建、平台、部署、停机、自更新和内存组件 |
| [Web](web.md) | React 所有权、服务端状态、实时事件、响应式和测试边界 |

代码可直接枚举的内容不复制到这里：

- 设置定义：`crates/domain/src/settings/definitions/registry.rs`
- Provider descriptor 和 facet：`crates/provider/src/api`
- Protocol registry/Profile：`crates/protocol/src/api`
- 公开 HTTP 路径：`crates/server/src/public/mod.rs`，使用者清单见 [README](../../README.md)
- SQLite 当前结构：`migrations/` 的完整前向链
- 管理 DTO：Rust 导出生成的 `web/src/shared/api/generated`

修改行为时更新拥有该事实的一个主题；不要同步维护大段矩阵、默认值或源文件布局快照。
