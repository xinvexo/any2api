# ADR-0101: 重试共享不可变 DecodedRequest

> 状态：Accepted
> 日期：2026-08-03
> 决策者：maintainer

## 背景

公开请求在入口已经解析为 `DecodedRequest`，其中 JSON payload 最多可达 32 MiB，Images Edit
可达 64 MiB。原实现仍在每个 Attempt 调用 `plan.decoded.clone()`；`serde_json::Value::clone`
会深拷贝整棵树，两个 HeaderMap 和 multipart part Vec 也随之复制。单次成功请求同样承担这次
复制，重试会线性放大峰值内存。OAuth 刷新后的 replan 还会再复制一份完整请求。

Protocol 的编码接口之所以消费 payload，只是为了替换 `model`、按操作移除 `stream`，或替换
multipart 的 model part；这些变化都只服务本次 wire body，不需要修改入口解析结果。Responses
到 Chat Completions 的 Bridge 还会先复制 conversation 生成 messages，再复制一次 conversation
保存 continuation，临时所有权也不合理。

## 决策

1. Runtime 的 `PlannedRequest` 使用 `Arc<DecodedRequest>` 保存入口完成规范化后的不可变计划。
   Attempt 和 OAuth replan 只借用或克隆 Arc，禁止克隆 `DecodedRequest`。
2. `ProtocolExchange`、`ProtocolAdapter::encode_upstream_request`、`ProtocolBridge::start` 和恢复
   continuation 的入口统一借用 `DecodedRequest` / `AdapterPayload` / HeaderMap。具体交换会话仍
   必须拥有自身的可变响应转换状态，不能借用公开请求跨越流式生命周期。
3. JSON 出站编码使用借用序列化视图：按原 Map 迭代顺序输出全部字段，只在序列化 `model`
   时写入最终上游模型，并按既有操作规则跳过 `stream`。内部异常 payload 缺少 model 时允许
   退回原有的 clone-and-insert 路径；正常入口和 Bridge 路径不得触发该回退。
4. multipart 出站编码直接遍历借用的 part 列表，仅在 model part 位置写入替换字节；其他文件
   `Bytes`、顺序和安全 Header 不复制。每次编码继续生成独立 boundary，不改变既有线协议语义。
5. Responses → Chat Completions Bridge 先编码本次转换 body，再从该临时 body 移出 conversation；
   完成响应后把 session conversation 移入 continuation，禁止用完整 Vec clone 保留已不再需要的
   临时副本。已有 continuation 在新请求中仍需生成独立可变 conversation，这一真实所有权复制保留。

## 不变量

- 同一 JSON 请求、上游模型和操作在首次 Attempt 与后续 Attempt 中生成逐字节相同的 body；
  未知字段、字段迭代顺序、`stream` 规则与协议 Header 不变。
- 任一 Attempt 或 Bridge 都不能修改共享入口 payload；失败、取消或重试不影响下一 Attempt。
- multipart 文件字节、字段顺序、重复字段和安全 Part Header 保持原样；只替换唯一 model part。
- Secret、客户端认证头和原始正文仍不得进入 Debug 或日志。

## 验证

- Protocol 单元与 Registry 契约枚举所有 Adapter，固定借用编码前后 payload 不变，并比较同一
  JSON payload 连续编码的完整 wire body。
- Runtime 重试契约固定多 Attempt 发送完全相同的请求正文，并覆盖 OAuth replan 共享同一计划。
- 使用独立测试进程对大 JSON 的 legacy deep-clone 模式和 shared-borrow 模式记录峰值常驻内存与
  编码耗时；命令、样本大小和结果写回本 ADR，避免只凭类型推断宣称收益。

## 基准结果

2026-08-03 在 Darwin arm64 26.6、Rust 1.90.0 debug test profile 上，使用 200,000 个
不同字符串节点组成的 14,000,045-byte 出站 JSON，一次性比较 deep-clone 与 shared-borrow，外层用
`/usr/bin/time -l` 记录 `maximum resident set size`。每种模式独立运行 5 次并取中位数：

| 模式 | 编码耗时中位数 | 峰值 RSS 中位数 |
|---|---:|---:|
| legacy deep-clone | 317 ms | 74,121,216 bytes |
| shared borrow | 312 ms | 51,527,680 bytes |

shared-borrow 在该样本中减少 22,593,536 bytes 峰值 RSS（约 30.5%），编码耗时没有回退。
该数字包含测试进程与原始 payload，不外推为所有请求的固定节省；结构越密集、Attempt 越多，
移除深拷贝所避免的分配越明显。结果保留于本 ADR；一次性 benchmark 源码在决策后删除。
