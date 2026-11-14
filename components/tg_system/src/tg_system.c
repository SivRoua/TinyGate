#include "tg_system.h"
#include <string.h>
#include "esp_chip_info.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "esp_heap_caps.h"

esp_err_t tg_system_get_info(tg_system_info_t *info)
{
    if (!info) return ESP_ERR_INVALID_ARG;
    esp_chip_info_t chip;
    esp_chip_info(&chip);
    memset(info, 0, sizeof(*info));
    info->chip_model = chip.model;
    info->revision = chip.revision;
    info->cores = chip.cores;
    info->reset_reason = esp_reset_reason();
    info->uptime_ms = (uint64_t)esp_timer_get_time() / 1000;
    info->free_heap_bytes = esp_get_free_heap_size();
    info->minimum_free_heap_bytes = esp_get_minimum_free_heap_size();
    info->largest_free_block_bytes = heap_caps_get_largest_free_block(MALLOC_CAP_8BIT);
    return ESP_OK;
}
