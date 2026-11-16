#include "tg_zigbee.h"
#include <string.h>
#include "sdkconfig.h"
#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#include "freertos/task.h"
#include "nvs_flash.h"
#include "esp_log.h"
#include "esp_zigbee.h"
#include "ezbee/zha.h"

static portMUX_TYPE s_lock = portMUX_INITIALIZER_UNLOCKED;
static tg_zigbee_status_t s_status;
static QueueHandle_t s_events;
static const char *TAG = "TG-Zigbee";

static void emit(tg_zigbee_event_t event)
{
    if (xQueueSend(s_events, &event, 0) != pdTRUE) {
        portENTER_CRITICAL(&s_lock);
        ++s_status.dropped_events;
        portEXIT_CRITICAL(&s_lock);
    }
}
static void error_event(int error)
{
    if (!error) return;
    portENTER_CRITICAL(&s_lock);
    s_status.last_error = error;
    portEXIT_CRITICAL(&s_lock);
    emit((tg_zigbee_event_t){.kind=TG_ZB_ERROR, .status=error});
}
static void peer_seen(uint16_t address)
{
    portENTER_CRITICAL(&s_lock);
    s_status.peer_address = address;
    portEXIT_CRITICAL(&s_lock);
}
static void network_ready(void)
{
    uint8_t channel=ezb_nwk_get_current_channel();
    uint16_t pan=ezb_nwk_get_panid();
    portENTER_CRITICAL(&s_lock);
    s_status.ready = true;
    s_status.channel = channel;
    s_status.pan_id = pan;
    s_status.last_error = 0;
    portEXIT_CRITICAL(&s_lock);
    emit((tg_zigbee_event_t){.kind=TG_ZB_NETWORK});
}

static void descriptor_result(const ezb_zdo_simple_desc_req_result_t *result, void *ctx)
{
    uint16_t address = (uint16_t)(uintptr_t)ctx;
    if (!result || result->error || !result->rsp || result->rsp->status) {
        emit((tg_zigbee_event_t){.kind=TG_ZB_ERROR, .short_address=address,
             .status=result ? (result->error ? result->error : ESP_FAIL) : ESP_FAIL});
        return;
    }
    const ezb_af_simple_desc_t *d = &result->rsp->desc;
    for (unsigned i=0; d->app_cluster_list && i<d->app_input_cluster_count; ++i) {
        emit((tg_zigbee_event_t){.kind=TG_ZB_ENDPOINT, .short_address=address,
            .endpoint=d->ep_id, .profile=d->app_profile_id, .cluster=d->app_cluster_list[i]});
    }
}
static void endpoints_result(const ezb_zdo_active_ep_req_result_t *result, void *ctx)
{
    uint16_t address = (uint16_t)(uintptr_t)ctx;
    if (!result || result->error || !result->rsp || result->rsp->status) {
        emit((tg_zigbee_event_t){.kind=TG_ZB_ERROR, .short_address=address,
             .status=result ? (result->error ? result->error : ESP_FAIL) : ESP_FAIL});
        return;
    }
    /* Bound discovery work; this fixture uses two endpoints. */
    for (unsigned i=0; result->rsp->active_ep_list && i<result->rsp->active_ep_count && i<8; ++i) {
        ezb_zdo_simple_desc_req_t req = {
            .dst_nwk_addr=address, .field={.nwk_addr_of_interest=address,
                .endpoint=result->rsp->active_ep_list[i]},
            .cb=descriptor_result, .user_ctx=ctx,
        };
        error_event(ezb_zdo_simple_desc_req(&req));
    }
}
static ezb_err_t discover_locked(uint16_t address)
{
    ezb_zdo_active_ep_req_t req = {
        .dst_nwk_addr=address, .field={.nwk_addr_of_interest=address},
        .cb=endpoints_result, .user_ctx=(void *)(uintptr_t)address,
    };
    return ezb_zdo_active_ep_req(&req);
}

static bool app_signal(const ezb_app_signal_t *signal)
{
    ezb_app_signal_type_t type = ezb_app_signal_get_type(signal);
    const void *params = ezb_app_signal_get_params(signal);
    switch (type) {
    case EZB_ZDO_SIGNAL_SKIP_STARTUP:
        error_event(ezb_bdb_start_top_level_commissioning(EZB_BDB_MODE_INITIALIZATION));
        break;
    case EZB_BDB_SIGNAL_DEVICE_FIRST_START:
    case EZB_BDB_SIGNAL_DEVICE_REBOOT:
        if (params && *(ezb_bdb_comm_status_t *)params == EZB_BDB_STATUS_SUCCESS) {
            if (ezb_bdb_is_factory_new())
                error_event(ezb_bdb_start_top_level_commissioning(EZB_BDB_MODE_NETWORK_FORMATION));
            else network_ready();
        } else error_event(params ? *(ezb_bdb_comm_status_t *)params : ESP_FAIL);
        break;
    case EZB_BDB_SIGNAL_FORMATION:
        if (params && *(ezb_bdb_comm_status_t *)params == EZB_BDB_STATUS_SUCCESS) network_ready();
        else error_event(params ? *(ezb_bdb_comm_status_t *)params : ESP_FAIL);
        break;
    case EZB_NWK_SIGNAL_PERMIT_JOIN_STATUS:
        portENTER_CRITICAL(&s_lock);
        s_status.join_open = params && *(uint8_t *)params != 0;
        portEXIT_CRITICAL(&s_lock);
        break;
    case EZB_ZDO_SIGNAL_DEVICE_ANNCE: {
        const ezb_zdo_signal_device_annce_params_t *p = params;
        if (!p) break;
        tg_zigbee_event_t event = {.kind=TG_ZB_JOIN, .short_address=p->short_addr};
        memcpy(event.ieee_address, p->device_addr.u8, 8);
        peer_seen(p->short_addr);
        emit(event);
        error_event(discover_locked(p->short_addr));
        break;
    }
    case EZB_ZDO_SIGNAL_LEAVE_INDICATION: {
        const ezb_zdo_signal_leave_indication_params_t *p = params;
        if (!p) break;
        tg_zigbee_event_t event = {.kind=TG_ZB_LEAVE, .short_address=p->short_addr};
        memcpy(event.ieee_address, p->device_addr.u8, 8);
        portENTER_CRITICAL(&s_lock);
        if (s_status.peer_address == p->short_addr) s_status.peer_address = 0xffff;
        portEXIT_CRITICAL(&s_lock);
        emit(event);
        break;
    }
    default: break;
    }
    return true;
}

static tg_zigbee_event_t from_header(uint8_t kind, const ezb_zcl_cmd_hdr_t *h)
{
    tg_zigbee_event_t event = {.kind=kind, .short_address=0xffff};
    if (h) {
        if (h->src_addr.addr_mode == EZB_ADDR_MODE_SHORT) event.short_address=h->src_addr.u.short_addr;
        event.endpoint=h->src_ep;
        event.cluster=h->cluster_id;
        event.tsn=h->tsn;
    }
    return event;
}
static void attribute_event(const ezb_zcl_cmd_hdr_t *h, uint16_t attribute,
                            uint8_t type, const void *value, uint8_t status, bool report)
{
    if (!h || (h->cluster_id != TG_ZB_TEMPERATURE_CLUSTER && h->cluster_id != TG_ZB_ONOFF_CLUSTER)) return;
    tg_zigbee_event_t event=from_header(TG_ZB_ATTRIBUTE, h);
    event.attribute=attribute;
    event.status=status;
    event.is_report=report;
    if (!status && attribute==0xf000 && h->cluster_id==TG_ZB_TEMPERATURE_CLUSTER &&
        type==EZB_ZCL_ATTR_TYPE_UINT64 && value) {
        memcpy(&event.test_payload,value,8);emit(event);return;
    }
    if (!status && attribute == 0 && value) {
        if (h->cluster_id == TG_ZB_TEMPERATURE_CLUSTER && type == EZB_ZCL_ATTR_TYPE_INT16) {
            int16_t temperature;
            memcpy(&temperature,value,sizeof(temperature));
            if (temperature == INT16_MIN) event.status=ESP_ERR_INVALID_RESPONSE;
            else event.value=temperature;
        } else if (h->cluster_id == TG_ZB_ONOFF_CLUSTER && type == EZB_ZCL_ATTR_TYPE_BOOL) {
            event.value=*(const uint8_t *)value;
            if (event.value > 1) event.status=ESP_ERR_INVALID_RESPONSE;
        } else event.status=ESP_ERR_NOT_SUPPORTED;
    } else if (!status) event.status=ESP_ERR_INVALID_RESPONSE;
    if (event.short_address < 0xfff8) peer_seen(event.short_address);
    emit(event);
}
static void zcl_action(ezb_zcl_core_action_callback_id_t id, void *message)
{
    if (!message) return;
    if (id == EZB_ZCL_CORE_REPORT_ATTR_CB_ID) {
        ezb_zcl_cmd_report_attr_message_t *m=message;
        for (ezb_zcl_report_attr_variable_t *v=m->in.variables; v; v=v->next)
            attribute_event(m->in.header,v->attr_id,v->attr_type,v->attr_value,0,true);
    } else if (id == EZB_ZCL_CORE_READ_ATTR_RSP_CB_ID) {
        ezb_zcl_cmd_read_attr_rsp_message_t *m=message;
        for (ezb_zcl_read_attr_rsp_variable_t *v=m->in.variables; v; v=v->next)
            attribute_event(m->in.header,v->attr_id,v->attr_type,v->attr_value,v->status,false);
    } else if (id == EZB_ZCL_CORE_DEFAULT_RSP_CB_ID) {
        ezb_zcl_cmd_default_rsp_message_t *m=message;
        tg_zigbee_event_t event=from_header(TG_ZB_DEFAULT_RESPONSE,m->in.header);
        event.status=m->in.status_code;
        event.value=m->in.rsp_to_cmd;
        emit(event);
    } else if (id == EZB_ZCL_CORE_CONFIG_REPORT_RSP_CB_ID) {
        ezb_zcl_cmd_config_report_rsp_message_t *m=message;
        for (ezb_zcl_config_report_rsp_variable_t *v=m->in.variables; v; v=v->next) {
            tg_zigbee_event_t event=from_header(TG_ZB_REPORT_CONFIG,m->in.header);
            event.attribute=v->attr_id; event.status=v->status;
            emit(event);
        }
    }
}
static void tx_confirm(ezb_af_user_cnf_t *cnf, void *ctx)
{
    if (!cnf) return;
    emit((tg_zigbee_event_t){.kind=TG_ZB_TX_CONFIRM, .request_id=(uint32_t)(uintptr_t)ctx,
        .short_address=cnf->dst_addr.u.short_addr, .endpoint=cnf->dst_ep,
        .cluster=cnf->cluster_id, .tsn=cnf->tsn, .status=cnf->status});
}

static void stack_task(void *arg)
{
    (void)arg;
    esp_zigbee_config_t config = {
        .device_config={.device_type=EZB_NWK_DEVICE_TYPE_COORDINATOR, .zczr_config={.max_children=10}},
        .platform_config={.storage_partition_name="zb_nvs", .radio_config={.radio_mode=ESP_ZIGBEE_RADIO_MODE_NATIVE}},
    };
    esp_err_t err=esp_zigbee_init(&config);
    if (err) goto fail;
    ezb_aps_secur_enable_distributed_security(false);
    if ((err=esp_zigbee_err_to_esp(ezb_bdb_set_primary_channel_set(1UL << CONFIG_TG_ZIGBEE_CHANNEL)))) goto fail;
    if ((err=esp_zigbee_err_to_esp(ezb_bdb_set_secondary_channel_set(0)))) goto fail;
    if ((err=esp_zigbee_err_to_esp(ezb_app_signal_add_handler(app_signal)))) goto fail;
    ezb_af_device_desc_t device=ezb_af_create_device_desc();
    ezb_zha_on_off_switch_config_t switch_config=EZB_ZHA_ON_OFF_SWITCH_CONFIG();
    ezb_af_ep_desc_t endpoint=ezb_zha_create_on_off_switch(1,&switch_config);
    if (!device || !endpoint) { err=ESP_ERR_NO_MEM; goto fail; }
    ezb_zcl_cluster_desc_t temperature=ezb_zcl_temperature_measurement_create_cluster_desc(NULL,EZB_ZCL_CLUSTER_CLIENT);
    if (!temperature) { err=ESP_ERR_NO_MEM; goto fail; }
    if ((err=esp_zigbee_err_to_esp(ezb_af_endpoint_add_cluster_desc(endpoint,temperature)))) goto fail;
    if ((err=esp_zigbee_err_to_esp(ezb_af_device_add_endpoint_desc(device,endpoint)))) goto fail;
    if ((err=esp_zigbee_err_to_esp(ezb_af_device_desc_register(device)))) goto fail;
    ezb_zcl_core_action_handler_register(zcl_action);
    if ((err=esp_zigbee_start(false))) goto fail;
    err=esp_zigbee_launch_mainloop();
fail:
    portENTER_CRITICAL(&s_lock);
    s_status.ready=false;
    portEXIT_CRITICAL(&s_lock);
    error_event(err ? err : ESP_FAIL);
    ESP_LOGE(TAG,"Stack stopped/error: %d; reboot required",err);
    vTaskDelete(NULL);
}

esp_err_t tg_zigbee_init(void)
{
    if (s_events) return ESP_ERR_INVALID_STATE;
    esp_err_t err=nvs_flash_init_partition("zb_nvs");
    if (err) return err; /* Never erase settings implicitly. */
    s_events=xQueueCreate(32,sizeof(tg_zigbee_event_t));
    if (!s_events) return ESP_ERR_NO_MEM;
    s_status=(tg_zigbee_status_t){.started=true,.peer_address=0xffff};
    if (xTaskCreate(stack_task,"tg_zigbee",6144,NULL,5,NULL) != pdPASS) {
        vQueueDelete(s_events); s_events=NULL; s_status.started=false;
        return ESP_ERR_NO_MEM;
    }
    return ESP_OK;
}
esp_err_t tg_zigbee_get_status(tg_zigbee_status_t *status)
{
    if (!status) return ESP_ERR_INVALID_ARG;
    portENTER_CRITICAL(&s_lock); *status=s_status; portEXIT_CRITICAL(&s_lock);
    return ESP_OK;
}
esp_err_t tg_zigbee_poll_event(tg_zigbee_event_t *event)
{
    if (!event) return ESP_ERR_INVALID_ARG;
    if (!s_events) return ESP_ERR_INVALID_STATE;
    return xQueueReceive(s_events,event,0) == pdTRUE ? ESP_OK : ESP_ERR_NOT_FOUND;
}
static esp_err_t lock_ready(void)
{
    tg_zigbee_status_t status; tg_zigbee_get_status(&status);
    if (!status.ready) return ESP_ERR_INVALID_STATE;
    return esp_zigbee_lock_acquire(pdMS_TO_TICKS(1000)) ? ESP_OK : ESP_ERR_TIMEOUT;
}
esp_err_t tg_zigbee_permit_join(uint8_t seconds)
{
    if (seconds == 255) return ESP_ERR_INVALID_ARG;
    esp_err_t err=lock_ready(); if (err) return err;
    err=esp_zigbee_err_to_esp(ezb_bdb_open_network(seconds));
    esp_zigbee_lock_release(); return err;
}
esp_err_t tg_zigbee_discover(uint16_t address)
{
    if (!address || address >= 0xfff8) return ESP_ERR_INVALID_ARG;
    esp_err_t err=lock_ready(); if (err) return err;
    err=esp_zigbee_err_to_esp(discover_locked(address));
    esp_zigbee_lock_release(); return err;
}
static bool valid_target(uint16_t address,uint8_t ep,uint16_t cluster,uint32_t id)
{
    return address && address < 0xfff8 && ep && ep <= 240 && id &&
        (cluster == TG_ZB_TEMPERATURE_CLUSTER || cluster == TG_ZB_ONOFF_CLUSTER);
}
static ezb_zcl_cmd_ctrl_t control(uint16_t address,uint8_t ep,uint16_t cluster,uint32_t id)
{
    return (ezb_zcl_cmd_ctrl_t){.dst_addr={.addr_mode=EZB_ADDR_MODE_SHORT,.u.short_addr=address},
        .dst_ep=ep,.src_ep=1,.cluster_id=cluster,.manuf_code=EZB_ZCL_STD_MANUF_CODE,
        .cnf_ctx={.cb=tx_confirm,.user_ctx=(void *)(uintptr_t)id}};
}
esp_err_t tg_zigbee_read_attribute(uint16_t address,uint8_t ep,uint16_t cluster,uint32_t id)
{
    if (!valid_target(address,ep,cluster,id)) return ESP_ERR_INVALID_ARG;
    esp_err_t err=lock_ready(); if (err) return err;
    uint16_t attribute=0;
    ezb_zcl_read_attr_cmd_t req={.cmd_ctrl=control(address,ep,cluster,id),
        .payload={.attr_number=1,.attr_field=&attribute}};
    err=esp_zigbee_err_to_esp(ezb_zcl_read_attr_cmd_req(&req));
    esp_zigbee_lock_release(); return err;
}
esp_err_t tg_zigbee_send_onoff(uint16_t address,uint8_t ep,uint8_t command,uint32_t id)
{
    if (!valid_target(address,ep,TG_ZB_ONOFF_CLUSTER,id) || command > 2) return ESP_ERR_INVALID_ARG;
    esp_err_t err=lock_ready(); if (err) return err;
    ezb_zcl_on_off_cmd_t req={.cmd_ctrl={
        .dst_addr={.addr_mode=EZB_ADDR_MODE_SHORT,.u.short_addr=address},.src_ep=1,.dst_ep=ep,
        .cnf_ctx={.cb=tx_confirm,.user_ctx=(void *)(uintptr_t)id}}};
    ezb_err_t result=command == 0 ? ezb_zcl_on_off_off_cmd_req(&req) :
        command == 1 ? ezb_zcl_on_off_on_cmd_req(&req) : ezb_zcl_on_off_toggle_cmd_req(&req);
    esp_zigbee_lock_release(); return esp_zigbee_err_to_esp(result);
}
esp_err_t tg_zigbee_configure_reporting(uint16_t address,uint8_t ep,uint16_t cluster,uint32_t id)
{
    if (!valid_target(address,ep,cluster,id)) return ESP_ERR_INVALID_ARG;
    esp_err_t err=lock_ready(); if (err) return err;
    ezb_zcl_config_report_record_t record={.direction=EZB_ZCL_REPORTING_SEND,.attr_id=0,
        .client={.attr_type=cluster == TG_ZB_TEMPERATURE_CLUSTER ? EZB_ZCL_ATTR_TYPE_INT16 : EZB_ZCL_ATTR_TYPE_BOOL,
            .min_interval=1,.max_interval=5,.reportable_change.s16=50}};
    ezb_zcl_config_report_cmd_t req={.cmd_ctrl=control(address,ep,cluster,id),
        .payload={.record_number=1,.record_field=&record}};
    err=esp_zigbee_err_to_esp(ezb_zcl_config_report_cmd_req(&req));
    esp_zigbee_lock_release(); return err;
}
