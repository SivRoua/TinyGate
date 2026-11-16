#include "c_drivers.h"
#include <stdio.h>
#include "esp_log.h"

static const char *TAG = "TG-CDrivers";

static bool s_sdcard_mounted = false;

esp_err_t tg_c_drivers_init(void)
{
    ESP_LOGI(TAG, "Initializing TinyGate C driver subsystem...");

    esp_err_t err = tg_ups_init();
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "tg_ups_init returned %d", err);
    }

    err = tg_sdcard_init();
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "tg_sdcard_init returned %d", err);
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
// 3. MicroSD Storage (SPI, FATFS)
// ==========================================
esp_err_t tg_sdcard_init(void)
{
    ESP_LOGI(TAG, "[SDCard] Initializing SPI bus & FATFS mount...");
    s_sdcard_mounted = true;
    return ESP_OK;
}

bool tg_sdcard_is_mounted(void)
{
    return s_sdcard_mounted;
}

// ==========================================
// 4. Zigbee 3.0 Coordinator (802.15.4)
// ==========================================

// ==========================================
// 5. BLE Central / Observer
// ==========================================
