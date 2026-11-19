#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

#define TG_SDCARD_MOUNT_PATH "/sdcard"

typedef struct {
    uint64_t capacity_bytes;
    uint32_t sector_size_bytes;
    uint32_t clock_khz;
} tg_sdcard_info_t;

/* One application task owns SPI2 and the mount. Close all files before unmount.
 * The driver mounts an existing FAT volume without formatting the card. */
esp_err_t tg_sdcard_init(void);
bool tg_sdcard_is_mounted(void);
esp_err_t tg_sdcard_get_info(tg_sdcard_info_t *info);
/* Also releases resources left by an incomplete mount. Safe to retry. */
esp_err_t tg_sdcard_deinit(void);

#ifdef __cplusplus
}
#endif
