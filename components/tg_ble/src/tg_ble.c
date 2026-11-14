#include "tg_ble.h"

#include <limits.h>
#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "host/ble_hs.h"
#include "nimble/nimble_port.h"
#include "nimble/nimble_port_freertos.h"
#include "nvs_flash.h"

#define REPORT_CAPACITY 32
/* Callback and application task share only short, bounded copies under lock. */
static portMUX_TYPE s_lock = portMUX_INITIALIZER_UNLOCKED;
static tg_ble_status_t s_status;
static tg_ble_event_t s_reports[REPORT_CAPACITY];
static unsigned s_head, s_count;
static bool s_complete_pending, s_reset_pending;
static int s_complete_reason, s_reset_reason;

static void on_sync(void)
{
    portENTER_CRITICAL(&s_lock);
    s_status.ready = true;
    portEXIT_CRITICAL(&s_lock);
}

static void on_reset(int reason)
{
    portENTER_CRITICAL(&s_lock);
    s_status.ready = false;
    s_status.scanning = false;
    s_status.last_host_error = reason;
    s_reset_reason = reason;
    s_reset_pending = true;
    portEXIT_CRITICAL(&s_lock);
}

static int on_gap(struct ble_gap_event *event, void *arg)
{
    (void)arg;
    if (event->type == BLE_GAP_EVENT_DISC) {
        const struct ble_gap_disc_desc *d = &event->disc;
        tg_ble_event_t report = { .kind = TG_BLE_EVENT_REPORT };
        memcpy(report.address, d->addr.val, sizeof(report.address));
        report.address_type = d->addr.type;
        report.rssi = d->rssi;
        report.advertising_type = d->event_type;
        report.data_len = d->length_data > TG_BLE_ADV_MAX_LEN
                              ? TG_BLE_ADV_MAX_LEN : d->length_data;
        if (report.data_len) memcpy(report.data, d->data, report.data_len);
        portENTER_CRITICAL(&s_lock);
        if (s_count < REPORT_CAPACITY) {
            s_reports[(s_head + s_count) % REPORT_CAPACITY] = report;
            ++s_count;
        } else if (s_status.dropped_reports != UINT32_MAX) {
            ++s_status.dropped_reports;
        }
        portEXIT_CRITICAL(&s_lock);
    } else if (event->type == BLE_GAP_EVENT_DISC_COMPLETE) {
        portENTER_CRITICAL(&s_lock);
        s_status.scanning = false;
        s_complete_reason = event->disc_complete.reason;
        s_complete_pending = true;
        portEXIT_CRITICAL(&s_lock);
    }
    return 0;
}

static void host_task(void *arg)
{
    (void)arg;
    nimble_port_run();
    nimble_port_freertos_deinit();
}

esp_err_t tg_ble_init(void)
{
    if (s_status.initialized) return ESP_ERR_INVALID_STATE;
    esp_err_t err = nvs_flash_init();
    if (err != ESP_OK) return err;
    err = nimble_port_init();
    if (err != ESP_OK) return err;
    portENTER_CRITICAL(&s_lock);
    memset(&s_status, 0, sizeof(s_status));
    s_status.initialized = true;
    s_head = s_count = 0;
    s_complete_pending = s_reset_pending = false;
    portEXIT_CRITICAL(&s_lock);
    ble_hs_cfg.sync_cb = on_sync;
    ble_hs_cfg.reset_cb = on_reset;
    nimble_port_freertos_init(host_task);
    return ESP_OK;
}

esp_err_t tg_ble_scan_start(const tg_ble_scan_config_t *config)
{
    if (!config || !config->duration_ms || config->duration_ms > INT32_MAX ||
        config->interval_units < 4 || config->interval_units > 0x4000 ||
        config->window_units < 4 || config->window_units > config->interval_units)
        return ESP_ERR_INVALID_ARG;
    portENTER_CRITICAL(&s_lock);
    /* Consume the previous session's data/completion before starting another. */
    bool allowed = s_status.initialized && s_status.ready && !s_status.scanning &&
                   !s_complete_pending && !s_reset_pending && !s_count;
    if (allowed) s_status.scanning = true;
    portEXIT_CRITICAL(&s_lock);
    if (!allowed) return ESP_ERR_INVALID_STATE;

    uint8_t own_address_type;
    int rc = ble_hs_id_infer_auto(0, &own_address_type);
    if (!rc) {
        struct ble_gap_disc_params params = {0};
        params.itvl = config->interval_units;
        params.window = config->window_units;
        params.passive = !config->active;
        params.filter_duplicates = config->filter_duplicates;
        rc = ble_gap_disc(own_address_type, (int32_t)config->duration_ms,
                          &params, on_gap, NULL);
    }
    if (rc) {
        portENTER_CRITICAL(&s_lock);
        s_status.scanning = false;
        s_status.last_host_error = rc;
        portEXIT_CRITICAL(&s_lock);
        return ESP_FAIL;
    }
    return ESP_OK;
}

esp_err_t tg_ble_poll_event(tg_ble_event_t *event)
{
    if (!event) return ESP_ERR_INVALID_ARG;
    esp_err_t result = ESP_OK;
    portENTER_CRITICAL(&s_lock);
    memset(event, 0, sizeof(*event));
    if (!s_status.initialized) {
        result = ESP_ERR_INVALID_STATE;
    } else if (s_reset_pending) {
        event->kind = TG_BLE_EVENT_RESET;
        event->reason = s_reset_reason;
        s_reset_pending = false;
    } else if (s_count) {
        *event = s_reports[s_head];
        s_head = (s_head + 1) % REPORT_CAPACITY;
        --s_count;
    } else if (s_complete_pending) {
        event->kind = TG_BLE_EVENT_COMPLETE;
        event->reason = s_complete_reason;
        s_complete_pending = false;
    } else {
        result = ESP_ERR_NOT_FOUND;
    }
    portEXIT_CRITICAL(&s_lock);
    return result;
}

esp_err_t tg_ble_scan_stop(void)
{
    tg_ble_status_t status;
    tg_ble_get_status(&status);
    if (!status.initialized || !status.ready) return ESP_ERR_INVALID_STATE;
    if (!status.scanning) return ESP_OK;
    int rc = ble_gap_disc_cancel();
    if (rc == 0 || rc == BLE_HS_EALREADY) return ESP_OK;
    portENTER_CRITICAL(&s_lock);
    s_status.last_host_error = rc;
    portEXIT_CRITICAL(&s_lock);
    return ESP_FAIL;
}

esp_err_t tg_ble_get_status(tg_ble_status_t *status)
{
    if (!status) return ESP_ERR_INVALID_ARG;
    portENTER_CRITICAL(&s_lock);
    *status = s_status;
    portEXIT_CRITICAL(&s_lock);
    return ESP_OK;
}

esp_err_t tg_ble_deinit(void)
{
    if (!s_status.initialized) return ESP_ERR_INVALID_STATE;
    int rc = nimble_port_stop();
    if (rc) return ESP_FAIL;
    esp_err_t err = nimble_port_deinit();
    if (err != ESP_OK) return err;
    portENTER_CRITICAL(&s_lock);
    memset(&s_status, 0, sizeof(s_status));
    s_head = s_count = 0;
    s_complete_pending = s_reset_pending = false;
    portEXIT_CRITICAL(&s_lock);
    return ESP_OK;
}
