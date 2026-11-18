#include "tg_ble.h"
#include "gatt_internal.h"
#include <string.h>
#include "host/ble_hs.h"
#include "os/os_mbuf.h"
#include "freertos/FreeRTOS.h"

static portMUX_TYPE s_lock=portMUX_INITIALIZER_UNLOCKED;
static struct {bool used,busy;uint16_t handle;uint32_t id;} s_links[2];
static tg_ble_gatt_event_t s_queue[32];
static unsigned s_head,s_count;
static tg_ble_links_t s_status;
static int slot(uint16_t handle) {
    for (int i=0;i<2;++i) if(s_links[i].used && s_links[i].handle==handle) return i;return -1;
}
static void emit(tg_ble_gatt_event_t event) {
    portENTER_CRITICAL(&s_lock);
    if(s_count<32) {s_queue[(s_head+s_count)%32]=event;++s_count;}
    else if(s_status.dropped_events!=UINT32_MAX) ++s_status.dropped_events;
    portEXIT_CRITICAL(&s_lock);
}
void tg_gatt_reset(void) {
    portENTER_CRITICAL(&s_lock);memset(s_links,0,sizeof(s_links));
    s_status=(tg_ble_links_t){0};s_head=s_count=0;portEXIT_CRITICAL(&s_lock);
}
static esp_err_t result(int rc) {return rc==0?ESP_OK:ESP_FAIL;}
static void finish(uint16_t conn,uint32_t id,int status) {
    portENTER_CRITICAL(&s_lock);int i=slot(conn);
    if(i>=0 && s_links[i].id==id) s_links[i].busy=false;portEXIT_CRITICAL(&s_lock);
    emit((tg_ble_gatt_event_t){.kind=TG_BLE_OPERATION_DONE,.connection=conn,.request_id=id,
        .status=status==BLE_HS_EDONE?0:status});
}
static esp_err_t begin(uint16_t conn,uint32_t id) {
    if(!id) return ESP_ERR_INVALID_ARG;
    portENTER_CRITICAL(&s_lock);int i=slot(conn);
    bool allowed=i>=0 && !s_links[i].busy;
    if(allowed) {s_links[i].busy=true;s_links[i].id=id;}portEXIT_CRITICAL(&s_lock);
    return allowed?ESP_OK:ESP_ERR_INVALID_STATE;
}
static int submit(uint16_t conn,uint32_t id,int rc) {if(rc) finish(conn,id,rc);return result(rc);}
static void copy_value(tg_ble_gatt_event_t *e,const struct os_mbuf *om) {
    uint16_t length=OS_MBUF_PKTLEN(om);e->truncated=length>sizeof(e->data);
    e->data_len=length>sizeof(e->data)?sizeof(e->data):length;
    if(os_mbuf_copydata(om,0,e->data_len,e->data)) {e->status=BLE_HS_EBADDATA;e->data_len=0;}
}
void tg_gatt_gap(struct ble_gap_event *event) {
    if(event->type==BLE_GAP_EVENT_CONNECT) {
        portENTER_CRITICAL(&s_lock);s_status.connecting=false;
        if(!event->connect.status) for(unsigned i=0;i<2;++i) if(!s_links[i].used) {
            s_links[i].used=true;s_links[i].handle=event->connect.conn_handle;++s_status.links;break;
        }
        portEXIT_CRITICAL(&s_lock);
        emit((tg_ble_gatt_event_t){.kind=TG_BLE_LINK,.connection=event->connect.conn_handle,.status=event->connect.status});
    } else if(event->type==BLE_GAP_EVENT_DISCONNECT) {
        uint16_t conn=event->disconnect.conn.conn_handle;
        portENTER_CRITICAL(&s_lock);int i=slot(conn);
        if(i>=0) {memset(&s_links[i],0,sizeof(s_links[i]));--s_status.links;}portEXIT_CRITICAL(&s_lock);
        emit((tg_ble_gatt_event_t){.kind=TG_BLE_UNLINK,.connection=conn,.status=event->disconnect.reason});
    } else if(event->type==BLE_GAP_EVENT_NOTIFY_RX) {
        tg_ble_gatt_event_t e={.kind=TG_BLE_NOTIFICATION,.connection=event->notify_rx.conn_handle,
            .handle=event->notify_rx.attr_handle};
        copy_value(&e,event->notify_rx.om);emit(e);
    }
}
static int link_gap(struct ble_gap_event *event,void *arg) {(void)arg;tg_gatt_gap(event);return 0;}
esp_err_t tg_ble_connect(const uint8_t address[6],uint8_t type) {
    if(!address || type>3) return ESP_ERR_INVALID_ARG;
    tg_ble_status_t status;tg_ble_get_status(&status);
    if(!status.ready || status.scanning) return ESP_ERR_INVALID_STATE;
    portENTER_CRITICAL(&s_lock);bool allowed=!s_status.connecting && s_status.links<2;
    if(allowed) s_status.connecting=true;portEXIT_CRITICAL(&s_lock);
    if(!allowed) return ESP_ERR_INVALID_STATE;
    ble_addr_t peer={.type=type};memcpy(peer.val,address,6);uint8_t own;
    int rc=ble_hs_id_infer_auto(0,&own);
    if(!rc) rc=ble_gap_connect(own,&peer,10000,NULL,link_gap,NULL);
    if(rc) {portENTER_CRITICAL(&s_lock);s_status.connecting=false;portEXIT_CRITICAL(&s_lock);}
    return result(rc);
}
esp_err_t tg_ble_connect_cancel(void) {
    tg_ble_status_t status;tg_ble_get_status(&status);if(!status.ready) return ESP_ERR_INVALID_STATE;
    return result(ble_gap_conn_cancel());
}
esp_err_t tg_ble_disconnect(uint16_t conn) {
    portENTER_CRITICAL(&s_lock);bool exists=slot(conn)>=0;portEXIT_CRITICAL(&s_lock);
    if(!exists) return ESP_ERR_INVALID_STATE;
    return result(ble_gap_terminate(conn,BLE_ERR_REM_USER_CONN_TERM));
}
static int service_cb(uint16_t conn,const struct ble_gatt_error *err,const struct ble_gatt_svc *svc,void *ctx) {
    uint32_t id=(uint32_t)(uintptr_t)ctx;
    if(err->status) {finish(conn,id,err->status);return 0;}
    tg_ble_gatt_event_t e={.kind=TG_BLE_SERVICE,.connection=conn,.request_id=id,.handle=svc->start_handle,.end_handle=svc->end_handle};
    ble_uuid_to_str(&svc->uuid.u,e.uuid);emit(e);return 0;
}
static int chr_cb(uint16_t conn,const struct ble_gatt_error *err,const struct ble_gatt_chr *chr,void *ctx) {
    uint32_t id=(uint32_t)(uintptr_t)ctx;
    if(err->status) {finish(conn,id,err->status);return 0;}
    tg_ble_gatt_event_t e={.kind=TG_BLE_CHARACTERISTIC,.connection=conn,.request_id=id,
        .handle=chr->val_handle,.end_handle=chr->def_handle,.properties=chr->properties};
    ble_uuid_to_str(&chr->uuid.u,e.uuid);emit(e);return 0;
}
static int dsc_cb(uint16_t conn,const struct ble_gatt_error *err,uint16_t chr_handle,const struct ble_gatt_dsc *dsc,void *ctx) {
    (void)chr_handle;uint32_t id=(uint32_t)(uintptr_t)ctx;
    if(err->status) {finish(conn,id,err->status);return 0;}
    tg_ble_gatt_event_t e={.kind=TG_BLE_DESCRIPTOR,.connection=conn,.request_id=id,.handle=dsc->handle};
    ble_uuid_to_str(&dsc->uuid.u,e.uuid);emit(e);return 0;
}
static int value_cb(uint16_t conn,const struct ble_gatt_error *err,struct ble_gatt_attr *attr,void *ctx) {
    uint32_t id=(uint32_t)(uintptr_t)ctx;
    if(!err->status && attr && attr->om) {
        tg_ble_gatt_event_t e={.kind=TG_BLE_VALUE,.connection=conn,.request_id=id,.handle=attr->handle};
        copy_value(&e,attr->om);emit(e);
    }
    finish(conn,id,err->status);return 0;
}
esp_err_t tg_ble_services(uint16_t conn,uint32_t id) {
    esp_err_t err=begin(conn,id);if(err) return err;
    return submit(conn,id,ble_gattc_disc_all_svcs(conn,service_cb,(void *)(uintptr_t)id));
}
esp_err_t tg_ble_characteristics(uint16_t conn,uint16_t start,uint16_t end,uint32_t id) {
    if(!start || start>end) return ESP_ERR_INVALID_ARG;
    esp_err_t err=begin(conn,id);if(err) return err;
    return submit(conn,id,ble_gattc_disc_all_chrs(conn,start,end,chr_cb,(void *)(uintptr_t)id));
}
esp_err_t tg_ble_descriptors(uint16_t conn,uint16_t start,uint16_t end,uint32_t id) {
    if(!start || start>=end) return ESP_ERR_INVALID_ARG;
    esp_err_t err=begin(conn,id);if(err) return err;
    return submit(conn,id,ble_gattc_disc_all_dscs(conn,start,end,dsc_cb,(void *)(uintptr_t)id));
}
esp_err_t tg_ble_read(uint16_t conn,uint16_t handle,uint32_t id) {
    if(!handle) return ESP_ERR_INVALID_ARG;
    esp_err_t err=begin(conn,id);if(err) return err;
    return submit(conn,id,ble_gattc_read(conn,handle,value_cb,(void *)(uintptr_t)id));
}
esp_err_t tg_ble_write(uint16_t conn,uint16_t handle,const uint8_t *data,uint16_t length,uint32_t id) {
    if(!handle || !data || !length || length>128) return ESP_ERR_INVALID_ARG;
    esp_err_t err=begin(conn,id);if(err) return err;
    return submit(conn,id,ble_gattc_write_flat(conn,handle,data,length,value_cb,(void *)(uintptr_t)id));
}
esp_err_t tg_ble_subscribe(uint16_t conn,uint16_t handle,uint8_t mode,uint32_t id) {
    if(mode>2) return ESP_ERR_INVALID_ARG;
    const uint8_t data[]={mode,0};return tg_ble_write(conn,handle,data,2,id);
}
esp_err_t tg_ble_poll_gatt(tg_ble_gatt_event_t *event) {
    if(!event) return ESP_ERR_INVALID_ARG;
    tg_ble_status_t status;tg_ble_get_status(&status);if(!status.initialized) return ESP_ERR_INVALID_STATE;
    portENTER_CRITICAL(&s_lock);bool available=s_count>0;
    if(available) {*event=s_queue[s_head];s_head=(s_head+1)%32;--s_count;}portEXIT_CRITICAL(&s_lock);
    return available?ESP_OK:ESP_ERR_NOT_FOUND;
}
esp_err_t tg_ble_get_links(tg_ble_links_t *status) {
    if(!status) return ESP_ERR_INVALID_ARG;
    portENTER_CRITICAL(&s_lock);*status=s_status;
    for(unsigned i=0;i<2;++i) status->handles[i]=s_links[i].used?s_links[i].handle:0xffff;
    portEXIT_CRITICAL(&s_lock);return ESP_OK;
}
