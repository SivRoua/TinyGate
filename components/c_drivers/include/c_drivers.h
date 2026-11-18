#pragma once

#include <stdint.h>
#include <stdbool.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Legacy UPS placeholder; real drivers have independent lifecycles.
 * @return esp_err_t ESP_OK on success
 */
esp_err_t tg_c_drivers_init(void);

// ==========================================
// 1. UPS & Battery Management
// ==========================================
esp_err_t tg_ups_init(void);
float tg_ups_get_battery_voltage(void);
bool tg_ups_is_mains_power_loss(void);

// ==========================================
// 5. BLE Central / Observer
// ==========================================

#ifdef __cplusplus
}
#endif
