# 架构决策文档

当前架构事实只维护在 [`ARCHITECTURE.md`](../../ARCHITECTURE.md)。本目录不再为每个实现细节复制一份当前设计。

## 文档所有权

| 问题 | 唯一入口 |
|---|---|
| 当前需求、边界、不变量、模块、协议、设置和部署语义 | [`ARCHITECTURE.md`](../../ARCHITECTURE.md) |
| 当前设计为什么这样，以及已经舍弃的方向 | [`ADR-0170`](0170-current-decision-register.md) |
| 安装、运行、发布和部署操作 | [`README.md`](../../README.md) |
| Agent 协作、编辑和验证规则 | [`AGENTS.md`](../../AGENTS.md) |
| 官方客户端脱敏观测证据 | [`docs/baselines/official-clients`](../baselines/official-clients/README.md) |

规则很简单：修改一个当前事实时只修改架构基线；修改一个取舍理由时只修改 ADR-0170。不要把同一个事实
复制到 README、Agent 规则、测试说明或新的 ADR。被取代的设计不保留完整旧文档，只在 ADR-0170 的历史摘要中
保留方向性结论。

## 完整当前清单

| 编号与文档 | 用途 |
|---|---|
| [0170](0170-current-decision-register.md) | 合并后的当前取舍、理由和已舍弃方向 |

[0000-template.md](0000-template.md) 仅用于创建未来确有必要的独立 ADR，不属于当前决策。
