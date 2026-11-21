# Rust 架构：按产品能力组织（2026-11-21）

> 本文回答“新代码放哪里、依赖谁能指向谁”。历史设计演进见 [DESIGN.md](../DESIGN.md)。

## 一句话

`app` 负责装配，`gateway` 负责产品能力，`drivers` 只做 C 适配，`ui` 是唯一屏幕写者。

早期目录按“验证 bench 用到了哪些外设/协议”切分（`drivers/`、`radio/`、`platform/` 三套平行外壳）。现在按产品主线切分：**接入设备 → 维护设备模型与在线状态 → 本地自动化 → 上云 → 展示与电源**。外设属于实现细节，统一收进 `drivers/`。

## 目录与职责

```text
src/
  main.rs                  启动 ESP-IDF 运行时，调用 app::run()
  lib.rs                   模块清单与分层规则
  app/                     组合根：只做装配，不放业务规则
    mod.rs                 board + identity + 双 executor + 服务注册
    board.rs               板级探测，逐项降级为 Option
    runtime.rs             critical-section 的 FreeRTOS 实现
  drivers/                 C 适配层：tg_* 的薄封装与所有权守卫
    lease.rs               单所有者 Lease + C 错误码映射
    board/                 板载外设：button/rgb/display/sdcard/ups
    radio/                 射频协议：ble/ble_ad/zigbee/zigbee_check
    net/                   Wi-Fi
    sys/                   芯片服务：system/temperature
  gateway/                 产品领域与能力服务
    device.rs              物模型：Protocol/Property/Value/GatewayEvent
    bus.rs                 进程内事件总线（access → UI/上云/规则）
    identity.rs            gateway_id、boot_counter、boot_id、seq
    presence.rs            设备在线判定（纯逻辑，可宿主测试）
    access/                接入能力
      zigbee.rs            Zigbee 协调器服务：入网、发现、上报、在场
  ui/                      展示层：唯一允许写 OLED 的地方
    console.rs             状态页行缓冲与分页刷新（后端无关）
    icon.rs                图标词汇
    macros.rs              print!/println!/print_at!/println_at!
    view.rs                事件 → 屏幕文本
    task.rs                低优先级 UI 任务
```

## 依赖方向

```text
main ──> app ──> gateway ──> drivers
          └────> ui ──────> drivers (仅 display)
```

- `drivers/` 不依赖 `gateway/`、`ui/`、`app/`。
- `gateway/` 只能通过 `drivers/` 访问硬件；唯一例外是 `identity.rs` 使用 esp-idf-svc 的 NVS **服务** 持久化业务数据。
- `ui/` 只能通过 `drivers/board/display` 拿屏幕，且 `Display` 只由 `Ui::new` 构造。
- `app/` 可以依赖所有层，但只做装配与生命周期管理。

## 单一所有者与串行契约

每个 C 组件在固件生命周期内只允许一个 Rust 所有者：

- 所有权由 `drivers::lease::Lease` 的原子守卫保证，重复初始化返回错误。
- “同一时间只有一个调用者”由任务结构保证：一个服务任务持有自己的驱动，跨 `await` 持有；不许把驱动共享给多个任务。
- 释放失败时保留所有权，避免第二个所有者接管半释放的 C 子系统。

## 事件流

```text
C 事件队列
   │ poll
   ▼
access 服务 ──> GatewayEvent ──> bus ──┬─> ui 任务
   │                                   ├─> uplink（M3）
   │                                   └─> rules（M4）
   └─ 设备/端点状态留在 access 内部
```

总线只传“值得对外表达的事实”；服务内部状态（端点表、请求 id、去重表）不穿过总线。

## 产品里程碑与落点

| 里程碑 | 能力 | 代码落点 |
| --- | --- | --- |
| M1（已完成） | Zigbee 接入观察、身份、UI 状态页 | `gateway/access/zigbee.rs`、`gateway/identity.rs`、`ui/` |
| M2 | 命令执行、TX-confirm 验证、`command_id` 去重与过期 | `gateway/command.rs`、`gateway/access/zigbee.rs`、`drivers/radio/zigbee_check.rs` |
| M3 | Wi-Fi + MQTT 上云：events/status/commands/results、心跳、断网补传 | `gateway/uplink/`、`drivers/net/wifi.rs` |
| M4 | SD 上的 `rules.toml` 本地 TCA、离线队列持久化 | `gateway/rules/`、`gateway/storage/`、`drivers/board/sdcard.rs` |
| M5 | BLE 执行设备接入与 GATT 控制 | `gateway/access/ble.rs`、`drivers/radio/ble.rs` |
| 贯穿 | UPS 电源监测、按键交互、配网 | `gateway/power.rs`、`ui/`、`drivers/board/ups.rs` |

未落地的能力只在文档中占位，不在 `src/` 里建空模块；实现时按上表落点新增目录。
