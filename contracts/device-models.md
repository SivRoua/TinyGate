# 设备模型与命令生命周期 v1

## 身份与能力

- `gateway_id` 在部署时分配并持久保存；`device_id` 在系统内唯一，绑定网关，名称允许修改。
- 不直接将 BLE 临时地址或 Zigbee 短地址作为设备永久 ID。C 层输出协议身份，Rust 将其映射到稳定 ID；两台 S3 分别使用 `actuator_01`、`actuator_02`。
- Zigbee Endpoint 在统一模型中使用字符串，例如 `"1"`；BLE 模拟设备也使用 `"1"`。多端点属性路径为 `(device_id, endpoint, property)`。
- `capabilities` 声明各端点属性的类型、读写权限和单位，以及支持的动作。前端依此决定控件，后端和网关仍独立校验权限与参数。
- `reported_state` 按 endpoint 分组，仅包含实际反馈。命令受理不能提前改写它。`last_seen_at_ms` 是后端观察到的最近实时活动时间；离线补传不刷新在线租约。

| 类型 | endpoint | 属性 | 类型 / 单位 | 动作 |
| --- | --- | --- | --- | --- |
| BLE 执行模拟器 | `1` | `power` | boolean，无单位 | `set_power`，参数仅 `power: boolean` |
| Zigbee 温度传感器 | 按实际 Endpoint | `temperature` | number，`degC` | 无 |

首期控制请求必须指定 `endpoint`。S3 的 GATT UUID、特征、帧格式和配对方式由 C/Rust 共同另行冻结；本契约定义逻辑动作，不声称已有 GATT 控制实现。两台 S3 共用固件协议，设备身份不同。

## 实际状态排序

网关持久化递增 `boot_counter`（每次启动加一）并产生不重复 `boot_id`；本次启动内 `seq` 全局递增。事件唯一键为 `event_id`，可取 `gateway_id:boot_id:seq`。重传保持全部原始标识和序号。

后端对每个 `(device_id, endpoint, property)` 按 `(boot_counter, seq)` 记录最近顺序。历史事件可以入库，但旧顺序不能覆盖当前状态。不要按 MQTT 到达顺序或未经同步的采集时间更新状态。恢复出厂必须重新注册身份或恢复计数基线，不能让计数回退后继续使用原身份。

后端为每台设备维护递增 `state_revision`，每次对外快照变化时增加，包括属性、在线状态和能力变化；HTTP 与 WSS 使用同一修订号。它不同于网关 `seq`，不能混用。

## 命令状态

```text
queued → dispatched → accepted → succeeded
   └───────────┴────────┴──────→ failed / expired
```

| 状态 | 语义 |
| --- | --- |
| queued | 后端已持久化受理，尚未发布 |
| dispatched | 已发布至 Broker，尚未收到网关业务确认 |
| accepted | 网关验证身份、动作、在线状态、有效期后已接收 |
| succeeded | 执行设备确认目标状态；仅 BLE 写入成功不足以判定 |
| failed | 明确拒绝或执行失败，必须带非空 error |
| expired | 有效期已过，未确认成功；必须带非空 error |

结果可能先于中间通知到达，后端允许跳过中间状态，禁止状态倒退；终态不可变。超时不证明设备没有动作，UI 应提示“未确认执行结果”。晚到反馈可以更新 reported_state 和历史，但不将 expired 改为 succeeded。

`timeout_ms` 为 100～30000，默认 5000；从后端受理时间开始计时，包含排队、传输、执行。后端生成 `issued_at_ms` 与 `expires_at_ms`。网关在校验和执行前检查过期，离线不积压即时控制。时间无效时返回 `CLOCK_UNSYNCED`，不得自行延长有效期。

## 幂等与去重

- HTTP `Idempotency-Key` 必填，按用户与设备作用域保存至少 24 小时。同键同规范化请求返回原命令（202，即使命令已终结）；同键不同请求返回 409 `IDEMPOTENCY_CONFLICT`。幂等重放先于在线状态检查，但必须先鉴权。
- 云端至网关重试保持同一 `command_id`、参数和有效期，不生成新命令。建议 1 秒后重试，之后每 2 秒重试，直到收到 accepted/终态或到期。
- 网关以 `command_id` 去重，持久保存已接收状态和最后结果，至少保留至 expires_at_ms 后 10 分钟；重复请求重发已知结果，不创建第二次逻辑操作。同 ID 不同内容拒绝并记录协议错误。
- 网关重启后不能确认未完成动作时，读取设备状态进行核对；无法核对则报告 failed / `EXECUTION_UNCERTAIN`，不盲目重放非幂等动作。首期只支持幂等 `set_power`。

## 错误码

| code | 使用场景 | HTTP（同步拒绝） |
| --- | --- | --- |
| INVALID_ARGUMENT | 参数或 Schema 不符 | 400 |
| UNAUTHENTICATED | 无有效用户身份 | 401 |
| FORBIDDEN | 无资源权限 | 403 |
| NOT_FOUND | 资源不存在 | 404 |
| GATEWAY_OFFLINE / DEVICE_OFFLINE | 即时控制目标离线 | 409 |
| IDEMPOTENCY_CONFLICT | 幂等键请求不一致 | 409 |
| UNSUPPORTED_ACTION | 设备不支持动作 | 422 |
| RATE_LIMITED | 请求过频 | 429 |
| INTERNAL_ERROR | 后端内部异常 | 500 |
| CLOCK_UNSYNCED | 网关时钟无效 | 异步 failed |
| DEVICE_TIMEOUT / EXECUTION_UNCERTAIN | 设备未响应 / 结果不确定 | 异步 failed |
| COMMAND_EXPIRED | 命令到期 | 异步 expired |

HTTP error 包含 request_id；MQTT error 嵌入命令结果，不使用 HTTP 状态码。
