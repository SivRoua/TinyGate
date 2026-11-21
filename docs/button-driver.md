# GPIO9 按键驱动

## 接线

常开按键一端接 GPIO9，另一端接 GND；软件启用输入上拉，低电平表示按下。
松开应读到高电平，按下读到低电平，不要把输入直接接到 5V。
如用四脚轻触开关，要选按下才接通的两端，同侧脚通常已内部相连。

GPIO9 也是 C6 的 BOOT/启动配置引脚：上电、复位和烧录结束启动时应松开按键。
如果板载 BOOT 同样接 GPIO9，两者是同一逻辑输入，软件无法区分来自哪一个按键。

## 组件与分工

```text
components/tg_button/
  include/tg_button.h    对 Rust 公开的 C ABI
  src/tg_button.c        GPIO、采样任务、事件缓存、生命周期
  src/button_state.c    无硬件依赖的消抖与长短按状态机
src/drivers/board/button.rs  Rust 独占安全封装
src/app/board.rs        板级探测（按键入槽；事件消费待配网里程碑）
tests/button_state_test.c
```

驱动独占该 GPIO，公开 API 由同一应用任务串行调用，不能在 ISR 中调用。
不配置 GPIO 中断，也不安装全局 ISR 服务；FreeRTOS 任务以约 10ms 间隔采样。
该任务优先级 3，SDK Zigbee 任务优先级 5，不在射频回调中处理按键业务。
实际延迟还受任务调度与采样周期影响，不承诺硬实时按键时间精度。

## 默认事件

- 消抖 30ms；持续按下达到 1000ms 为长按。
- `PRESS`：消抖后的按下边沿，held_ms 为 0。
- `SHORT`：释放确认时产生，后面跟一个 `RELEASE`。
- `LONG`：按住达到阈值时产生一次；释放时只产生 `RELEASE`，不会再产生短按。
- 如果恰在阈值释放且尚未产生长按，释放确认时补发一次 `LONG` 和 `RELEASE`。
- 启动时若已按住，不产生虚假的按下/长按；稳定释放后才接受下一次正常按压。

事件包含单调运行时间 timestamp_ms 与按住时长 held_ms，不是日历时间。
按住时长基于最终通过消抖的边沿起点计算，释放确认的消抖等待不计入时长。
状态机使用 64 位时间；超过 uint32 范围的 held_ms 饱和到 UINT32_MAX。

## 公开接口

| API | 用途 |
| --- | --- |
| `tg_button_init()` | GPIO9、30ms、1000ms 默认配置 |
| `tg_button_init_with_config()` | 指定 GPIO、消抖和长按阈值 |
| `tg_button_poll_event()` | 非阻塞读取值副本；无事件返回 ESP_ERR_NOT_FOUND |
| `tg_button_get_status()` | initialized、消抖后的 pressed、丢弃事件累计数 |
| `tg_button_deinit()` | 通知任务退出，最多等待 500ms，之后释放任务、GPIO 与缓存 |

参数范围：消抖 10..200ms，长按阈值不小于消抖且不超过 60000ms。
空指针/无效参数返回 ESP_ERR_INVALID_ARG，重复初始化/未启动操作返回状态错误。
缓存 16 项，满时丢弃新事件并累计 dropped_events，不阻塞采样。
高层若依赖每个边沿，应监测丢弃数并用 pressed 快照重新同步，不能假设队列永不丢数据。
销毁等待超时则保留资源和 Rust 所有权，可再次尝试销毁，不强行释放仍在使用的内存。

## Rust 接入与演示

```rust,ignore
let mut button = button::Button::new()?;
if let Some(event) = button.poll()? {
    // 根据 kind 决定配网、切页等动作；本次仅打印，不自动控制或重置网络。
    log::info!("{} held={}ms", event.kind, event.held_ms);
}
// button.shutdown()?; // 不再使用时主动释放，Drop 提供尽力清理
```

默认 Zigbee 主循环每约 20ms 消费按键。非 Zigbee 模式的 Wi-Fi/BLE 扫描等待和心跳等待
也会持续消费事件，避免用 10 秒应用休眠阻塞按键读取。C 采样本身始终独立运行。
不改变现有 Zigbee 自动测试，也不把短按直接接到执行器控制、入网或清除网络操作。

预期串口格式（并非实测日志）：

```text
BUTTON GPIO9 PRESS held=0ms at=...
BUTTON GPIO9 SHORT held=250ms at=...
BUTTON GPIO9 RELEASE held=250ms at=...

BUTTON GPIO9 PRESS held=0ms at=...
BUTTON GPIO9 LONG held=1000ms at=...
BUTTON GPIO9 RELEASE held=1800ms at=...
```

## 验证

```powershell
gcc -std=c11 -Wall -Wextra -Werror -Icomponents/tg_button/src tests/button_state_test.c components/tg_button/src/button_state.c -o target/button-state-test.exe
./target/button-state-test.exe
# 自定义 C 组件变化后刷新 SDK 缓存/绑定
cargo clean -p esp-idf-sys
cargo build
# 原 Wi-Fi/BLE 演示回归
cargo build --no-default-features
# 烧录默认 Zigbee 模式时使用 cargo run，它会重新选择默认 feature
cargo run
```

上板需要验证：松开无事件、短按一次只产生一组事件、长按不重复且无 SHORT、连续快速按压、
无线运行时仍可响应，以及初始化/释放后重新初始化。
本次主机状态机测试和固件构建不能代替真实接触抖动、线路与 GPIO 电平测试。
