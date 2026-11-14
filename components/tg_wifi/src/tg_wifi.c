#include "tg_wifi.h"
#include <string.h>
#include "freertos/FreeRTOS.h"
#include "esp_event.h"
#include "esp_netif.h"
#include "esp_wifi.h"
#include "nvs_flash.h"

static portMUX_TYPE s_lock = portMUX_INITIALIZER_UNLOCKED;
static tg_wifi_status_t s_status;
static tg_wifi_ap_t s_aps[TG_WIFI_MAX_APS];
static esp_netif_t *s_netif;
static esp_event_handler_instance_t s_wifi_handler, s_ip_handler;
static bool s_driver, s_started, s_want_connection;

static esp_err_t record_error(esp_err_t err)
{
    portENTER_CRITICAL(&s_lock);
    s_status.last_error = err;
    ++s_status.generation;
    portEXIT_CRITICAL(&s_lock);
    return err;
}

static void on_event(void *arg, esp_event_base_t base, int32_t id, void *data)
{
    (void)arg;
    if (base == WIFI_EVENT && id == WIFI_EVENT_SCAN_DONE) {
        wifi_event_sta_scan_done_t *done = data;
        wifi_ap_record_t records[TG_WIFI_MAX_APS];
        uint16_t total = 0, count = TG_WIFI_MAX_APS;
        esp_err_t err = done->status == 0 ? esp_wifi_scan_get_ap_num(&total) : ESP_FAIL;
        if (err == ESP_OK) err = esp_wifi_scan_get_ap_records(&count, records);
        if (err != ESP_OK) { esp_wifi_clear_ap_list(); count = 0; }
        /* get_ap_records frees the SDK list, including entries beyond our cap. */
        portENTER_CRITICAL(&s_lock);
        for (uint16_t i = 0; i < count; ++i) {
            tg_wifi_ap_t *ap = &s_aps[i];
            memset(ap, 0, sizeof(*ap));
            ap->ssid_len = strnlen((const char *)records[i].ssid, 32);
            memcpy(ap->ssid, records[i].ssid, ap->ssid_len);
            memcpy(ap->bssid, records[i].bssid, 6);
            ap->rssi = records[i].rssi;
            ap->channel = records[i].primary;
            ap->auth_mode = records[i].authmode;
        }
        s_status.total_aps = total;
        s_status.stored_aps = count;
        s_status.last_error = err;
        s_status.scanning = false;
        s_status.scan_ready = true;
        ++s_status.generation;
        portEXIT_CRITICAL(&s_lock);
        return;
    }
    portENTER_CRITICAL(&s_lock);
    if (base == WIFI_EVENT && id == WIFI_EVENT_STA_CONNECTED && s_want_connection) {
        s_status.associated = true;
        s_status.connecting = false;
        s_status.disconnect_reason = 0;
    } else if (base == WIFI_EVENT && id == WIFI_EVENT_STA_DISCONNECTED) {
        s_want_connection = false;
        s_status.disconnecting = false;
        s_status.connecting = s_status.associated = s_status.has_ip = false;
        memset(s_status.ipv4, 0, 4);
        s_status.rssi = 0;
        s_status.disconnect_reason = ((wifi_event_sta_disconnected_t *)data)->reason;
    } else if (base == IP_EVENT && id == IP_EVENT_STA_GOT_IP && s_want_connection) {
        ip_event_got_ip_t *ip = data;
        /* addr is in network byte order; preserve its octets without endian assumptions. */
        memcpy(s_status.ipv4, &ip->ip_info.ip.addr, 4);
        s_status.has_ip = true;
    } else if (base == IP_EVENT && id == IP_EVENT_STA_LOST_IP) {
        s_status.has_ip = false;
        memset(s_status.ipv4, 0, 4);
    }
    ++s_status.generation;
    portEXIT_CRITICAL(&s_lock);
}

esp_err_t tg_wifi_init(void)
{
    if (s_driver || s_netif) return ESP_ERR_INVALID_STATE;
    esp_err_t err = nvs_flash_init();
    if (err != ESP_OK) return err;
    err = esp_netif_init();
    if (err != ESP_OK) return err;
    err = esp_event_loop_create_default();
    if (err != ESP_OK && err != ESP_ERR_INVALID_STATE) return err;
    s_netif = esp_netif_create_default_wifi_sta();
    if (!s_netif) return ESP_ERR_NO_MEM;
    wifi_init_config_t config = WIFI_INIT_CONFIG_DEFAULT();
    err = esp_wifi_init(&config);
    if (err != ESP_OK) goto fail;
    s_driver = true;
    err = esp_event_handler_instance_register(WIFI_EVENT, ESP_EVENT_ANY_ID,
                                              on_event, NULL, &s_wifi_handler);
    if (err != ESP_OK) goto fail;
    err = esp_event_handler_instance_register(IP_EVENT, ESP_EVENT_ANY_ID,
                                              on_event, NULL, &s_ip_handler);
    if (err != ESP_OK) goto fail;
    err = esp_wifi_set_storage(WIFI_STORAGE_RAM);
    if (err != ESP_OK) goto fail;
    err = esp_wifi_set_mode(WIFI_MODE_STA);
    if (err != ESP_OK) goto fail;
    err = esp_wifi_start();
    if (err != ESP_OK) goto fail;
    s_started = true;
    portENTER_CRITICAL(&s_lock);
    s_status.initialized = true;
    ++s_status.generation;
    portEXIT_CRITICAL(&s_lock);
    return ESP_OK;
fail:
    tg_wifi_deinit();
    return err;
}

esp_err_t tg_wifi_scan_start(void)
{
    portENTER_CRITICAL(&s_lock);
    bool allowed = s_status.initialized && !s_status.scanning &&
                   !s_status.connecting && !s_status.disconnecting;
    if (allowed) {
        s_status.scanning = true;
        s_status.scan_ready = false;
        s_status.last_error = ESP_OK;
        s_status.stored_aps = s_status.total_aps = 0;
    }
    portEXIT_CRITICAL(&s_lock);
    if (!allowed) return ESP_ERR_INVALID_STATE;
    wifi_scan_config_t config = { .show_hidden = true };
    esp_err_t err = esp_wifi_scan_start(&config, false);
    if (err != ESP_OK) {
        portENTER_CRITICAL(&s_lock);
        s_status.scanning = false;
        portEXIT_CRITICAL(&s_lock);
    }
    return err == ESP_OK ? ESP_OK : record_error(err);
}

esp_err_t tg_wifi_scan_get_ap(uint16_t index, tg_wifi_ap_t *ap)
{
    if (!ap) return ESP_ERR_INVALID_ARG;
    portENTER_CRITICAL(&s_lock);
    esp_err_t err = ESP_OK;
    if (!s_status.initialized || !s_status.scan_ready) err = ESP_ERR_INVALID_STATE;
    else if (index >= s_status.stored_aps) err = ESP_ERR_NOT_FOUND;
    else *ap = s_aps[index];
    portEXIT_CRITICAL(&s_lock);
    return err;
}

esp_err_t tg_wifi_connect(const char *ssid, const char *password)
{
    if (!ssid || !password) return ESP_ERR_INVALID_ARG;
    size_t slen = strnlen(ssid, 33), plen = strnlen(password, 64);
    if (!slen || slen > 32 || plen > 63 || (plen && plen < 8)) return ESP_ERR_INVALID_ARG;
    portENTER_CRITICAL(&s_lock);
    bool allowed = s_status.initialized && !s_status.scanning &&
                   !s_status.connecting && !s_status.disconnecting && !s_status.associated;
    if (allowed) {
        s_want_connection = true;
        s_status.connecting = true;
        s_status.disconnect_reason = 0;
        s_status.last_error = ESP_OK;
    }
    portEXIT_CRITICAL(&s_lock);
    if (!allowed) return ESP_ERR_INVALID_STATE;
    wifi_config_t config = {0};
    memcpy(config.sta.ssid, ssid, slen);
    memcpy(config.sta.password, password, plen);
    /* Empty password explicitly selects an open network; no WEP/enterprise API. */
    config.sta.threshold.authmode = plen ? WIFI_AUTH_WPA2_PSK : WIFI_AUTH_OPEN;
    config.sta.pmf_cfg.capable = true;
    esp_err_t err = esp_wifi_set_config(WIFI_IF_STA, &config);
    if (err == ESP_OK) err = esp_wifi_connect();
    memset(&config, 0, sizeof(config));
    if (err != ESP_OK) {
        portENTER_CRITICAL(&s_lock);
        s_want_connection = s_status.connecting = false;
        portEXIT_CRITICAL(&s_lock);
    }
    return err == ESP_OK ? ESP_OK : record_error(err);
}

esp_err_t tg_wifi_disconnect(void)
{
    if (!s_status.initialized) return ESP_ERR_INVALID_STATE;
    portENTER_CRITICAL(&s_lock);
    bool busy = s_status.connecting || s_status.associated;
    bool disconnecting = s_status.disconnecting;
    if (busy && !disconnecting) s_status.disconnecting = true;
    s_want_connection = false;
    portEXIT_CRITICAL(&s_lock);
    if (!busy || disconnecting) return ESP_OK;
    esp_err_t err = esp_wifi_disconnect();
    if (err == ESP_OK) {
        portENTER_CRITICAL(&s_lock);
        s_status.connecting = s_status.associated = s_status.has_ip = false;
        memset(s_status.ipv4, 0, 4);
        s_status.rssi = 0;
        portEXIT_CRITICAL(&s_lock);
    } else {
        portENTER_CRITICAL(&s_lock);
        s_status.disconnecting = false;
        portEXIT_CRITICAL(&s_lock);
    }
    return record_error(err);
}

esp_err_t tg_wifi_get_status(tg_wifi_status_t *status)
{
    if (!status) return ESP_ERR_INVALID_ARG;
    portENTER_CRITICAL(&s_lock);
    *status = s_status;
    portEXIT_CRITICAL(&s_lock);
    if (status->associated) {
        wifi_ap_record_t ap;
        if (esp_wifi_sta_get_ap_info(&ap) == ESP_OK) status->rssi = ap.rssi;
    }
    return ESP_OK;
}

esp_err_t tg_wifi_deinit(void)
{
    if (!s_driver && !s_netif) return ESP_ERR_INVALID_STATE;
    if (s_started) {
        esp_err_t err = esp_wifi_stop();
        if (err != ESP_OK) return err;
        s_started = false;
    }
    if (s_wifi_handler) {
        esp_event_handler_instance_unregister(WIFI_EVENT, ESP_EVENT_ANY_ID, s_wifi_handler);
        s_wifi_handler = NULL;
    }
    if (s_ip_handler) {
        esp_event_handler_instance_unregister(IP_EVENT, ESP_EVENT_ANY_ID, s_ip_handler);
        s_ip_handler = NULL;
    }
    if (s_driver) {
        esp_err_t err = esp_wifi_deinit();
        if (err != ESP_OK) return err;
        s_driver = false;
    }
    if (s_netif) { esp_netif_destroy_default_wifi(s_netif); s_netif = NULL; }
    portENTER_CRITICAL(&s_lock);
    memset(&s_status, 0, sizeof(s_status));
    s_want_connection = false;
    portEXIT_CRITICAL(&s_lock);
    return ESP_OK;
}
