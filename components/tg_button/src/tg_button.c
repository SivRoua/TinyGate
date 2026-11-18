#include "tg_button.h"
#include "button_state.h"
#include "driver/gpio.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "freertos/queue.h"
#include "freertos/semphr.h"

static QueueHandle_t s_events;
static SemaphoreHandle_t s_stopped;
static TaskHandle_t s_task;
static gpio_num_t s_gpio;
static tg_button_state_t s_state;
static portMUX_TYPE s_lock=portMUX_INITIALIZER_UNLOCKED;
static tg_button_status_t s_status;

static void emit(uint8_t kind,uint64_t now,uint32_t held)
{
    tg_button_event_t event={.kind=kind,.timestamp_ms=now,.held_ms=held};
    if (xQueueSend(s_events,&event,0)!=pdTRUE) {
        portENTER_CRITICAL(&s_lock);
        if (s_status.dropped_events!=UINT32_MAX) ++s_status.dropped_events;
        portEXIT_CRITICAL(&s_lock);
    }
}
static void sampling_task(void *arg)
{
    (void)arg;
    TickType_t interval=pdMS_TO_TICKS(10);
    if (!interval) interval=1;
    while (!ulTaskNotifyTake(pdTRUE,interval)) {
        uint64_t now=(uint64_t)esp_timer_get_time()/1000;
        uint8_t events=tg_button_state_sample(&s_state,gpio_get_level(s_gpio)==0,now);
        portENTER_CRITICAL(&s_lock);
        s_status.pressed=s_state.stable;
        portEXIT_CRITICAL(&s_lock);
        if (events&TG_BTN_EDGE_PRESS) emit(TG_BUTTON_PRESS,now,0);
        if (events&TG_BTN_LONG) emit(TG_BUTTON_LONG,now,s_state.held_ms);
        if (events&TG_BTN_SHORT) emit(TG_BUTTON_SHORT,now,s_state.held_ms);
        if (events&TG_BTN_EDGE_RELEASE) emit(TG_BUTTON_RELEASE,now,s_state.held_ms);
    }
    xSemaphoreGive(s_stopped);
    /* Caller deletes this task after acknowledgment, before releasing shared resources. */
    vTaskSuspend(NULL);
}
esp_err_t tg_button_init(void)
{
    const tg_button_config_t config={.gpio=9,.debounce_ms=30,.long_press_ms=1000};
    return tg_button_init_with_config(&config);
}
esp_err_t tg_button_init_with_config(const tg_button_config_t *config)
{
    if (!config || !GPIO_IS_VALID_GPIO(config->gpio) || config->debounce_ms<10 ||
        config->debounce_ms>200 || config->long_press_ms<config->debounce_ms ||
        config->long_press_ms>60000) return ESP_ERR_INVALID_ARG;
    if (s_events || s_task) return ESP_ERR_INVALID_STATE;
    s_events=xQueueCreate(16,sizeof(tg_button_event_t));
    s_stopped=xSemaphoreCreateBinary();
    if (!s_events || !s_stopped) {
        if (s_events) {vQueueDelete(s_events);s_events=NULL;}
        if (s_stopped) {vSemaphoreDelete(s_stopped);s_stopped=NULL;}
        return ESP_ERR_NO_MEM;
    }
    s_gpio=config->gpio;
    gpio_config_t gpio={.pin_bit_mask=1ULL<<config->gpio,.mode=GPIO_MODE_INPUT,
        .pull_up_en=GPIO_PULLUP_ENABLE,.pull_down_en=GPIO_PULLDOWN_DISABLE,.intr_type=GPIO_INTR_DISABLE};
    esp_err_t err=gpio_config(&gpio);
    if (err!=ESP_OK) goto fail;
    tg_button_state_init(&s_state,gpio_get_level(s_gpio)==0,(uint64_t)esp_timer_get_time()/1000,
        config->debounce_ms,config->long_press_ms);
    portENTER_CRITICAL(&s_lock);
    s_status=(tg_button_status_t){.initialized=true,.pressed=s_state.stable};
    portEXIT_CRITICAL(&s_lock);
    if (xTaskCreate(sampling_task,"tg_button",2048,NULL,3,&s_task)==pdPASS) return ESP_OK;
    err=ESP_ERR_NO_MEM;
    (void)gpio_reset_pin(s_gpio);
fail:
    vQueueDelete(s_events);s_events=NULL;
    vSemaphoreDelete(s_stopped);s_stopped=NULL;
    portENTER_CRITICAL(&s_lock);s_status=(tg_button_status_t){0};portEXIT_CRITICAL(&s_lock);
    return err;
}
esp_err_t tg_button_poll_event(tg_button_event_t *event)
{
    if (!event) return ESP_ERR_INVALID_ARG;
    if (!s_events) return ESP_ERR_INVALID_STATE;
    return xQueueReceive(s_events,event,0)==pdTRUE ? ESP_OK : ESP_ERR_NOT_FOUND;
}
esp_err_t tg_button_get_status(tg_button_status_t *status)
{
    if (!status) return ESP_ERR_INVALID_ARG;
    portENTER_CRITICAL(&s_lock);*status=s_status;portEXIT_CRITICAL(&s_lock);
    return ESP_OK;
}
esp_err_t tg_button_deinit(void)
{
    if (!s_task) return ESP_ERR_INVALID_STATE;
    xTaskNotifyGive(s_task);
    if (xSemaphoreTake(s_stopped,pdMS_TO_TICKS(500))!=pdTRUE) return ESP_ERR_TIMEOUT;
    vTaskDelete(s_task);s_task=NULL;
    (void)gpio_reset_pin(s_gpio);
    vQueueDelete(s_events);s_events=NULL;
    vSemaphoreDelete(s_stopped);s_stopped=NULL;
    portENTER_CRITICAL(&s_lock);s_status=(tg_button_status_t){0};portEXIT_CRITICAL(&s_lock);
    return ESP_OK;
}
