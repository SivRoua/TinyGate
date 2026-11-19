#include "c_drivers.h"
#include <stdio.h>
#include "esp_log.h"

static const char *TAG = "TG-CDrivers";

esp_err_t tg_c_drivers_init(void)
{
    ESP_LOGI(TAG, "Initializing TinyGate C driver subsystem...");

    esp_err_t err = tg_ups_init();
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "tg_ups_init returned %d", err);
    }

    ESP_LOGI(TAG, "TinyGate C drivers initialized.");
    return ESP_OK;
}

// ==========================================
// 1. UPS & Battery Management
// ==========================================
esp_err_t tg_ups_init(void)
{
    ESP_LOGI(TAG, "[UPS] Initializing ADC voltage sampling & power loss interrupt...");
    return ESP_OK;
}

float tg_ups_get_battery_voltage(void)
{
    // Mock 3.85V for initial bring-up
    return 3.85f;
}

bool tg_ups_is_mains_power_loss(void)
{
    // Return false (normal external power)
    return false;
}

// ==========================================
// 5. BLE Central / Observer
// ==========================================
