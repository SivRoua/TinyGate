#include "c_drivers.h"
#include <stdio.h>
#include "esp_log.h"

static const char *TAG = "TG-CDrivers";

esp_err_t tg_c_drivers_init(void)
{
    ESP_LOGI(TAG, "Initializing TinyGate C driver subsystem...");

    esp_err_t err = tg_power_init();
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "tg_power_init returned %d", err);
    }

    ESP_LOGI(TAG, "TinyGate C drivers initialized.");
    return ESP_OK;
}

// ==========================================
// 1. Power rail / supercapacitor hold-up
// ==========================================
esp_err_t tg_power_init(void)
{
    ESP_LOGI(TAG, "[POWER] Initializing mains-loss interrupt & supercap rail sampling...");
    return ESP_OK;
}

bool tg_power_mains_lost(void)
{
    // Placeholder: external power is present.
    return false;
}

float tg_power_rail_voltage(void)
{
    // Placeholder: supercap rail is up.
    return 5.0f;
}

// ==========================================
// 5. BLE Central / Observer
// ==========================================
