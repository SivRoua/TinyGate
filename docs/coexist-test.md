# Wi-Fi / Zigbee 稳定性测试分支

分支 `codex/coexist-tests`，基于开发提交 `9c128a0`。本阶段不修改 S3 固件，
**BLE 不启动、不扫描，输出 ble_tested=false。不能将本轮结果描述为三模稳定性已通过。**
先验证一块主 C6 + 一块副 C6 + 路由器 + 有线连接电脑的 Wi-Fi/Zigbee 测试基础设施。
后续有 BLE 外设负载发生器后，再在独立测试模式中扩展受支持的并发角色组合。

## 文件与工程

- `src/coexist.rs`：专用有限时长测试主循环；不使用四轮后结束的原演示逻辑。
- `src/coexist_stats.rs`：64 包滑窗去重/乱序/缺失与固定桶延迟直方图，带主机测试。
- `tools/coexist_echo.py`：电脑 UDP 回显端，仅响应指定测试格式。
- `tools/capture_coexist.py`：串口结构化采集，不记录命令与密码，输出 JSONL。
- `tools/summarize_coexist.py`：按 run 汇总，计算直方图分位数上界，不自动判定 PASS。
- `tools/package_node.py`：打包同级独立副板源码供队友使用，排除 build、SDK 依赖和本机 sdkconfig。
- `../TinyGate-ZigbeeNode`：独立工程已升级测试遥测，无 Git 仓库，不在本仓库提交范围。

构建：

```powershell
cargo build --no-default-features --features coexist-test
# 确认串口后手动烧录，本次开发不自动烧录
cargo run --no-default-features --features coexist-test
```

修改 C ABI 后需 `cargo clean -p esp-idf-sys` 再构建，防止使用旧绑定。
最终 SDK 配置检查 `CONFIG_ESP_COEX_SW_COEXIST_ENABLE=y`；记录 SDK/库版本、信道、供电、位置和天线方向。
原有 `zigbee-demo`、`connectivity-demo` 仍保留；不把实验模式作为生产默认模式。

## 副板升级

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

## 测试流程

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

## 采集与指标解释

先输入 start，退出并释放原监控器串口，再启动采集（同一串口不能被两个程序同时占用）：

```powershell
python tools/capture_coexist.py COM8 --seconds 3650 --output test-results/run103.jsonl
python tools/summarize_coexist.py test-results/run103.jsonl
```

输出文件使用独占创建，避免覆盖旧记录。副板可在队友电脑用同一采集工具记录 NODE。
电脑时间仅用于粗略对齐日志，不作为单向射频延迟。采集中间断开会导致统计覆盖不完整。

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

推荐先做 30 分钟基线、每档 1 小时，再跑 24 小时。自行确定验收阈值，保留样本数量和原始日志。
本轮只验证 Wi-Fi/Zigbee；BLE 连接/通知、三模总负载及 S3 测试设备均未覆盖。
