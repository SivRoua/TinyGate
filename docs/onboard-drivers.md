# 无外设板上资源驱动（2026-11-14）

目标按现有配置为 ESP32-C6；不假定 LED、BOOT 引脚，不访问未连接的外设。
芯片型号也会在启动日志中报告。实际核心板资料到位后再添加板级映射。

## 工程与分工

```text
components/
  platform_bindings.h              仅为 bindgen 汇总头文件
  tg_ble/{include,src}/             C：BLE 扫描；从 c_drivers 迁出
  tg_wifi/{include,src}/            C：Wi-Fi 扫描、STA 连接与状态
  tg_temperature/{include,src}/     C：片内温度
  tg_system/{include,src}/          C：芯片、复位原因、内存、运行时间
  c_drivers/                       保留的旧外设占位实现，本例不初始化
src/
  ble.rs                           Rust：BLE 所有权与安全封装
  ble_ad.rs                        Rust：广播名称解析
  platform.rs                      Rust：Wi-Fi、温度、系统信息封装
  main.rs                          Rust：运行演示与业务日志
```

你的主要交付是各组件的 C 头文件、实现与 CMakeLists.txt。
Rust 成员消费这些接口，决定控制策略、重连、错误提示和数据上报。
每个 C 组件有独立依赖；BLE 不依赖 Wi-Fi，Wi-Fi 不依赖显示或存储驱动。
SDK 协议栈由 C 层独占，不要再创建 esp-idf-svc 的另一套 Wi-Fi/BLE 所有者。

## 默认演示

1. 输出芯片型号、版本、核心数、复位原因、内存。
2. 读取真实芯片内部温度（不是环境温度）。
3. Wi-Fi 异步扫描，最多保存 20 条 AP，输出 SSID、信道、RSSI、认证模式。
4. 关闭 Wi-Fi 后进行 15 秒 BLE 扫描，随后关闭 BLE。
5. 释放温度传感器，每 10 秒输出运行时间与内存。

单个无线演示失败会记录错误后继续下一项；清理失败会保持该组件的 Rust 所有权锁定，
避免重新创建实例覆盖未释放资源。没有自动擦除 NVS、自动连接网络或写入凭据。
Wi-Fi 与 BLE 顺序运行不代表已验证共存稳定性。USB 使用项目现有日志与烧录通道。

## Wi-Fi 接口

所有公开调用由单个应用任务串行执行，不得在事件回调/ISR 内调用。

| C API | 语义 |
| --- | --- |
| `tg_wifi_init` | 初始化 NVS、网络运行时、默认事件循环、STA 网卡和 Wi-Fi；启动驱动 |
| `tg_wifi_scan_start` | 提交异步扫描；通过 scan_ready 查询完成 |
| `tg_wifi_scan_get_ap` | 读取指定结果的值副本；越界返回 ESP_ERR_NOT_FOUND |
| `tg_wifi_connect` | 提交 STA 连接，成功返回不代表已经联网 |
| `tg_wifi_disconnect` | 请求断开；等待 disconnecting=false 后再发起新操作 |
| `tg_wifi_get_status` | 获取最新状态快照；generation 随事件变化 |
| `tg_wifi_deinit` | 停止 Wi-Fi、注销本组件回调、销毁驱动与 STA 网卡 |

状态区分 connecting、disconnecting、associated 与 has_ip。RSSI 仅在 associated 时有效。
只有 has_ip=true 才表示获得 IPv4，仍不代表云端可达；IP 以四字节网络顺序传出。
连接失败通过 disconnect_reason 报告 SDK 原因码，不在驱动内自动重连。
共享默认事件循环与 esp-netif 运行时不会被组件销毁，便于后续其他网络服务复用。

扫描回调取回 SDK 结果并释放 SDK 列表，再保存固定容量快照。
total_aps 表示发现数，stored_aps 是保留数；新扫描替换旧结果。
scan_ready=true 后需检查 last_error，然后才能读取结果；零 AP 也是正常扫描结果。
本例扫描与连接请求互斥；已连接且不在连接过渡期时允许扫描。
SSID 保留最多 32 字节和显式长度，支持非 UTF-8，日志以有损字符串显示。
Wi-Fi 状态是最新快照，中间事件可合并，不能作为逐条可靠的事件审计日志。

连接支持空密码的开放网络，或 8..63 字节密码的 WPA2/WPA3 Personal 网络。
不支持 WEP、企业认证、64 字符原始 PSK；SSID 必须为 1..32 字节。
配置为 RAM 存储；C 同步复制凭据，不记录密码。

Rust 核心接入示例（在获取真实配置后调用，不要把实际密码提交到仓库）：

```rust,ignore
let mut wifi = platform::Wifi::new()?;
wifi.connect(ssid_from_config, password_from_config)?;
let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
loop {
    let status = wifi.status()?;
    if status.has_ip { break; }
    if !status.connecting && !status.associated && status.disconnect_reason != 0 {
        anyhow::bail!("association failed: {}", status.disconnect_reason);
    }
    if std::time::Instant::now() >= deadline {
        wifi.disconnect()?;
        anyhow::bail!("connection/DHCP timeout");
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
}
// 读取 status.ipv4、status.rssi，交给网关核心使用。
wifi.disconnect()?;
// 或直接 shutdown，内部停止驱动；Drop 为错误路径提供尽力清理。
wifi.shutdown()?;
```

## 温度和系统信息

`tg_temperature_init(min,max)` 安装并启用传感器，SDK 校验测量区间。
Rust 示例请求 -10..80℃ 范围；`read` 返回真实芯片温度，越界和底层错误直接传播。
`deinit` 先禁用再卸载，失败时保留可重试的状态。单任务独占，不允许重复初始化。
芯片自热受无线与 CPU 负载影响，不把该值映射为外部环境传感器读数。

`tg_system_get_info` 无需初始化，返回 esp_chip_model_t、SDK 修订号编码、
esp_reset_reason_t、微秒时钟转换的毫秒运行时间及内存统计。
各项内存数据是分别采样的诊断快照，不保证跨字段瞬时一致。

## 构建和待上板验收

本工程当前使用的 esp-idf-sys 增量构建未自动跟踪自定义 C 源文件/头文件。
修改 components 内的源码、头文件、CMake 或 SDK 配置后，先运行下面的
定向清理命令，再构建，以免链接旧 C 代码或使用旧绑定；仅改 Rust 时不必清理。

```powershell
cargo clean -p esp-idf-sys
cargo build
cargo fmt --check
rustc --test src/ble_ad.rs -o target/ble-ad-tests.exe
./target/ble-ad-tests.exe
# 接板并确认串口后烧录：
cargo run
```

本次未自动烧录；以下项目必须上板验证：

| 项目 | 验收 |
| --- | --- |
| 系统 | 芯片确认为 C6，复位原因合理，运行时间递增 |
| 温度 | 可读并能随负载变化；非法区间/未初始化读取返回错误 |
| Wi-Fi 扫描 | 能发现已知 2.4GHz 热点，结果数不超过 20，重复扫描可用 |
| Wi-Fi 连接 | 正确密码获得 IP 和 RSSI；错误密码/无 AP/无 DHCP 可结束等待 |
| Wi-Fi 断开 | 清空 IP，等待断开后可重连；运行中拔掉热点可观察断线 |
| 资源释放 | 各模块初始化/停止后可重新初始化，多轮测试内存不持续下降 |
| BLE | 原扫描报告、名称、完成与丢弃统计仍可用 |

尚未实现：LED、BOOT、Zigbee 接入、BLE GATT 控制、NVS 业务存储与 USB 命令终端。
旧 c_drivers 的电池/SD 数据仍是占位值，主程序不调用它们。

参考官方接口：
- https://docs.espressif.com/projects/esp-idf/en/stable/esp32c6/api-reference/network/esp_wifi.html
- https://docs.espressif.com/projects/esp-idf/en/stable/esp32c6/api-reference/peripherals/temp_sensor.html
