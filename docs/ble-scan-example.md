# BLE 扫描驱动示例

> 2026-11-14：BLE 已迁入独立 `components/tg_ble` 组件。
> 当前主程序先运行 Wi-Fi 扫描，再运行本节 BLE 示例，最后打印系统心跳。
> 完整目录与新接口见 [板上资源驱动说明](onboard-drivers.md)。

本例实现 ESP32-C6 的传统 BLE 广播扫描，不需要外接排针、SD 或 OLED。
不包含 GATT 连接、控制两台 S3 或 Wi-Fi/Zigbee 共存验证。

## 代码归属

| 文件 | 职责 | 分工 |
| --- | --- | --- |
| `components/tg_ble/include/tg_ble.h` | C ABI、扫描配置、事件与错误约定 | 硬件/驱动成员 |
| `components/tg_ble/src/tg_ble.c` | NimBLE 生命周期、GAP 扫描、回调与事件缓存 | 硬件/驱动成员 |
| `components/tg_ble/CMakeLists.txt` | C 源文件、bt/nvs_flash 依赖 | 硬件/驱动成员 |
| `sdkconfig.defaults` | 启用 NimBLE 与扫描角色 | 双方协作 |
| `src/ble.rs` | 生成绑定的安全封装、资源所有权 | Rust 核心成员 |
| `src/ble_ad.rs` | 广播名称解析 | Rust 核心成员 |
| `src/main.rs` | 示例业务：等待就绪、扫描、打印、退出 BLE | Rust 核心成员 |

Cargo extra_components 使用 `components/platform_bindings.h` 汇总公开头文件，
各 C 组件保持独立；Rust 不手写 BLE extern 声明。

## 驱动方式

```text
Rust Scanner::new()
  -> C tg_ble_init()
  -> NVS 初始化（不自动擦除）+ NimBLE Host 启动
  -> Host sync 回调将 ready 设为 true

Rust Scanner::start(15000)
  -> C tg_ble_scan_start()
  -> NimBLE GAP 主动扫描
  -> GAP 广播回调复制报告到固定 32 项环形缓存

Rust Scanner::poll()
  -> C tg_ble_poll_event() 非阻塞读取值副本
  -> Rust 解析名称、反转地址字节用于显示、输出日志
```

协议栈在自己的任务中接收广播；Rust 每次无事件时休眠 10 ms。
回调不调用 Rust、不分配业务对象、不写 SD、不打印广播日志。
回调与读取任务通过短临界区保护状态和定长复制。

## 接口与生命周期

所有公开 C 调用必须由同一个应用任务串行执行，禁止 ISR/Host 回调调用。
此组件独占 NimBLE 初始化与销毁，不可与另一套 BLE 管理代码同时使用。
Rust 包装通过进程级所有权标记防止重复创建，且 Scanner 不可跨线程传递或共享。

| 接口 | 说明 |
| --- | --- |
| `tg_ble_init` | 启动 Host；成功仅表示启动已提交，需等待 ready |
| `tg_ble_get_status` | 查询 initialized/ready/scanning、累计丢弃数、最近 Host 错误 |
| `tg_ble_scan_start` | 有限时扫描；时长 1..INT32_MAX ms；间隔、窗口单位 0.625 ms |
| `tg_ble_poll_event` | 返回报告、扫描完成或 Host reset；暂无事件返回 ESP_ERR_NOT_FOUND |
| `tg_ble_scan_stop` | 取消扫描；已停止时返回成功；取消与正常结束均有完成通知 |
| `tg_ble_deinit` | 在应用任务中阻塞停止 Host、释放 BLE 资源；不销毁共享 NVS |

其他状态：空指针或参数越界为 ESP_ERR_INVALID_ARG；未就绪、重复初始化、
扫描中启动等为 ESP_ERR_INVALID_STATE；Host 操作失败为 ESP_FAIL。
最近的底层错误可从 status 查询；完成/reset 的 reason 是 NimBLE 原始原因码。

扫描配置默认 100 ms 间隔、50 ms 窗口、主动扫描、控制器重复过滤。
仅支持传统广播，每个报告最大 31 字节。扫描响应作为独立报告保留，
advertising_type=4（Rust 的 scan_response=true）；不自动拼接广播与响应。
地址按 NimBLE 原始顺序保存，显示时反序；地址类型一起传递。
随机地址不等于稳定设备 ID；扫描报告数量也不等于独立设备数量。

报告缓冲满时丢弃新报告并累计 dropped_reports，不阻塞协议栈。
完成和 reset 状态单独保存，不占报告缓存；reset 优先读出，完成在旧报告读完后返回。
开始下一轮前必须消费上一轮事件，避免混淆会话；统计从初始化起累计。
若长时间未读，多次 reset 会合并为最近一次原因，属于状态通知而非可靠审计日志。

## 构建与运行

沿用项目 `.cargo/config.toml`、ESP-IDF v6.1 和 Rust esp 工具链。
未配置的机器先参考 `.cargo/config.toml.example` 设置本机工具路径。

```powershell
cargo build
# 接板并选择正确串口后执行；此步骤将烧录固件：
cargo run
```

程序等待 Host 就绪最多 10 秒，扫描 15 秒，消费剩余报告后打印统计并关闭 BLE。
关闭后保持空闲，按板上 RESET 可重新运行。示例不会初始化其他外设占位驱动。
名称仅在对应报告含本地名称字段时出现，None 是正常情况。
附近设备必须实际发送 BLE 广播，仅打开手机蓝牙不保证可被发现。

以下是预期格式，并非实测日志：

```text
TinyGate C6 BLE scan example: C driver -> Rust events
Scanning for 15 seconds (active scan, duplicate filter enabled)
AA:BB:CC:DD:EE:FF type=1 RSSI=-60 dBm response=true name=Some("S3-A") data=[...]
Scan complete: native reason=...
Reports=... dropped=... host_error=0
BLE stopped and deinitialized. Reset board to scan again.
```

## 验证

无需硬件的名称解析测试：

```powershell
rustc --test src/ble_ad.rs -o target/ble-ad-tests.exe
./target/ble-ad-tests.exe
```

上板检查：

1. 开启一个已知名称的 BLE 广播源，检查地址、RSSI、名称和扫描响应。
2. 无广播源时仍应按时正常完成，不把“零报告”当作错误。
3. 临时在调用端将扫描时长加长、提前调用 stop，验证完成事件后可以销毁。
4. 消费完上一轮报告与完成事件后再次 start，验证重复扫描。
5. 临时放慢消费或关闭重复过滤，检查缓存满时 dropped_reports 增长、完成仍可读。
6. 测试非法窗口、重复 init/start、shutdown 后调用应返回错误，不能访问失效对象。

若 NVS 初始化报空间或版本问题，本例返回错误，不自动擦除已有配置。
构建成功不等于射频验证成功；Wi-Fi/BLE/Zigbee 共存应另行测量。

参考：
- https://github.com/espressif/esp-idf/tree/v6.1/examples/bluetooth/nimble/blecent
- https://github.com/esp-rs/esp-idf-sys/blob/master/BUILD-OPTIONS.md
