# MQTT 协议 v1

传输使用 MQTT over TLS，每台网关独立凭据。Broker ACL 将凭据绑定 gateway_id，仅允许该网关订阅自身下行、发布自身上行。后端核对 Topic、载荷及已认证身份，不能信任 JSON 中自报的归属。

## Topic 与 Schema

前缀 `tinygate/v1/gateways/{gateway_id}`：

| 后缀 | 方向 | QoS | retained | Schema 定义 |
| --- | --- | --- | --- | --- |
| `/events` | 网关 → 后端 | 1 | false | Event |
| `/status` | 网关 → 后端 | 1 | true | GatewayStatus |
| `/commands` | 后端 → 网关 | 1 | false | GatewayCommand |
| `/command_results` | 网关 → 后端 | 1 | false | CommandResult |

Schema 均在 `schemas/messages.schema.json#/$defs/名称`。QoS 确认仅为传输确认；MQTT PUBACK 不等于后端持久化或执行设备确认。业务语义以 command_results 和实际属性反馈为准。

## 在线与心跳

连接后发布 status，每 15 秒更新，后端超过 45 秒未收到本连接心跳则判离线。每次 MQTT 连接生成新 `connection_id`，LWT 携带相同 ID，`online=false`、`reported_at_ms=null`。正常退出主动发布 offline。

status 的 `boot_counter` 与 `connection_seq` 分别在重启和本次启动内重连时增加。后端按二元组比较连接新旧；旧连接的 LWT 不得覆盖新连接。retained online 只作提示，后端重启后也必须等待新心跳确认在线。在线/离线变化通知客户端，并更新网关修订号。

设备在线由网关按协议探测策略判断，通过 device_presence 事件报告；BLE 执行设备首期可用 15 秒探测、45 秒未成功响应判离线。传感器按上报周期配置超时，不能统一视为持续连接设备。

## 命令流程

1. 后端鉴权并校验能力、设备与网关在线状态，持久化 queued 后返回 HTTP 202。
2. 发布 GatewayCommand，记录 dispatched；重试沿用相同命令 ID 与过期时间。
3. 网关校验后回 accepted，拒绝则 failed；执行设备确认目标状态后回 succeeded。
4. 每个结果带递增 `result_seq`，同一命令重发同一结果保持相同序号。后端去重并按序合并；终态不可逆。
5. 云端和网关均在有效期结束时停止派发；云端仍未收到终态时记录 expired。不得发布 retained 控制命令。

全部时间、去重、超时和错误码遵守 device-models.md。建议 MQTT 会话不保存离线即时控制；即使 Broker 重投也必须检查 expires_at_ms。

## 事件与断网补传

Event.type 支持 property_reported、device_presence、alarm；data 结构随类型校验。事件含 boot_counter、boot_id、seq、uptime_ms、时间及 replayed。

- 网关缓存后按 FIFO 限速补传，建议初始上限 10 条/秒；实时控制与结果优先。历史补传不产生新的 event_id。
- replayed=true 的历史默认不触发实时自动化、不延长设备在线状态；旧属性不覆盖新属性。
- 后端以 event_id 唯一入库并记录 received_at_ms；若同 ID 不同内容（不含 replayed）则记录协议错误。
- **v1 补传保证范围**：QoS 1 PUBACK 后网关可将缓存标记已传送，保证到 Broker，不保证后端已入库。后端使用持久订阅与数据库去重降低丢失风险。如验收要求端到端持久化确认，须先扩展业务 ACK Topic/Schema，再删除缓存；不得宣称现有机制实现端到端无损。

## 初始设备目录

后端从已注册设备配置获得稳定 ID、名称和 capabilities；本版不定义自动发现/绑定消息。联调先预置 gw_01、actuator_01、actuator_02 和 sensor_01。MQTT 事件仅引用已登记且属于该网关的设备，未知设备不能自动获得控制权限。
