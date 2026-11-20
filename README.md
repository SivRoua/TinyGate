# TinyGate
A netgate based on ESP32-C6 supermini board

- [架构设计](DESIGN.md)
- [核心板引脚与当前占用](materials/pinout-and-usage.md)
- 硬件验证 bench 固件在 `demo` 分支；`main` 为网关产品主线（embassy 双 executor：无线电服务 + 低优先级 UI 任务）
- [TF 卡驱动](docs/tf-card-driver.md)
- [网关、云端与 Web / 微信小程序通信契约](contracts/README.md)
- [双 C6 Zigbee 构建与联调](docs/zigbee-bringup.md)
- [GPIO9 按键驱动与验证](docs/button-driver.md)
- [Wi-Fi、BLE 与 RGB 串口联调](docs/connectivity-and-rgb.md)
- [测试分支：Wi-Fi / Zigbee 稳定性程序](docs/coexist-test.md)
