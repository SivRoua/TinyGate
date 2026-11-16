#pragma once
#include "esp_err.h"
#ifdef __cplusplus
extern "C" {
#endif
/* Single application-task owner. This measures die temperature, not ambient.
 * Range must fit one hardware measurement range; SDK validates it. */
esp_err_t tg_temperature_init(int min_celsius, int max_celsius);
esp_err_t tg_temperature_read(float *celsius);
esp_err_t tg_temperature_deinit(void);
#ifdef __cplusplus
}
#endif
