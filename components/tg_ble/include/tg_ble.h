#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

#define TG_BLE_ADV_MAX_LEN 31
#define TG_BLE_EVENT_REPORT 1
#define TG_BLE_EVENT_COMPLETE 2
#define TG_BLE_EVENT_RESET 3

/* All public calls must be serialized by one application task. Do not call
 * these APIs from ISR or NimBLE callbacks. This component owns NimBLE. */
typedef struct {
    uint32_t duration_ms;       /* 1..INT32_MAX; finite scans only */
    uint16_t interval_units;    /* 0.625 ms units, 0x0004..0x4000 */
    uint16_t window_units;      /* Same units; <= interval_units */
    bool active;               /* Request scan responses */
    bool filter_duplicates;
} tg_ble_scan_config_t;

typedef struct {
    uint8_t kind;
    uint8_t address[6];         /* NimBLE little-endian; reverse for display */
    uint8_t address_type;       /* Preserve public/random identity information */
    int8_t rssi;
    uint8_t advertising_type;   /* GAP legacy event type; 4 = scan response */
    uint8_t data_len;
    uint8_t data[TG_BLE_ADV_MAX_LEN];
    int32_t reason;             /* Native NimBLE completion/reset reason */
} tg_ble_event_t;

typedef struct {
    bool initialized;
    bool ready;
    bool scanning;
    uint32_t dropped_reports;
    int32_t last_host_error;
} tg_ble_status_t;

/* NVS errors are returned without erasing the application's NVS partition. */
esp_err_t tg_ble_init(void);
esp_err_t tg_ble_scan_start(const tg_ble_scan_config_t *config);
/* ESP_ERR_NOT_FOUND = no event; output is a value copy with no borrowed pointers. */
esp_err_t tg_ble_poll_event(tg_ble_event_t *event);
esp_err_t tg_ble_scan_stop(void);
esp_err_t tg_ble_get_status(tg_ble_status_t *status);
/* Blocking stop/deinit; invoke only from the application task. */
esp_err_t tg_ble_deinit(void);

#ifdef __cplusplus
}
#endif
