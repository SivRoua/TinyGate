#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#ifdef __cplusplus
extern "C" {
#endif
#define TG_BUTTON_PRESS 1
#define TG_BUTTON_RELEASE 2
#define TG_BUTTON_SHORT 3
#define TG_BUTTON_LONG 4
typedef struct {
    int32_t gpio;
    uint32_t debounce_ms;
    uint32_t long_press_ms;
} tg_button_config_t;
typedef struct {
    uint8_t kind;
    uint64_t timestamp_ms; /* Monotonic uptime, not wall-clock time */
    uint32_t held_ms;
} tg_button_event_t;
typedef struct {
    bool initialized;
    bool pressed;         /* Debounced logical state */
    uint32_t dropped_events;
} tg_button_status_t;
/* Normally-open button between GPIO and GND; internal pull-up, active-low.
 * Single application caller, exclusive pin ownership. Never call from ISR.
 * GPIO9 is also BOOT: release during power-on/reset. Startup-held is suppressed
 * until a debounced release. Default: GPIO9, 30ms debounce, 1000ms long press. */
esp_err_t tg_button_init(void);
esp_err_t tg_button_init_with_config(const tg_button_config_t *config);
esp_err_t tg_button_poll_event(tg_button_event_t *event); /* NOT_FOUND: empty */
esp_err_t tg_button_get_status(tg_button_status_t *status);
esp_err_t tg_button_deinit(void); /* Bounded wait for sampling task, then free */
#ifdef __cplusplus
}
#endif
