#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#ifdef __cplusplus
extern "C" {
#endif
#define TG_ZB_NETWORK 1
#define TG_ZB_JOIN 2
#define TG_ZB_ENDPOINT 3
#define TG_ZB_ATTRIBUTE 4
#define TG_ZB_TX_CONFIRM 5
#define TG_ZB_DEFAULT_RESPONSE 6
#define TG_ZB_REPORT_CONFIG 7
#define TG_ZB_LEAVE 8
#define TG_ZB_ERROR 9
#define TG_ZB_TEMPERATURE_CLUSTER 0x0402
#define TG_ZB_ONOFF_CLUSTER 0x0006

typedef struct {
    uint8_t kind;
    uint16_t short_address;
    uint8_t ieee_address[8]; /* Native little-endian EUI64, present in JOIN/LEAVE */
    uint8_t endpoint;
    uint16_t cluster;
    uint16_t attribute;
    uint16_t profile;
    int32_t value;           /* temperature: 0.01 C; OnOff: 0/1 */
    uint64_t test_payload;   /* Test-only attr 0xF000: boot-session high32, sequence low32 */
    int32_t status;          /* Native SDK/ZCL status; domain depends on kind */
    uint32_t request_id;     /* Supplied ID, only on TX_CONFIRM */
    uint8_t tsn;            /* Native ZCL transaction sequence number */
    bool is_report;         /* ATTRIBUTE: unsolicited report vs read response */
} tg_zigbee_event_t;
typedef struct {
    bool started;
    bool ready;
    bool join_open;
    uint8_t channel;
    uint16_t pan_id;
    uint16_t peer_address;  /* Most recently announced/reporting peer, 0xFFFF if unknown */
    uint32_t dropped_events;
    int32_t last_error;
} tg_zigbee_status_t;

/* Coordinator, channel 15, persistent dedicated zb_nvs. One application caller.
 * Startup is asynchronous; check ready. SDK accesses are lock-protected.
 * No shutdown/re-init API: this component runs for the lifetime of firmware. */
esp_err_t tg_zigbee_init(void);
esp_err_t tg_zigbee_get_status(tg_zigbee_status_t *status);
esp_err_t tg_zigbee_poll_event(tg_zigbee_event_t *event);
esp_err_t tg_zigbee_permit_join(uint8_t seconds); /* 0 closes; 1..254 seconds */
esp_err_t tg_zigbee_discover(uint16_t address);
esp_err_t tg_zigbee_read_attribute(uint16_t address, uint8_t endpoint, uint16_t cluster,
                                  uint32_t request_id);
esp_err_t tg_zigbee_send_onoff(uint16_t address, uint8_t endpoint, uint8_t command,
                              uint32_t request_id); /* command 0/1/2 = off/on/toggle */
esp_err_t tg_zigbee_configure_reporting(uint16_t address, uint8_t endpoint,
                                       uint16_t cluster, uint32_t request_id);
#ifdef __cplusplus
}
#endif
