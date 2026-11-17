# Wi-Fi / BLE Central / RGB 联调（2026-11-16）

引脚来源与占用见 `materials/pinout-and-usage.md`。OLED SDA20/SCL19、按键 GPIO9，
单颗 RGB 数据脚 GPIO8；GPIO15 普通 LED 本次不驱动。SD 和 UPS 暂缓，仍为占位代码。

## 构建模式

```powershell
# 修改 C 后刷新自动生成绑定
cargo clean -p esp-idf-sys
cargo build --no-default-features --features connectivity-demo
cargo run --no-default-features --features connectivity-demo
```

串口命令台运行 Wi-Fi STA 与 BLE Central，不启动 Zigbee。默认 `cargo run` 仍为 Zigbee 模式。
串口 115200，输入 UTF-8 文本并回车；输入帮助 `help`。单行上限 256 字节，超长行丢弃。
不自动连接扫描到的设备，不自动保存网络密码。CLI 仅本地联调工具，不是云端或小程序接口。

## Wi-Fi

```text
wifi scan
wifi status
wifi connect My WiFi|password123
wifi disconnect
wifi save My WiFi|password123
wifi saved
wifi forget
wifi ap TinyGate-Setup|setup12345
wifi ap-off
```

`connect` 异步提交，日志分别报告 associated 与 has_ip；获得 IP 不等于云端可达。
`save` 显式保存一个版本化 NVS blob，`saved` 读取并连接，`forget` 删除该记录。
SSID 和密码用第一个 `|` 分隔，支持 SSID 内空格；SSID 不能含 `|`，密码允许含 `|`。
记录默认存于未加密 NVS，代码不打印密码，串口终端仍可能自行回显或保存输入。
开放 STA 网络允许空密码；WPA2/3 Personal 密码 8..63 字节。无 WEP/企业认证/原始 PSK 接口。

SoftAP 使用 WPA2、信道 1、最多 4 个客户端，只允许 STA 未连接且无进行中的扫描/连接时启动。
先停止射频再配置 AP，配置完成后启动，避免默认开放 AP 短暂广播。之后可以在 APSTA 下发起 STA 连接。
这只是配网接入能力，未实现 HTTP 表单、DNS captive portal、小程序配网协议。
APSTA 下实际 AP 信道跟随已连接的 STA。AP 停止保留网卡资源，到 deinit 一并释放。

Rust `Wifi::connect_with_retry` 是显式有界策略：每次连接/获取 IP 最多 20 秒，
最多 1..5 次，退避 0.5、1、2、4 秒；失败返回错误。默认 CLI 使用非阻塞 connect，
防止等待重连时阻塞其他事件。断线原因交给网关核心决定是否再连接，不无限重试。

## BLE

```text
ble scan 10
ble status
# 等扫描完成，使用日志里的地址类型（0/1/2/3）与地址
ble connect 1 AA:BB:CC:DD:EE:FF
# 连接成功事件 kind=1 给出 conn；下面 0 只是示例，必须替换
ble services 0
ble chars 0 0x0001 0x0020
ble descs 0 0x0010 0x0015
ble read 0 0x0010
ble write 0 0x0010 01
ble subscribe 0 0x0011 1
ble subscribe 0 0x0011 0
ble disconnect 0
ble cancel
```

必须使用实际发现的 handle/range：characteristic 事件的 handle 是值句柄，end 是声明句柄；
service 的 handle/end 是服务范围。descriptor 的扫描 start 为 characteristic 值句柄，
end 要在下一个 characteristic 声明之前，或服务末尾；不能把别的特征 CCCD 拿来订阅。
发现 UUID 0x2902 后，`subscribe` 模式 0/1/2 分别关闭/Notify/Indicate。

支持最多两个连接；先结束扫描再连接。每条连接同时仅一个 GATT 操作，等待 kind=7 完成后再发下一个。
kind：1连接、2断开、3服务、4特征、5描述符、6读取值、7操作完成、8通知。
服务、特征、描述符均包含 UUID 字符串；异步结果含 request_id 与原生 NimBLE status。
读取与写入不代表执行器业务成功；需要根据设备协议检查通知或回读。
调用立即失败会返回错误；已接受的操作依靠 NimBLE 完成/超时回调，业务仍应设置自己的截止时间。
重连使用原地址及类型重新 connect，重新发现句柄；不自动重放执行器写入。

GATT 控制/数据队列 32 项，与广播缓存分开；每项最多 128 字节，超长显式 truncated。
`get_links` 返回句柄快照和丢弃数；溢出或 Host reset 后高层应重新同步/发现，不能假设所有通知都收到。
普通 ATT read/write，默认 MTU 限制仍生效；未实现长写、长读拼接、配对/绑定、BLE Peripheral。
当前用于无配对要求的模拟 GATT 设备；需要加密的特征会返回协议错误，不绕过访问权限。

同级独立工程 `../TinyGate-BleNode-S3` 已提供配套 S3 模拟执行器，支持开关/翻转、命令去重、
20B 状态通知及可调周期。双节点连接步骤、故障恢复和分阶段负载见
[稳定性测试方案](coexist-test.md)。该对端已构建并通过协议主机测试，上板联调和三模自动统计仍待完成。

## RGB

`tg_rgb` 独立 C 组件，Rust `Rgb` 提供独占封装。
假设 GPIO8 对应一颗 WS2812 兼容 GRB 灯珠，不是三路普通 PWM LED。
使用 10MHz RMT，24bit GRB + 300us 低电平复位，完整帧小于 RMT 64-symbol 缓冲。
数据保持到同步发送完成，100ms 等待超时会尝试停止通道；停止失败锁定驱动直到清理。
`set(r,g,b,brightness)` 各参数 0..255，按全局亮度缩放；初始化和释放时熄灯。

```text
rgb 255 0 0 24
rgb 0 255 0 24
rgb 0 0 255 24
rgb 0 0 0 0
```

默认启动低亮蓝色。Zigbee 模式：等待节点蓝色、在线绿色、测试失败红色；
颜色是 Rust 业务选择，C 只发送颜色。器件/颜色顺序尚需实际灯珠验证，不能通过 RMT 返回成功判断灯已亮。

## 验证边界

必须上板验证：Wi-Fi 正确/错误密码、DHCP 超时、热点断开重连、AP 手机接入、NVS 跨重启；
BLE 两设备连接/发现/读写/通知/断连/重连、失联和队列溢出；RGB 红绿蓝与熄灭。
本次不主动烧录、不更改已保存网络、不创建 AP，默认固件不自动连接陌生 BLE 设备。
多协议共存与配套 S3 GATT 固件端到端执行仍需单独联调，编译通过不代替这些测试。
