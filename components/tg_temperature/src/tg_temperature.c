#include "tg_temperature.h"
#include "driver/temperature_sensor.h"

static temperature_sensor_handle_t s_sensor;
static bool s_enabled;

esp_err_t tg_temperature_init(int min_celsius, int max_celsius)
{
    if (s_sensor) return ESP_ERR_INVALID_STATE;
    if (min_celsius >= max_celsius) return ESP_ERR_INVALID_ARG;
    temperature_sensor_config_t config = TEMPERATURE_SENSOR_CONFIG_DEFAULT(min_celsius, max_celsius);
    esp_err_t err = temperature_sensor_install(&config, &s_sensor);
    if (err != ESP_OK) return err;
    err = temperature_sensor_enable(s_sensor);
    if (err == ESP_OK) {
        s_enabled = true;
    } else {
        /* Leave the handle recoverable by deinit if uninstall itself fails. */
        if (temperature_sensor_uninstall(s_sensor) == ESP_OK) s_sensor = NULL;
    }
    return err;
}

esp_err_t tg_temperature_read(float *celsius)
{
    if (!celsius) return ESP_ERR_INVALID_ARG;
    if (!s_sensor || !s_enabled) return ESP_ERR_INVALID_STATE;
    return temperature_sensor_get_celsius(s_sensor, celsius);
}

esp_err_t tg_temperature_deinit(void)
{
    if (!s_sensor) return ESP_ERR_INVALID_STATE;
    if (s_enabled) {
        esp_err_t err = temperature_sensor_disable(s_sensor);
        if (err != ESP_OK) return err;
        s_enabled = false;
    }
    esp_err_t err = temperature_sensor_uninstall(s_sensor);
    if (err == ESP_OK) s_sensor = NULL;
    return err;
}
