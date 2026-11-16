#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#ifdef __cplusplus
extern "C" {
#endif
#define TG_WIFI_MAX_APS 20
typedef struct {
    uint8_t ssid[32];
    uint8_t ssid_len;
    uint8_t bssid[6];
    uint8_t channel;
    int8_t rssi;
    int32_t auth_mode; /* wifi_auth_mode_t */
} tg_wifi_ap_t;
typedef struct {
    bool initialized;
    bool scanning;
    bool scan_ready;
    bool connecting;
    bool disconnecting;
    bool associated;
    bool has_ip;
    uint8_t ipv4[4];
    int8_t rssi;       /* Valid only when associated */
    uint16_t total_aps;
    uint16_t stored_aps;
    uint16_t disconnect_reason; /* wifi_err_reason_t; 0 when not applicable */
    int32_t last_error;         /* esp_err_t from scan/connect operations */
    uint32_t generation;       /* Changes on status events; intermediate events may coalesce */
} tg_wifi_status_t;
/* Single application-task owner. Owns Wi-Fi driver and default STA netif.
 * The default event loop and esp-netif runtime are shared and retained.
 * Never mix this with an esp-idf-svc Wi-Fi owner. No automatic NVS erase. */
esp_err_t tg_wifi_init(void);
/* Async scan; poll status.scan_ready, then read entries. No connect during scan. */
esp_err_t tg_wifi_scan_start(void);
esp_err_t tg_wifi_scan_get_ap(uint16_t index, tg_wifi_ap_t *ap);
/* NUL-terminated SSID 1..32 bytes; password empty or 8..63 bytes.
 * Open or WPA2/WPA3 personal networks only. Credentials in RAM, never logged. */
esp_err_t tg_wifi_connect(const char *ssid, const char *password);
esp_err_t tg_wifi_disconnect(void);
esp_err_t tg_wifi_get_status(tg_wifi_status_t *status);
esp_err_t tg_wifi_deinit(void);
#ifdef __cplusplus
}
#endif
