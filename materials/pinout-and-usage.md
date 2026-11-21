# ESP32-C6 SuperMini 引脚与当前占用（用户提供，2026-11-16）

BAT 是 USB 旁独立焊盘，不是右侧排针第一脚。右侧十脚依次为
5V、GND、3V3、20、19、18、15、14、9、8。引脚名称不能当作排针序号。

| 序号 | GPIO | 标记 | 类别（原表） | ADC | 功能 / 当前占用 |
| --- | --- | --- | --- | --- | --- |
| 1 | — | BAT | power | — | 板载充电器锂电池输入焊盘，非排针；2026-11-21 起电源改为超级电容，不再使用电池 |
| 2 | — | 5V | power | — | 5V 电源输入 |
| 3 | — | GND | ground | — | 公共地，OLED 与按键共地 |
| 4 | — | 3V3 | power | — | 外设 3.3V 输出 |
| 5 | 16 | TX/IO16 | uart | — | UART TX |
| 6 | 17 | RX/IO17 | uart | — | UART RX |
| 7 | 0 | IO0/GP0 | safe | 有 | **TF CS** |
| 8 | 1 | IO1/GP1 | safe | 有 | **TF SCK** |
| 9 | 2 | IO2/GP2 | safe | 有 | **TF MOSI** |
| 10 | 3 | IO3/GP3 | safe | 有 | **TF MISO** |
| 11 | 4 | IO4/GP4 | strapping | 有 | GPIO、ADC |
| 12 | 5 | IO5/GP5 | strapping | 有 | GPIO、ADC |
| 13 | 6 | IO6/GP6 | strapping | 有 | GPIO、ADC |
| 14 | 7 | IO7/GP7 | strapping | — | GPIO |
| 15 | 8 | IO8/GP8 | strapping | — | 板载 RGB；tg_rgb 使用，WS2812/GRB 待实测确认 |
| 16 | 9 | IO9/GP9 | strapping | — | BOOT；外接常开按钮另一端接 GND，tg_button 使用 |
| 17 | 12 | IO12/GP12 | uart | — | USB D−，保留串口/JTAG 使用 |
| 18 | 13 | IO13/GP13 | uart | — | USB D+，保留串口/JTAG 使用 |
| 19 | 14 | IO14/GP14 | safe | — | GPIO |
| 20 | 15 | IO15/GP15 | strapping | — | 板载普通 LED，未驱动 |
| 21 | 18 | IO18/GP18 | safe | — | GPIO |
| 22 | 19 | IO19/GP19 | safe | — | **OLED SCL** |
| 23 | 20 | IO20/GP20 | safe | — | **OLED SDA** |
| 24 | 21 | IO21/GP21 | safe | — | GPIO；背面焊盘见 pinout 图 |
| 25 | 22 | IO22/GP22 | safe | — | GPIO；背面焊盘见 pinout 图 |
| 26 | 23 | IO23/GP23 | safe | — | GPIO；背面焊盘见 pinout 图 |

类别沿用用户原表；不将 safe 解释为可任意接电压或自动可用。
GPIO6/7 的原表 strapping 分类与图中 JTAG 标记应结合芯片手册核实，代码不占用它们。
GPIO8/9 等启动配置脚不能在启动时由外部电路强制到不合适电平。
RGB 数据脚并不能证明具体灯珠型号；当前驱动按单颗 WS2812 兼容器件编写。
TF 卡已分配 SPI2 和 GPIO0–3，接线与接口见 [TF 卡驱动](../docs/tf-card-driver.md)。
电源掉电检测与超级电容保持电压采样待硬件确认（不使用电池电压）。
