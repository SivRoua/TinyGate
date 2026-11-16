#pragma once
#include <stdint.h>
#include "esp_err.h"
#ifdef __cplusplus
extern "C" {
#endif
/* One WS2812-compatible GRB LED on GPIO8, 10MHz RMT; single app-task owner.
 * brightness 0..255 is applied to all channels. No ISR/callback calls. */
esp_err_t tg_rgb_init(void);
esp_err_t tg_rgb_set(uint8_t red,uint8_t green,uint8_t blue,uint8_t brightness);
esp_err_t tg_rgb_off(void);
esp_err_t tg_rgb_deinit(void);
#ifdef __cplusplus
}
#endif
