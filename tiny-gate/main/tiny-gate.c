#include <stdio.h>
#include <inttypes.h>
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "rule_engine.h"

static const char *TAG = "TinyGate-Main";

static void handle_action_callback(const tg_action_t *action, void *user_data)
{
    if (!action) {
        return;
    }

    ESP_LOGI(TAG, ">>> [Action Triggered] Target: %s, Proto: %d, EP: %d, Cmd: %s",
             action->target_device ? action->target_device : "null",
             action->protocol,
             action->endpoint,
             action->command ? action->command : "null");

    switch (action->param.type) {
    case TG_VAL_INT:
        ESP_LOGI(TAG, "    Param (Int): %" PRId64, action->param.val.i_val);
        break;
    case TG_VAL_FLOAT:
        ESP_LOGI(TAG, "    Param (Float): %f", action->param.val.f_val);
        break;
    case TG_VAL_BOOL:
        ESP_LOGI(TAG, "    Param (Bool): %s", action->param.val.b_val ? "true" : "false");
        break;
    case TG_VAL_STRING:
        ESP_LOGI(TAG, "    Param (String): %s", action->param.val.s_val ? action->param.val.s_val : "null");
        break;
    default:
        break;
    }
}

void app_main(void)
{
    ESP_LOGI(TAG, "========================================");
    ESP_LOGI(TAG, "TinyGate Gateway Booting...");
    ESP_LOGI(TAG, "Rust Rule Engine Version: %s", tg_rule_engine_version());
    ESP_LOGI(TAG, "========================================");

    int32_t ret = tg_rule_engine_init();
    if (ret != 0) {
        ESP_LOGE(TAG, "Failed to initialize Rust Rule Engine (err=%ld)", (long)ret);
        return;
    }
    ESP_LOGI(TAG, "Rule engine initialized successfully.");

    // Demo Event 1: Zigbee sensor temperature 32.5 C (triggers cooler relay)
    tg_event_t temp_event = {
        .device_id = "zigbee_sensor_01",
        .protocol = TG_PROTO_ZIGBEE,
        .endpoint = 1,
        .attribute = "temperature",
        .value = {
            .type = TG_VAL_FLOAT,
            .val = { .f_val = 32.5 }
        },
        .timestamp_ms = 1000
    };

    ESP_LOGI(TAG, "Dispatching Event: Temperature 32.5 C from %s", temp_event.device_id);
    int32_t matched = tg_rule_engine_dispatch(&temp_event, handle_action_callback, NULL);
    ESP_LOGI(TAG, "Rule matching complete. Total actions triggered: %ld", (long)matched);

    // Demo Event 2: Physical Button Press (triggers local relay toggle)
    tg_event_t button_event = {
        .device_id = "button_main",
        .protocol = TG_PROTO_SYSTEM,
        .endpoint = 0,
        .attribute = "press",
        .value = {
            .type = TG_VAL_BOOL,
            .val = { .b_val = true }
        },
        .timestamp_ms = 1050
    };

    ESP_LOGI(TAG, "Dispatching Event: Physical Button Press");
    matched = tg_rule_engine_dispatch(&button_event, handle_action_callback, NULL);
    ESP_LOGI(TAG, "Rule matching complete. Total actions triggered: %ld", (long)matched);

    ESP_LOGI(TAG, "TinyGate C <-> Rust FFI pipeline established.");
}
