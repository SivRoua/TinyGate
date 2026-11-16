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

/* Central/GATT: up to two links, one procedure per link. Scan must be stopped
 * before connect. IDs must remain unique until completion; no automatic replay.
 * UUIDs are printable strings; data is copied, at most 128 bytes, truncation explicit. */
#define TG_BLE_LINK 1
#define TG_BLE_UNLINK 2
#define TG_BLE_SERVICE 3
#define TG_BLE_CHARACTERISTIC 4
#define TG_BLE_DESCRIPTOR 5
#define TG_BLE_VALUE 6
#define TG_BLE_OPERATION_DONE 7
#define TG_BLE_NOTIFICATION 8
typedef struct {
    uint8_t kind;
    uint16_t connection;
    uint16_t handle;
    uint16_t end_handle;
    uint8_t properties;
    char uuid[37];
    uint32_t request_id;
    int32_t status; /* Native NimBLE status; DONE maps BLE_HS_EDONE to success */
    uint16_t data_len;
    bool truncated;
    uint8_t data[128];
} tg_ble_gatt_event_t;
typedef struct {
    uint8_t links;
    uint16_t handles[2]; /* 0xFFFF if unused; snapshot for recovery after queue overflow */
    bool connecting;
    uint32_t dropped_events;
} tg_ble_links_t;
esp_err_t tg_ble_connect(const uint8_t address[6],uint8_t address_type);
esp_err_t tg_ble_connect_cancel(void);
esp_err_t tg_ble_disconnect(uint16_t connection);
esp_err_t tg_ble_services(uint16_t connection,uint32_t id);
esp_err_t tg_ble_characteristics(uint16_t connection,uint16_t start,uint16_t end,uint32_t id);
esp_err_t tg_ble_descriptors(uint16_t connection,uint16_t start,uint16_t end,uint32_t id);
esp_err_t tg_ble_read(uint16_t connection,uint16_t handle,uint32_t id);
esp_err_t tg_ble_write(uint16_t connection,uint16_t handle,const uint8_t *data,uint16_t length,uint32_t id);
/* CCCD handle must come from discovery (UUID 0x2902); mode 0/1/2 = off/notify/indicate. */
esp_err_t tg_ble_subscribe(uint16_t connection,uint16_t cccd,uint8_t mode,uint32_t id);
esp_err_t tg_ble_poll_gatt(tg_ble_gatt_event_t *event);
esp_err_t tg_ble_get_links(tg_ble_links_t *status);

#ifdef __cplusplus
}
#endif
