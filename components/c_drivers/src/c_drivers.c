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

    err = tg_display_init();
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "tg_display_init returned %d", err);
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
// 2. 0.96 OLED Display (SSD1306, I2C)
// ==========================================
esp_err_t tg_display_init(void)
{
    ESP_LOGI(TAG, "[OLED] SSD1306 I2C display driver init...");
    return ESP_OK;
}

esp_err_t tg_display_clear(void)
{
    ESP_LOGI(TAG, "[OLED] Clear screen");
    return ESP_OK;
}

esp_err_t tg_display_show_status(const char *title, const char *line1, const char *line2)
{
    ESP_LOGI(TAG, "[OLED Display] [%s] %s | %s",
             title ? title : "",
             line1 ? line1 : "",
             line2 ? line2 : "");
    return ESP_OK;
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
esp_err_t tg_zigbee_coord_init(void)
{
    ESP_LOGI(TAG, "[Zigbee] Initializing 802.15.4 coordinator stack...");
    return ESP_OK;
}

// ==========================================
// 5. BLE Central / Observer
// ==========================================
esp_err_t tg_ble_init(void)
{
    ESP_LOGI(TAG, "[BLE] Initializing BLE 5.3 observer/central...");
    return ESP_OK;
}
