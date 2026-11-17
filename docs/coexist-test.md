# Wi-Fi / BLE / Zigbee 分阶段稳定性测试方案

更新：2026-11-17。测试分支 `codex/coexist-tests`，基于开发提交 `9c128a0`。
目标是评估单 C6 网关共享射频资源时的业务可靠性，并为是否拆分无线芯片提供数据。
测试按单协议、双协议、三协议逐步增加负载，分别记录功能正确性、稳定性和负载边界。

## 当前实现与测试边界

独立 S3 BLE 测试节点工程已建立，固件构建与协议主机测试通过；尚未完成 S3 上板及网关联调。
**当前 `coexist-test` 仍不启动、不扫描 BLE，输出 `ble_tested=false`。**
S3 对端完成不代表网关三模测试程序完成，也没有三模稳定性已通过的结论。

| 阶段 | 网关固件 / 对端 | 当前可执行范围 |
| --- | --- | --- |
| Zigbee 基线 | `coexist-test` + 副 C6 | 已有定时控制、遥测序号及 COEX 统计 |
| Wi-Fi + Zigbee | `coexist-test` + 副 C6 + UDP 回显电脑 | 已有受控双向 UDP 流量与延迟统计 |
| BLE 单 / 双节点 | `connectivity-demo` + 1 / 2 块 S3 | 可手动发现、连接、读写和订阅；没有自动 BLE 汇总 |
| Wi-Fi + BLE | `connectivity-demo` + 路由器 + S3 | 可检查 STA 连接、扫描等状态变化；没有该模式下的受控 UDP 压测 |
| Zigbee + BLE、三模同时承载 | 待扩展网关测试模式 + 副 C6 + S3 | 尚不能执行，需先实现统一事件消费、业务统计及负载控制 |

不同 Cargo 模式不能同时运行。切换模式需重新构建并烧录，不能把两次运行的结果拼成三模并发结果。

## 测试设备与记录

| 设备 | 职责 | 固定条件 |
| --- | --- | --- |
| 主 C6 | Wi-Fi STA、Zigbee Coordinator、BLE Central（依测试模式启用） | 同一供电、固件、天线方向；OLED SDA20/SCL19 |
| 副 C6 | Zigbee Router，模拟温度传感器和开关执行器 | 同级 `TinyGate-ZigbeeNode`，记录报告周期 |
| S3 A / B | BLE Peripheral，模拟两台开关执行器 | 同级 `TinyGate-BleNode-S3`，记录完整 MAC、node ID 和报告周期 |
| 路由器 | 2.4GHz Wi-Fi AP | 固定信道、带宽和位置，记录其他同频业务 |
| 电脑 | 有线 LAN UDP 回显、各板串口采集 | 避免用电脑无线链路额外引入干扰 |

每轮记录 run ID、主仓库提交、两个独立工程源码版本或 SHA256、SDK/库版本、实际信道、
BLE 连接参数、设备位置及距离、供电方式、测试时长、目标/实际发包率和日志路径。
S3 不使用 GPIO、板载 RGB 或外接负载，测试的是无线控制与 RAM 模拟状态，不验证真实执行器硬件。
SD、UPS 不纳入本轮。

### 共享射频的场景限制

当前使用 ESP-IDF v6.1。该版本 SDK 的 `docs/en/api-guides/coexist.rst` 说明共享射频采用时分调度，
其矩阵将 Wi-Fi STA + Zigbee Router 标为 C1（支持但性能不稳定），Zigbee Router + BLE Scan 标为 X，
而 Zigbee Router + BLE Connected 标为 Y。矩阵没有单列本项目的 Coordinator，也没有给出完整三协议组合的保证，
因此这些条目只用于识别风险，协调器实际组合仍需核对 Zigbee SDK 支持范围并实测。

后续三模程序应把发现/配网与稳态业务分阶段：先发现并建立 BLE 链接，再进入 Zigbee 稳态收发；
不要把持续 BLE 扫描作为默认负载。断线后的重扫必须作为单独状态机和实验场景设计。
在确认所用 SDK 对角色组合的支持前，不把不支持的重叠扫描场景列为必须通过的验收项。

## 文件与工程

- `src/coexist.rs`：专用有限时长测试主循环；不使用四轮后结束的原演示逻辑。
- `src/coexist_stats.rs`：64 包滑窗去重/乱序/缺失与固定桶延迟直方图，带主机测试。
- `tools/coexist_echo.py`：电脑 UDP 回显端，仅响应指定测试格式。
- `tools/capture_coexist.py`：串口结构化采集，不记录命令与密码，输出 JSONL。
- `tools/summarize_coexist.py`：按 run 汇总，计算直方图分位数上界，不自动判定 PASS。
- `tools/auto_run.py`：单串口自动配置、等待 IP、启动目标 run、采集及汇总；仅用于现有 Wi-Fi/Zigbee 测试模式。
- `tools/package_node.py`：打包同级独立副板源码供队友使用，排除 build、SDK 依赖和本机 sdkconfig。
- `../TinyGate-ZigbeeNode`：独立工程已升级测试遥测，无 Git 仓库，不在本仓库提交范围。
- `../TinyGate-BleNode-S3`：独立 ESP-IDF v6.1 / C / NimBLE Peripheral 工程，无 Git 仓库；
  `main/ble_node.c` 提供 GATT 与统计，`main/protocol.h` 实现命令去重和编码，详细协议见其 README。

构建：

```powershell
cargo build --no-default-features --features coexist-test
# 确认目标串口后烧录
cargo run --no-default-features --features coexist-test
```

修改 C ABI 后需 `cargo clean -p esp-idf-sys` 再构建，防止使用旧绑定。
最终 SDK 配置检查 `CONFIG_ESP_COEX_SW_COEXIST_ENABLE=y`；记录 SDK/库版本、信道、供电、位置和天线方向。
Wi-Fi/802.15.4 还需要运行时调用 `esp_coex_wifi_i154_enable()`；`tg_zigbee` 在协议栈初始化后、
Wi-Fi 启动前执行并检查返回值，启动日志应包含 `Wi-Fi/802.15.4 coexistence enabled`。
只检查配置宏不足以确认已完成共存初始化；修改该 C 组件后必须刷新原生构建再烧录。
原有 `zigbee-demo`、`connectivity-demo` 仍保留；不把实验模式作为生产默认模式。

## Zigbee 副板配置

保留 EP1 温度和 EP2 OnOff，新增 EP1 / Temperature Measurement Cluster / 私有测试属性 0xF000，
类型 uint64，内容为 `(boot_session << 32) | sequence`。这是配套测试扩展，不是标准温度属性，
不用于生产设备互通。副板重启生成新的随机 session，序号从 0 开始。

Kconfig `TG_NODE_REPORT_MS` 范围 50..10000ms，默认 1000ms，控制测试遥测频率。
原温度和开关周期上报保持约 5 秒；开关执行后的反馈继续保留。
通过 `idf.py menuconfig` 调整频率（TinyGate simulation node），每组记录实际配置。
报告前更新属性，统计 attempted/admitted/submit_fail 与发送确认 tx_ok/tx_fail，
每秒输出 `NODE {...}`，不逐包打印测试遥测。定时任务/SDK 排队可能让实际频率低于配置。
入网失败的重试频率保持约 5 秒，不随高频遥测一起加速。

```powershell
# 在副板目录
./build.ps1
# 在 TinyGate 目录打包给队友
python tools/package_node.py
```

ZIP 默认保存在被忽略的 test-results 下，内含源码 SHA256 清单，不包括完整构建缓存。
副板项目不是 Git 仓库，因此它的修改不能被本仓库的 commit 自动保存；保留源码包供交付归档。

## Wi-Fi / Zigbee 自动测试流程（当前可执行）

### 一键执行脚本

需要 `pyserial`，主板烧录 `coexist-test`。先退出其他串口监控器；下面 Wi-Fi 信息均为占位符。

```powershell
# A：Zigbee 基线，不启动 Wi-Fi 业务
python tools/auto_run.py --port COM8 --run 101 --seconds 180 --control 10
# B：连接 Wi-Fi，等待 IP 后再开始；不自动保存凭据
python tools/auto_run.py --port COM8 --run 102 --seconds 180 --control 10 --wifi "YOUR_SSID|YOUR_PASSWORD"
# C：先在有线电脑启动 UDP 回显，再执行
python tools/auto_run.py --port COM8 --run 103 --seconds 3600 --wifi "YOUR_SSID|YOUR_PASSWORD" --udp "192.168.1.100 9000 10"
```

脚本先确认 COEX 命令台和空闲状态，已有 active run 时拒绝改配置；需要时先在串口显式 `stop`。
每次配置前关闭遗留 UDP 和 Wi-Fi，再应用 control/ui；指定 `--wifi` 后重新连接，避免复用上次连接和历史错误码。
不指定 `--wifi` 的轮次会关闭已有 Wi-Fi，用于真正的 Zigbee 基线；`--udp` 要求同时提供 `--wifi`。

板端没有逐命令成功 ACK，脚本用顺序发送的 `命令 → status → help` 响应确认命令处理顺序，
检查 `COEX command rejected` 并只使用新的状态快照。只有收到 IP 确认后才 start，默认等待 30 秒，
可用 `--wifi-timeout 60` 调整；本次连接产生非零断线原因时直接失败，不无限重试，也不开始采样。

默认独占创建 `test-results/run<ID>.jsonl`，已有文件绝不删除或覆盖；重复测试请换 run ID 或指定新 `--output`。
配置失败可能留下空文件；采集中断保留部分数据并尝试停止本脚本已确认启动的轮次。
只有目标 run 的 active 快照和达到指定时长的结束快照均被确认，脚本才报告完成。
换 run、计时回退、重启、长时间无 COEX 或提前停止均报“不完整”，不输出 PASS。
文件仅保存目标 run 的结构化 COEX，不写入原始命令或 Wi-Fi 凭据；目前仍不支持 BLE/S3 自动统计。

`reason=201 / NO_AP_FOUND` 表示本次没有找到目标 AP，不足以确定热点未启用 2.4GHz。
先核对 2.4GHz、SSID 大小写、热点是否持续开启、距离和信号；安全模式不兼容可能有 210/211 等原因码。
诊断更新后的 `coexist-test` 可直接输入 `wifi status` 查看连接/扫描状态和底层错误，
输入 `wifi scan` 后等待 `WIFI_SCAN complete`，再查看 `WIFI_AP` 的 SSID、信道、RSSI 和 auth 数字。
此为异步诊断，正常自动测试不会主动扫描；在独立故障排查轮运行，避免改变稳态采样负载。
驱动最多保留 20 条 AP，`total > stored` 时列表被截断，未显示目标不能证明完全未扫描到它。
auth 来自 SDK 的 `wifi_auth_mode_t`，例如 3 为 WPA2-PSK、6 为 WPA3-PSK、7 为 WPA2/WPA3 混合。
旧固件不打印这些结果，需要更新诊断固件，或另用 `connectivity-demo` 的 `wifi scan`。
若在已确认信道 1..11、可见的 WPA2 路由器附近，扫描成功却始终返回 0 个 AP，
先核对运行时共存初始化和扫描输出，再做 Wi-Fi 单独运行与 Zigbee 共存的对照；不能仅归咎于 SSID 或密码。
原因码含义可对照 [ESP-IDF Station Scenarios](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c6/api-guides/wifi-driver/station-scenarios.html)。

以下 A..D 为手动执行方式；自动脚本运行时不要再用另一个程序占用串口或重复发送 start。

主板启动只运行 Zigbee，60 秒入网窗口；未发送命令前不初始化 Wi-Fi。
默认关闭持续 OLED 刷新，RGB 不动画。按键仍可打印少量用户事件，正式长测不要频繁按键。

### A：Zigbee 基线

串口输入：

```text
join 60
control 10
ui off
start 101 1800
```

每 10 秒发送一次与实际状态相反的 On/Off，最多一个在途控制；每个 2 秒截止，
需要发送确认、匹配 TSN 的 ZCL 响应以及期望状态反馈三项均满足才统计成功。
每条请求是新的测试请求，不因失败立即重放。没有在线节点则不发送控制。
2 秒业务截止不会取消 SDK 请求。控制超时后保留在途状态，等待旧请求收尾后再发新请求；
晚到结果单列，不能补算按时成功。逐事务日志、双串口诊断和回归记录见
[双 C6 控制链路诊断](control-diagnostics.md)。

### B：Wi-Fi 已连接但无数据业务

```text
wifi connect Your SSID|YourPassword
start 102 1800
```

上一轮必须先结束或 `stop`，每组用不同 run ID；命令不会自动保存凭据。
确认 COEX 中 wifi_has_ip=true 后再开始正式采样。密码不要写入提交或采集文件。

### C：Wi-Fi 受控双向流量 + Zigbee

电脑打开有线 LAN 的 UDP 回显服务，按需放行本机防火墙端口：

```powershell
python tools/coexist_echo.py --port 9000
```

网关串口：

```text
udp 192.168.1.100 9000 10
start 103 3600
```

将 IP 换为电脑的 LAN 地址。每个 UDP 数据报固定 256B，含 run、序号、本地时间戳和校验填充，
回包必须匹配来源、序号及时间戳。仅使用指定局域网端点，速率范围 1..1000 包/秒，
使用 10/50/100 等档位递增。实际发送速率由 udp_sent/elapsed 计算，不把配置速率当实测值。
在途上限 256；满队列和本地发送失败计 udp_error，不混入网络超时。每包 2 秒截止。
RTT 由主板自己的单调时钟计算，不相减两块板的时钟。

### D：状态变化与应用负载对照

- 运行中 `wifi scan`，观察 Zigbee 和 UDP；一次只注入一种变化。
- 断开路由器 30 秒后恢复，通过 `wifi connect` 显式重连；当前测试台不无限自动重试。
- 副板掉电重启，检查 session 变化、控制超时及后续报告恢复。
- 下一轮 `ui on` 与 `ui off` 对照，区分应用消费阻塞与无线问题。
- `wifi off` 停止 Wi-Fi，`udp off` 关闭流量；`control 0` 停止控制；`stop` 停止当前采样。
- 每轮最长 86400 秒，到期停止发包，继续运行协议栈与输出最后计数供采集。

强信号固定位置先完成基线；再分别改变距离、Wi-Fi 信道和 Zigbee 信道。
Zigbee 持久化网络可能使用已有信道，不能只改编译参数就认定实际切换成功。

## S3 BLE 测试流程（当前手动执行）

### E：准备两块 S3 与单节点功能验证

先在独立工程目录构建，再按实际串口分别烧录。下面 COM7 / COM9 仅为示例；烧录覆盖板上原程序。

```powershell
cd C:\coDoc\lesson\IoT\TinyGate-BleNode-S3
.\build.ps1
.\build.ps1 -Action flash -Port COM7
.\build.ps1 -Action flash -Port COM9
.\build.ps1 -Action monitor -Port COM7
```

同一固件可用于两块 S3，广播名为 MAC 派生的 `TinyGateS3-XXXXXX`；默认 node ID 为 MAC 后两字节，
不能代替完整 MAC 作为唯一身份。若需固定 ID，在 `menuconfig → TinyGate S3 BLE test node` 分别设置 1、2，
各自重新构建烧录。默认报告周期 1000ms，允许 50..10000ms，不启用 PSRAM。
默认 UART0 控制台；只有原生 USB 的开发板需在 menuconfig 选择 USB Serial/JTAG 控制台，详见节点 README。
先确认两块板均正常广播并输出 `S3NODE {JSON}`，记录各自 session。

主网关改用以下固件；该模式不启动 Zigbee：

```powershell
cd C:\coDoc\lesson\IoT\TinyGate
cargo build --no-default-features --features connectivity-demo
cargo run --no-default-features --features connectivity-demo
```

先只连接一块 S3。GATT 自定义服务 UUID 为 `54470001-1234-4abc-8000-0123456789ab`，
控制、状态、周期、统计 UUID 首段依次为 `54470002`..`54470005`，其余部分相同。
状态支持 Read/Notify，控制使用带响应的 Write；无需配对。每次重连重新发现句柄，不使用固定句柄。

```text
ble scan 10
ble connect ADDRESS_TYPE AA:BB:CC:DD:EE:FF
ble services CONN
ble chars CONN SERVICE_START SERVICE_END
ble descs CONN STATE_VALUE_HANDLE STATE_DESCRIPTOR_END
ble subscribe CONN STATE_CCCD_HANDLE 1
ble read CONN STATE_VALUE_HANDLE
ble write CONN CONTROL_VALUE_HANDLE 0101000001000000
ble write CONN CONTROL_VALUE_HANDLE 0102000002000000
ble write CONN CONTROL_VALUE_HANDLE 0102000002000000
ble read CONN STATE_VALUE_HANDLE
ble read CONN STATS_VALUE_HANDLE
```

替换所有大写占位符，地址类型来自扫描结果；扫描结束后再连接。CCCD 为 0x2902，描述符范围不能跨入下一特征。
网关 `kind=1` 给出连接句柄；每次 GATT 操作等待 `kind=7` 完成再发下一条，通知为 `kind=8`。
CLI 的 `req` 是网关 GATT 操作编号，负载中的 request ID 是执行器命令编号，两者分别关联，不能混用。

以上三次 Write 的业务含义为 request=1 开、request=2 翻转、重复 request=2 翻转。
预期执行器最终关闭，实际执行次数只增加 2；最后一次回执标记为重复且不再改变状态。
命令 request ID 每节点独立递增，不能因重连直接从 1 重用；S3 保留最近 8 个 ID，缓存淘汰或重启后不保证去重。

业务控制成功需要 ATT 操作成功，并收到匹配该 MAC/session/request ID、结果符合预期的回执。
回读可核实最终状态，但当前值可能已被后续命令改变，不应代替丢失的历史回执计算控制成功率。
断线后设备重新广播；重连重新发现和订阅。单纯断线不会清除模拟状态或去重缓存，掉电重启则恢复关闭并生成新 session。

### F：双节点负载、Wi-Fi 状态变化与恢复

完成单节点功能验证后，在同一网关上连接 S3 A、B，按各自句柄发现并订阅。
每节点单独维护序号、session 和业务 request ID，不交错使用另一节点的 CCCD 或控制句柄。

| 档位 | BLE 节点数 | 每节点周期 | 名义周期通知总量 | 建议时长 |
| --- | --- | --- | --- | --- |
| E1 | 1 | 1000ms | 1 条/秒 | 功能验证至少 10 分钟 |
| E2 | 2 | 1000ms | 2 条/秒 | 基线 30 分钟 |
| E3 | 2 | 100ms | 20 条/秒 | 先 10 分钟，再按结果延长至 1 小时 |
| E4 | 2 | 50ms | 40 条/秒 | 边界探索，先 10 分钟 |

修改周期使用 `ble write CONN INTERVAL_VALUE_HANDLE HEX`：`e803` 为 1000ms，`6400` 为 100ms，
`3200` 为 50ms。这些只计算周期通知，命令回执还会增加通知量；实际速率以两端统计为准。
每节点每 10 秒下发一个新 ID 的开/关命令；目前需要手动操作并记录，长时间定量测试应等自动调度接入后执行。

先不连接 Wi-Fi，再在 `connectivity-demo` 内执行 `wifi connect SSID|PASSWORD`，
确认 `has_ip=true` 后保持相同 BLE 负载对照。该模式仅验证 Wi-Fi 连接状态下的 BLE，
没有 `coexist-test` 的 `udp` / `start` 命令，不能据此声称通过了 Wi-Fi 满载共存测试。

逐轮单独注入：Wi-Fi 扫描、路由器断电 30 秒并恢复、S3 A 断电、S3 B 断开重连。
观察另一个 BLE 节点是否继续上报、GATT 是否超时、队列丢弃是否增加，以及恢复是否需要手动操作。
当前网关不自动重连 Wi-Fi / BLE，应显式重新连接、发现和订阅；分别记录检测时间、人工干预和恢复时间。
反复执行 20 次断线/重连，作为初步功能检查；自动恢复能力需后续状态机实现后另测。

## 三模测试扩展计划（尚未实现）

先增加 Zigbee + 已连接 BLE 的双模对照，再叠加 Wi-Fi STA；所有组合保持相同位置、周期和控制节奏。
建议矩阵如下，先逐项跑通再做组合，不同时提升多个负载维度：

| 组别 | Zigbee 测试遥测 | BLE 负载 | Wi-Fi 业务 | 建议时长 |
| --- | --- | --- | --- | --- |
| G1 | 1000ms | 1 节点 × 1000ms | 关闭 | 30 分钟 |
| G2 | 1000ms | 2 节点 × 1000ms | STA 连接、无主动数据 | 30 分钟 |
| G3 | 1000ms | 2 节点 × 1000ms | UDP 10 / 50 / 100 包/秒逐档 | 每档 1 小时 |
| G4 | 1000ms → 100ms | 固定 2 节点 × 1000ms | 固定在 G3 已通过档位 | 每档 1 小时 |
| G5 | 固定在已通过档位 | 2 节点 × 100ms → 50ms | 固定在 G3 已通过档位 | 每档先 10 分钟，再延长 |
| G6 | 选定生产目标周期 | 选定生产目标周期 | 选定生产目标流量 | 连续 24 小时 |

UDP 继续使用 256B 请求及回显。正式稳态采样期间不扫描、不入网、不刷高频 OLED、不做 RGB 动画；
扫描、掉线、重新入网、UI 负载和信道变化分轮注入，作为状态转换对照实验。
两对端工程均不包含自动修改网关测试模式的能力，G1..G6 在下列工作完成前只作为计划：

1. 在一个网关模式内初始化三套协议栈并串行消费事件，核对实际 SDK 角色组合和共存配置。
2. 按 MAC + session 跟踪两条 BLE 链接，解析 20B 协议包；实现周期配置、订阅确认、控制调度和 2 秒截止。
3. 分开记录周期与回执数量，但用共享通知序号去重/统计缺口；Read 返回值不计通知接收数。
4. 增加 BLE 控制成功/超时/协议错误、通知缺失/乱序/重复、队列丢弃、连接变化及延迟直方图。
   GATT 写响应和业务回执可能交错到达，必须按业务 ID 关联，不依赖到达顺序。
5. 明确 start/stop、断连、重启时在途请求及统计窗口的处理，队列消费须有预算，避免一个协议饿死其他协议。
6. 采集器加入 `S3NODE`，汇总器加入 BLE 及会话分组；程序仅在真正启用 BLE 的对应模式标识其覆盖状态。

### BLE 指标定义与初步判定

S3 通知固定 20B、适配默认 ATT MTU 23：版本/类型在偏移 0/1，node ID 在 2，session 在 4，
通知序号在 8，业务 request ID 在 12，状态在 16，回执标记在 17，多字节均为小端。
类型 1 为周期、2 为命令回执、3 为 Read。回执标记 0 为新执行、1 为去重；重复命令回执保留原执行结果。

- 序号只在已订阅的通知尝试时递增，周期和回执共享；本地入队失败也消耗序号。
  因此端到端通知缺口要同时对照 S3 rejected、网关 GATT dropped 和断线区间，不能直接称为空口丢包。
- S3 的 attempted = accepted + rejected；tx_ok / tx_fail 属于发送回调计数，不代表网关应用收到了通知，
  也不能与 rejected 相加当成互斥的失败总量。GATT 统计 Read 返回 attempted、accepted、rejected、tx_fail、commands。
- 每个 MAC/session 分开统计；session 变化切断旧窗口。Read 不推进序号，不能填补通知缺口。
  首包前、末包后、退订或断线区间须注明统计覆盖范围；不能用静默期间没有新序号推断零丢包。
- 同一连续订阅、同一 session 的有效窗口内，通知接收率为 `unique / span`，其中 span 为首末序号包含的尝试数，
  unique 为有效、去重后的通知数；允许乱序补齐后再结算。分子包含周期及回执，不能只统计周期包再用共享序号作分母。
  完全没有收到通知时标记“无有效窗口 / 业务中断”，不能报告 100%。两节点分别给结果，不以总量掩盖单节点停报。
- 控制 RTT 应由网关单调时钟，从提交业务命令到收到匹配回执计算；不能相减 S3 与 C6 的启动时间。
  S3 实际 commands 增量用于检查重复执行，手动串口日志只能做功能观察，不能给出精确 RTT 分位数。
  成功率分母为本轮发起的唯一业务命令数，本地未接受、超时及未完成均单列且不从分母剔除；主动去重用例另组统计。
  RTT 分位数注明是否仅含成功请求，并同时公布超时比例，避免仅凭成功样本的低延迟判定通过。

以下为建议的首轮工程目标，尚非既定产品验收标准；执行前在运行记录固定阈值，压测后不得按结果回改：

| 项目 | 正常负载建议目标 | 判定条件 |
| --- | --- | --- |
| 可用性 | 无意外复位、看门狗、死锁；稳态无意外断连 | 排除有意注入的故障轮，保留全过程日志 |
| 遥测 | 每协议端到端有效通知/报告接收率 ≥99.9% | 按有效窗口和不同 session 统计，说明首尾盲区 |
| 控制 | 成功率 ≥99%，无重复执行；P99 ≤2 秒 | 每节点至少 100 个唯一命令，未完成单列；超时不算成功 |
| 本地拥塞 | 正常负载期间本地提交失败及网关队列丢弃增量为 0 | 首尾快照完整且未跨重启；否则只能标记未知 |
| 资源 | 无持续堆下降，恢复后回到可重复的稳态范围 | 记录周期采样与低水位能力；当前每秒 heap 不代表瞬时最低 |

首次硬件联调先判定功能闭环；未接入自动统计时不出具上述数值通过结论。
50ms 等高负载档用于找边界，需标明是否属于产品需求，不直接沿用正常负载门槛宣称产品失败。
正式报告同时附 Wi-Fi UDP 的成功率、超时、本地错误及 RTT P95/P99，比较各组相对基线的变化。

若正常目标负载在可重复环境中持续不达标，先排除供电、串口日志阻塞、队列消费不足和信道干扰，
再以相同用例对比拆分方案。双 C6、外挂 ESP32 / ESP32-S3 / ESP8266 是候选中的单一替代方案，
按实际协议能力分配 Wi-Fi / BLE / Zigbee，不能假定所有候选都能承担 BLE。保留原单芯数据作对照。

## 采集与指标解释

先输入 start，退出并释放原监控器串口，再启动采集（同一串口不能被两个程序同时占用）：

```powershell
python tools/capture_coexist.py COM8 --seconds 3650 --output test-results/run103.jsonl
python tools/summarize_coexist.py test-results/run103.jsonl
```

输出文件使用独占创建，避免覆盖旧记录。副板可在队友电脑用同一采集工具记录 NODE。
电脑时间仅用于粗略对齐日志，不作为单向射频延迟。采集中间断开会导致统计覆盖不完整。

**当前采集器仅识别 `COEX` / `NODE`，会忽略 `S3NODE` 和网关的 GATT 文本事件；汇总器只汇总 COEX。**
S3 阶段请使用串口终端分别保存两块 S3 的原始日志，并保存网关 GATT 日志；不能用现有 JSONL 汇总声称统计了 BLE。
同一串口仍只允许一个程序占用。保存网关原始日志时暂停或脱敏记录 Wi-Fi 凭据输入，避免将密码写入归档。
S3 与网关累计值按同一启动 session 取首尾差，不将每秒快照相加；若开关串口触发复位，应重新划分会话和采样窗口。

- COEX 为每秒累计快照，不把多个快照重复相加。
- 序号仅统计运行中实际观察到的窗口；第一包之前及最后一包之后的丢失无法推断。
- 64 包窗口内迟到包可以填补缺口，重复不增加成功数。超窗包计 too_old，不能精确去重。
  保存最近 8 个已结束 session 以过滤重启前迟到包，更多连续重启需分组重新测试。
- missing 是端到端序号缺失，包括副板提交失败、无线损失、网关队列丢弃，不能直接称为纯空口丢包。
  对照 NODE 的 submit_fail/tx_fail 与网关 queue_drop_total 定位原因。
- queue_drop_total 是启动以来累计数，对比采集首尾差值；若错过运行起点，无法断言整个运行无丢弃。
- UDP/控制在 stop、到期、切换节点或显式关闭时尚未完成的请求计 unfinished，不伪装成功或超时。
- 延迟桶上界为 1/5/10/25/50/100/250/500/1000/2000/5000ms 与 >5000；P95/P99 是桶上界，不是精确原始值。
- 没有副板报告或没有足够控制样本时，不能根据零失败判定通过。
- heap 是每秒采样值，不代表记录到了瞬时最低点。

推荐先做 30 分钟基线、每档 1 小时，再跑 24 小时，保留样本数量和原始日志。
结论按“已构建 / 已功能联调 / 已完成指定时长与负载实测”分别填写。
当前已有下述 Wi-Fi/Zigbee 60 秒实测，S3 上板、UDP 负载长测及三模并发仍未完成；
不得把 S3 对端已交付或短时连通结果写成三模稳定性实测通过。

## 已执行的连接故障排查与短测（2026-11-17）

环境：COM8 的 ESP32-C6，ESP-IDF v6.1、Zigbee 库 2.1.0；路由器 `Reacher_net`，
2.4GHz 信道 6、WPA2 个人版，UI 关闭。工作树基于 `5d90866`，包含尚未提交的修复。

- 修复前：Wi-Fi 连接返回 201，异步扫描成功但 `total=0 / stored=0`。
- 原因与修复：遗漏 SDK 运行时 `esp_coex_wifi_i154_enable()`，原先仅配置了共存宏。
  已在 Zigbee 初始化后、Wi-Fi 启动前调用并检查结果，更新到主 C6 验证。
- 相同条件下修复后：扫描发现目标 SSID，信道 6、RSSI -26dBm、auth=3（WPA2-PSK），随后成功获得 IP。
- run 102：指定 60 秒，结束快照 `elapsed_ms=60002`，共 61 份快照均为 `wifi_has_ip=true`。
  Zigbee `unique=59 / span=59 / missing=0`，控制 `5/5` 成功、最大 298ms，无失败/超时/未完成，
  网关队列丢弃增量 0。缺口统计仅覆盖实际观察窗口，不代表完整 60 秒内所有发端尝试均已核对。
- 本次未发送 UDP，未启用 BLE，不能据此判定高负载、长期或三模稳定性通过。

原始记录：`test-results/run102-coex-fixed-20261117.jsonl`；扫描对照：
`test-results/wifi-scan-coex-fixed-20261117.json`；摘要及固件/源码哈希：
`test-results/wifi-coex-fix-20261117-summary.json`。这些本地结果被 Git 忽略，交付时应另行归档；旧结果未覆盖。

## 分工与交付记录

| 负责人 | 本轮职责 | 交付内容 |
| --- | --- | --- |
| 硬件与 C 驱动（你） | 两 C6 / 两 S3 供电与连接、C 驱动接口、BLE / Zigbee 模拟对端、板端统计 | 可烧录对端源码、引脚及供电记录、两端原始日志、异常复现步骤 |
| Rust 网关核心 | 三模统一测试模式、连接状态机、事件预算、命令关联及超时、端到端统计 | 自动测试固件、采集与汇总工具、逐组运行记录 |
| 云端 / 前后端 / 小程序 | 测试结果归档及业务侧观测；射频基线完成后再接入云链路 | 结果展示、业务控制日志、局域网与云端路径对照 |

射频基线先使用本地 UDP 回显及模拟执行器；云端延迟与业务排队后续单列，避免混入无线指标。
两个对端工程均在主仓库之外且没有 Git，本仓库提交不自动保存其源码；归档时分别保留源码快照或哈希清单。
现有 `tools/package_node.py` 只打包 Zigbee 副板，不能直接用于 S3。

每轮建议使用以下记录模板；未执行的字段明确填“未测”，不填 0：

```text
run / 日期 / 操作者：
组别与目标阈值 / 是否实际启用 Wi-Fi、BLE、Zigbee：
网关提交与构建 feature / SDK 与库版本 / 对端源码哈希：
板型、MAC/EUI64、session / 信道、连接参数 / 供电、距离与天线方向：
目标与实际速率 / 控制样本数 / 有效采样时长与首尾快照覆盖：
遥测缺口、重复、乱序 / 本地提交失败 / 网关队列丢弃：
控制成功、失败、超时、未完成 / RTT P95、P99 / UDP 指标：
重启、断连、故障注入与人工恢复 / heap 变化：
主板、副 C6、S3 A、S3 B 日志路径：
结论、限制与复现步骤：
```
