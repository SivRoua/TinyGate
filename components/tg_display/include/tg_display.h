#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#ifdef __cplusplus
extern "C" {
#endif
#define TG_DISPLAY_WIDTH 128
#define TG_DISPLAY_HEIGHT 64
#define TG_DISPLAY_ROWS 8
#define TG_DISPLAY_COLUMNS 21

typedef struct {
    int32_t sda_gpio;
    int32_t scl_gpio;
    uint32_t clock_hz;       /* 10000..400000; default 100000 for breadboard */
    uint8_t address;         /* 0 = probe 0x3C/0x3D; otherwise either address */
} tg_display_config_t;

/* SSD1306 128x64 only. Serialized application-task calls, never ISR/radio callback.
 * Owns an I2C master bus exclusively. Default config from Kconfig: SDA20/SCL19.
 * All calls are synchronous; errors propagate; no background task is created. */
esp_err_t tg_display_init(void);
esp_err_t tg_display_init_with_config(const tg_display_config_t *config);
esp_err_t tg_display_get_address(uint8_t *address);
/* clear and show_status flush immediately; text/pixel edit RAM until flush.
 * Text uses 5x7 capitals/digits/basic punctuation, lowercase mapped to uppercase,
 * unsupported bytes rendered as '?'. 21 columns x 8 rows; long text clipped.
 * High-bit bytes 0x80..0xFF are the icon code space: paired halves
 * (0x80+2i, 0x81+2i) render an 11x7 icon across two character cells;
 * undefined icon bytes render blank. Rust maps U+E080..U+E0FF onto it. */
esp_err_t tg_display_clear(void);
esp_err_t tg_display_write_line(uint8_t row, const char *text);
esp_err_t tg_display_set_pixel(uint16_t x, uint16_t y, bool on);
esp_err_t tg_display_flush(void);
/* Single-row refresh: text rows map 1:1 to SSD1306 pages, so a status page
 * can redraw only the rows that changed instead of the whole frame. */
esp_err_t tg_display_flush_page(uint8_t page);
esp_err_t tg_display_show_status(const char *title, const char *line1, const char *line2);
esp_err_t tg_display_set_power(bool on);
esp_err_t tg_display_set_contrast(uint8_t contrast);
esp_err_t tg_display_deinit(void);
#ifdef __cplusplus
}
#endif
