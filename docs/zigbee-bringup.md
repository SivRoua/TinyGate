# 双 C6 Zigbee 联调

## 两个独立工程

- 当前仓库：Rust TinyGate + C `components/tg_zigbee`，协调器，OLED SDA20/SCL19。
- 同级独立项目：`../TinyGate-ZigbeeNode`，纯 ESP-IDF C，常供电 Router。
  副板不属于当前仓库，也没有初始化 Git。
- 固定 `espressif/esp-zigbee-lib ==2.1.0`，本次在现有 ESP-IDF v6.1 下编译验证。
  SDK 官方推荐基线为 v5.5.6；编译兼容不代表已经验证 6.1 下的射频/运行稳定性。
- 两板默认初始信道 15，HA profile 0x0104。恢复已有网络时使用持久化网络参数。

## 初始联调模式

默认 Cargo feature `zigbee-demo`：只启动 Zigbee、OLED 与系统信息服务，
不初始化 Wi-Fi、BLE。不是在原扫描示例之后再启动 Zigbee。

1. 网关初始化后首次建网；有历史网络时恢复，不自动擦除。
2. 网关就绪后开放入网 60 秒；之后关闭。错过窗口可重启网关重新开放，网络身份仍保留。
3. 副板每 5 秒尝试初始化/入网；加入后每 5 秒生成模拟温度并发送温度与开关状态。
4. 网关通过 Active Endpoint + Simple Descriptor 查询发现端点和服务端 Cluster，最多处理 8 个端点。
5. 对配套 EP1 温度/EP2 开关执行读取与报告配置；OLED 显示温度、开关和测试计数。
6. 自动用例等待已知状态，发送与当前状态相反的 On/Off，间隔 10 秒，共 4 轮。
   初始状态为 OFF 时，预期 ON → OFF → ON → OFF。只用于本配套模拟节点，不用于真实执行器。
7. 每轮必须同时满足发送确认成功、对应 TSN 的 ZCL Default Response 成功、
   新接收的开关属性等于目标状态。发送成功本身不算执行成功。
8. 每轮等待最多 12 秒；失败/超时立即停止自动用例，不无限重试。数据接收仍继续。
9. 15 秒没有有效属性则标记离线。只是应用层活跃判定，不等同于 Zigbee Leave。

本演示只跟踪最近一个节点，不是多设备注册表。配套节点以模拟数据证明通信闭环，
不是温度/继电器的真实测量与执行。重启网关重新运行自动用例，但不清除网络。

## 端点协议

| 设备/端点 | Cluster | 属性 | 类型/单位 | 行为 |
| --- | --- | --- | --- | --- |
| 网关 EP1 | 0x0402、0x0006 Client | 接收远端属性 | — | 控制与采集入口 |
| 副板 EP1 | 0x0402 Server | MeasuredValue 0x0000 | int16，0.01℃ | 20.00→20.50→…→25.00 循环 |
| 副板 EP2 | 0x0006 Server | OnOff 0x0000 | bool | Off/On/Toggle；执行后反馈 |

副板两个端点包含 Basic/Identify 等标准设备 Cluster，制造商 TinyGate，型号 C6-SIM1。
模拟值在副板日志以 centiC 输出；网关 OLED 换算为 ℃。
副板每 5 秒**定向单播报告到协调器地址 0、EP1**，执行后也提交一次状态反馈。
此固定接收端约定让首轮联调不依赖绑定表。

网关确实提供 Configure Reporting 请求及结果事件，但本例的固定 5 秒主动报告
不由该配置控制；它不能验证通用设备的绑定/自动报告策略。后续接入真实设备时需增加
Bind 流程，并按设备支持情况管理报告参数，不能直接假设所有设备会主动上报。

## C/Rust 职责与事件

`tg_zigbee.h` 为公开 ABI：init、status、poll、permit_join、discover、read_attribute、
send_onoff、configure_reporting。SDK 数据不直接跨 FFI，事件为定长值拷贝。
EUI64 在 JOIN/LEAVE 中有效，按 SDK 原始小端字节序保存；短地址会变化，不是稳定设备 ID。

- C：SDK 任务、建网与恢复、入网、描述符查询、ZCL 收发、事件复制。
- Rust：单一调用者、设备状态、关联结果、12 秒超时、测试策略、OLED 页面。
- SDK 回调已有协议栈上下文，不再次获取锁；外部 API 先检查 ready，再限时获取 SDK 锁。
- 不在回调刷 OLED、写文件或执行 Rust 业务。
- 事件队列 32 项，满时丢弃新事件并累计 dropped_events；关键事件丢失会让用例超时，
  不会因为发送成功就误判通过。状态快照仍可查询。
- TX_CONFIRM 携带应用 request_id 和原生 TSN；接收响应携带原生 TSN、地址、端点、Cluster。
  关联时必须包含这些上下文，TSN 只有 8 位，不能当长期全局 ID。本例同一时间仅一个开关用例。
- TX_CONFIRM.status 属于发送层；DEFAULT_RESPONSE/REPORT_CONFIG/属性响应的 status 是 ZCL 域；
  ERROR/status.last_error 可为 SDK/ESP 错误。不要混为同一错误枚举。
- 没有在线停止/销毁/重初始化接口，协议栈按固件生命周期运行，严重错误需记录后重启。

## 分区与构建

新分区表保持 factory app 起点 0x10000，另在 0x1F0000 配置独立 64KiB `zb_nvs`。
整个分区布局在 2MiB 内；普通 nvs 不与 Zigbee 网络数据混用。
正常启动绝不自动擦除 NVS；若 zb_nvs 初始化失败，保留错误并停止，检查实际分区与历史数据。
更换分区表/首次部署应使用新的完整烧录命令，不只更新 app。

Cargo 模板通过 ESP_IDF_GLOB 将 partitions.csv 复制到原生 SDK 工程，runner 显式使用该分区表。
已有本机 `.cargo/config.toml` 需包含模板中的这两项 env 和 runner，不能只依赖旧配置。
`components_esp32c6.lock` 固定 Registry 组件依赖，应与代码一起提交。

```powershell
# 网关，在 TinyGate 根目录
cargo build
# 修改自定义 C 源码/头文件之后，按当前构建链先定向清理
cargo clean -p esp-idf-sys
cargo build
# 明确选择网关串口后烧录，模板 runner 也提供等效 cargo run
espflash flash --port COM_GATEWAY --monitor --partition-table partitions.csv target/riscv32imac-esp-espidf/debug/tiny-gate

# 原板上 Wi-Fi/BLE/OLED 演示仍可构建与运行
cargo build --no-default-features
cargo run --no-default-features

# 副板，在已激活 ESP-IDF v6.1 的终端
cd ../TinyGate-ZigbeeNode
idf.py build
idf.py -p COM_NODE flash monitor
```

不要把 COM_GATEWAY/COM_NODE 原样复制到命令中；替换为识别好的两个不同串口。
副板目录的 build.ps1 仅为本机已安装 C6 工具提供便利构建入口，不负责下载 SDK、烧录或擦除。
两板应分别使用各自构建产物，不要将网关和节点固件烧反。

## 验收记录

```powershell
rustc --test src/drivers/radio/zigbee_check.rs -o target/zigbee-check-tests.exe
./target/zigbee-check-tests.exe
```

主机测试覆盖“发送成功不足以通过”、回调顺序、错误事务、错误目标状态及协议失败。
还必须进行以下实物验证，本次代码构建不代替射频测试：

| 场景 | 预期 |
| --- | --- |
| 入网窗口内启动副板 | 加入同 PAN/channel；发现 EP1/EP2 |
| 温度上报 | 每 5 秒，网关与节点日志数值一致 |
| 开关测试 | 四轮各自有 TX、Default Response、实际状态反馈 |
| 拔掉副板电源 | 15 秒内显示离线；挂起命令最多 12 秒超时并停止用例 |
| 网关重启 | 网络身份保留；副板后续主动报告触发重新发现 |
| 节点重启 | 网络恢复，继续报告，应用重新获取状态 |
| 关闭入网 | 另一个全新节点不得加入；既有节点恢复另行验证 |
| 无节点 | 不自动发控制命令，队列无持续增长 |

初步通过后再加入 Wi-Fi/BLE 共存测试；单芯负载和稳定性仍需测量。

参考：
- https://github.com/espressif/esp-zigbee-sdk/blob/main/RELEASE_NOTES.md
- https://components.espressif.com/components/espressif/esp-zigbee-lib/versions/2.1.0
