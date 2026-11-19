# TF 卡驱动

ESP32-C6 使用 SPI2 访问 TF 卡，驱动将 SPI 时钟上限设为 4 MHz。模块与核心板需共地。

| 模块信号 | ESP32-C6 引脚 |
| --- | --- |
| CS | GPIO0 |
| SCK | GPIO1 |
| MOSI | GPIO2 |
| MISO | GPIO3 |

C 组件 [tg_sdcard](../components/tg_sdcard/include/tg_sdcard.h) 负责初始化总线、识别卡片和挂载已有的 FAT 卷，挂载路径为 `/sdcard`。驱动只在应用显式调用时挂载，挂载失败会返回错误并释放已取得的资源。挂载流程不格式化卡。

Rust 的 [SdCard](../src/sdcard.rs) 封装 `mount()`、`info()`、`path()` 和 `unmount()`。挂载后可通过 `std::fs` 访问文件。由一个应用任务持有挂载实例，卸载前先关闭该卷上的文件。
