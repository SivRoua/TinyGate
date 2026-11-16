#pragma once
#include <stdint.h>
#include "esp_err.h"
#ifdef __cplusplus
extern "C" {
#endif
typedef struct {
    int32_t chip_model;       /* esp_chip_model_t */
    uint16_t revision;
    uint8_t cores;
    int32_t reset_reason;     /* esp_reset_reason_t */
    uint64_t uptime_ms;
    uint32_t free_heap_bytes;
    uint32_t minimum_free_heap_bytes;
    uint32_t largest_free_block_bytes; /* 8-bit accessible heap */
} tg_system_info_t;
esp_err_t tg_system_get_info(tg_system_info_t *info);
#ifdef __cplusplus
}
#endif
