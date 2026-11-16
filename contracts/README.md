# TinyGate 通信契约 v1

日期：2026-11-13。状态：协作开发基线，尚不表示接口已实现。

## 通信边界

```text
C 驱动 / BLE 执行设备 ←→ Rust 网关 ← MQTT/JSON → 后端 ← HTTPS/JSON → Web / 微信小程序
                                                        └ WSS → 状态通知
```

客户端不直接连接网关或 MQTT Broker。后端负责认证、资源授权、持久化、HTTP 与 MQTT 转换；网关负责协议接入、本地规则与实际执行。Web 与小程序共用后端 API。

## 契约文件

- [openapi.yaml](openapi.yaml)：HTTP 请求、响应、鉴权与错误。
- [mqtt.md](mqtt.md)：Topic、消息语义、确认、重试、离线补传。
- [device-models.md](device-models.md)：设备身份、属性、能力及命令生命周期。
- [realtime.md](realtime.md)：WebSocket 通知及断线恢复。
- [schemas/messages.schema.json](schemas/messages.schema.json)：公共模型、MQTT 与实时通知 JSON Schema（2020-12）。
- [examples/scenarios.json](examples/scenarios.json)：成功、离线、超时、重复请求和补传的联调数据；每项 `schema` 指向对应定义，`payload` 为消息本体。

## 公共约定

1. 编码 UTF-8，JSON 字段 `snake_case`。ID 为 1～128 字符的不透明字符串；网关 ID 只使用字母、数字、下划线和连字符。
2. `schema_version: 1` 为消息大版本；HTTP 使用 `/api/v1`。增添可选字段保持兼容；删除字段、改变类型或语义、新增需要客户端处理的枚举值必须协商。消费者忽略未知字段，但不能忽略未知命令动作或不支持的版本。
3. 时间 `*_at_ms` 为 UTC Unix 毫秒整数，范围 0～9007199254740991。设备采集时间未同步时显式传 `null`，同时提供 `uptime_ms`；后端另记 `received_at_ms`，不能冒充采集时间。
4. JSON 使用原生 boolean/number；不发送 NaN/Infinity。不用空字符串表示缺失。Schema 的 `required` 决定必填，类型中的 `null` 决定能否为空。
5. API 列表使用 `data.items`、`data.next_cursor`；无下一页时游标为 `null`。`limit` 为 1～100，默认 20；游标不透明且绑定筛选条件。
6. 错误按 HTTP 状态码与稳定 `error.code` 处理，`message` 仅用于展示。不要将所有错误包装为 HTTP 200。
7. 建议 v1 单条 MQTT JSON 上限 8192 字节（UTF-8 字节数），接收方超限拒绝并记录；大型规则文件另行约定分片/下载协议。本版本不定义规则更新、登录流程、历史分析和 Webhook API。

## 分工与变更流程

| 负责人 | 接口责任 |
| --- | --- |
| 硬件 / C | BLE/Zigbee 事件和控制结果映射、设备身份、执行模拟器 |
| Rust 网关 | MQTT 消息、规则动作、命令去重与过期、缓存补传 |
| 云端 | HTTP/WSS、鉴权、存储、消息转换、前端与小程序 Mock |

Rust 与云端共同确认 MQTT；C 与 Rust 共同确认设备动作和结果。修改契约时在同一提交更新 Schema、文档和样例，影响双方确认后再实现。不得各自维护不同字段版本。

## 首期联调验收

- 两台 S3 使用独立 `device_id`；可单独查询和设置 `power`，不串设备。
- `POST commands` 返回 202；仅设备反馈目标状态后命令才是 `succeeded`。
- 网关或设备离线时拒绝创建即时控制；受理后发生断连则按命令有效期处理。
- 重复 HTTP 请求复用命令 ID；重复 MQTT 命令不会再次产生新的逻辑执行。
- 超时显示“未确认执行结果”；晚到状态仍可更新实际状态，但不反转终态。
- 旧补传事件进入历史，不覆盖新状态、不触发实时自动化。
- HTTP/消息模拟器使用 examples 中同一批数据；当前驱动仅扫描能力不视为 GATT 控制已实现。

设计依据：[OpenAPI 3.1](https://spec.openapis.org/oas/v3.1.0.html)、[MQTT 3.1.1](https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html)。
