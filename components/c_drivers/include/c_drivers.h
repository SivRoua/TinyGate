#pragma once

#include <stdint.h>
#include <stdbool.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Legacy combined C driver placeholder; real drivers have independent lifecycles.
 * @return esp_err_t ESP_OK on success
 */
esp_err_t tg_c_drivers_init(void);

// ==========================================
// 1. Power rail / supercapacitor hold-up
// ==========================================
// Mains-loss detection plus rail sampling. The supercap is not a battery:
// it only provides a short save window, so there is no SoC or runtime API.
esp_err_t tg_power_init(void);
bool tg_power_mains_lost(void);
float tg_power_rail_voltage(void);

// ==========================================
// 5. BLE Central / Observer
// ==========================================

#ifdef __cplusplus
}
#endif
