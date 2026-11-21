# OLED SSD1306 驱动

> 默认固件现运行 Zigbee 联调，OLED 显示节点温度与开关状态；
> 本页原有 Wi-Fi/BLE 页面演示使用 `cargo run --no-default-features`。

## 启动冲突排查（2026-11-16）

上板日志曾出现 `i2c: CONFLICT! driver_ng is not allowed to be used with this old driver`，
随后 abort 并循环重启，发生在 Rust main/OLED 初始化之前。
链接映射表确认旧 `i2c.c.obj` 由 esp-idf-hal 的未使用 `i2c_driver_delete` 引用带入。
Cargo 的 dev/release 均启用 fat LTO 与单 codegen unit，在链接原生库之前消除未使用的 Rust I2C 代码。
不通过关闭 SDK 的驱动冲突检查掩盖问题，也不要在业务里同时调用新/旧 I2C API。

后续改动构建配置或升级依赖时，除编译成功外，还需检查最终 ELF：
旧构造函数 `check_i2c_driver_conflict` 不应存在，新 API `i2c_new_master_bus` 应存在。
启动应能到达 `tg_display` 的地址探测日志，而非只看到启动器日志和重启。
驱动同时将 VCOMH 参数从数据手册未定义的 0x40 改为标准 0x20；该参数不是上述启动崩溃原因。

修复版本已通过 COM8 烧录并重启验证：0x3C 收到 ACK、SSD1306 初始化及亮屏命令成功，
Wi-Fi/BLE 示例完成，运行时间持续递增且未再次出现启动冲突。串口验证不等于目视确认屏幕内容。

## 当前硬件约定

- ESP32-C6 SuperMini；引脚参考 `materials/ESP32-C6 super mini_pinout.png`。
- SDA → GPIO20，SCL → GPIO19；与 USB 的 GPIO12/13 无冲突。
- OLED GND → GND；对于支持 3.3V 的模块，VCC 接 3V3，I2C 上拉也需到 3.3V。
- 按原 DESIGN.md 假设为四线 I2C SSD1306、128×64、内置电荷泵，无独立 RESET。
- 默认 100kHz，自动探测 7-bit 0x3C / 0x3D（不是 0x78 / 0x7A）。

屏幕尺寸不能唯一确定控制器；I2C ACK 也不能识别 SSD1306 型号。
SH1106、128×32、SPI 模块需要另行适配，不能直接按本驱动判断硬件故障。
驱动开启内部上拉；稳定通信仍依赖模块/线路上合适的外部上拉和良好接触。

## 代码与职责

```text
components/tg_display/
  Kconfig                   默认引脚与地址
  include/tg_display.h       你提供给 Rust 的 C 接口
  src/tg_display.c           I2C 总线、地址探测、SSD1306 初始化、刷屏与资源释放
  src/display_canvas.c       可独立测试的页面布局、像素和小字体渲染
src/drivers/board/display.rs Rust 安全封装和独占生命周期
src/ui/                     页面内容与刷新时机（唯一屏幕写者）
tests/display_canvas_test.c  无硬件渲染测试
```

旧 c_drivers 中仅打印日志的显示函数已经移除；同名真实接口由 tg_display 提供。
旧 C 调用者需改为包含 `tg_display.h` 并声明组件依赖，Rust 使用新 `Display` 封装。
不调用旧 init_all，避免显示驱动与 UPS/SD 占位实现绑定。

## C 接口

| 接口 | 行为 |
| --- | --- |
| `tg_display_init` | Kconfig 默认接线，100kHz，探测地址 |
| `tg_display_init_with_config` | 显式指定 SDA、SCL、时钟和地址 |
| `tg_display_get_address` | 返回实际探测到的地址 |
| `tg_display_clear` | 清空 RAM 和屏幕 |
| `tg_display_write_line` | 覆盖一行 RAM，短文本会擦掉旧尾部，不自动刷屏 |
| `tg_display_set_pixel` | 修改 RAM 中单个像素，不自动刷屏 |
| `tg_display_flush` | 同步发送完整 1024 字节帧缓冲 |
| `tg_display_flush_page` | 只刷新一个文本行（行号与 SSD1306 页 1:1），状态页局部更新用 |
| `tg_display_show_status` | 清屏，在第 0、2、4 行写三段文字并刷新 |
| `tg_display_set_power` | 亮屏/熄屏，保留 RAM 内容 |
| `tg_display_set_contrast` | 对比度 0..255 |
| `tg_display_deinit` | 尽力熄屏，移除设备并释放总线；拔掉屏幕仍可释放 |

行号 0..7；一行 21 字符；像素坐标 x=0..127、y=0..63。
字体为 5×7 大写英文字母、数字与基本标点，小写转大写，其余字节显示 `?`。
无中文字库；长文本裁剪，不自动换行。所有 I2C 事务同步，有 100ms 单次超时。
完整帧会执行多次事务，因此 100ms 不是整个 flush 的总超时。
初始化失败尝试回收资源，重复初始化和未就绪访问返回状态错误。

当前组件独占一个自动分配的 I2C 主机及 SDA/SCL，不与其他组件共享句柄。
将来在同一总线上增加传感器时，应由板级总线管理者统一分配设备。
所有调用必须在同一个应用任务中串行执行，禁止在无线回调或 ISR 里刷屏。
Rust 包装禁止跨线程传递和重复创建；应用主任务更新 UI，协议栈仍由 SDK 任务运行。
没有在驱动内创建刷新线程；后续需要 UI 任务时，由 Rust 核心统一调度。

## Rust 使用

```rust,ignore
let mut screen = display::Display::new()?;
screen.show_status("TinyGate", "WIFI READY", "192.168.1.10")?;
screen.write_line(6, "UP 10 S")?;
screen.set_pixel(127, 63, true)?;
screen.flush()?;
screen.set_contrast(100)?;
// screen.set_power(false)?;
// screen.shutdown()?; // Drop 为错误路径提供尽力清理
```

主程序先显示 BOOT 页面，随后显示 Wi-Fi/BLE 扫描阶段，最终每 10 秒更新运行时间和剩余内存。
OLED 初始化失败则记录错误并继续无线示例，不伪造显示成功。
当前页面是接入演示，不表示已实现联网和自动化业务。

## 验证与运行

```powershell
# 修改自定义 C 代码后：直接 cargo build（根 build.rs 已 watch components/ 并直驱 ninja 增量重编）
cargo build
# 修改头文件（platform_bindings.h 等，涉及 bindings 重新生成）时才需要刷新：
cargo clean -p esp-idf-sys && cargo build
# 主机测试（使用本机 MinGW GCC）
gcc -std=c11 -Wall -Wextra -Werror -Icomponents/tg_display/src tests/display_canvas_test.c components/tg_display/src/display_canvas.c -o target/display-canvas-test.exe
./target/display-canvas-test.exe
# 确认接线和串口后烧录
cargo run
```

需实物验证：地址应答、屏幕方向、字体、四角像素、清屏、亮灭、重复初始化、
运行时断开 I2C 后的错误反馈。编译和主机测试不能替代以上验证。
若 NACK：检查供电、共地、SDA/SCL 顺序及实际模块地址；若 ACK 但黑屏/偏移，
检查控制器与分辨率，再检查电荷泵、复位和初始化参数。

实现采用 ESP-IDF v6.1 新 I2C master API；SSD1306 指令直接发送，未引入 LVGL 等 UI 依赖。
参考：https://docs.espressif.com/projects/esp-idf/en/stable/esp32c6/api-reference/peripherals/i2c.html
