#pragma once
#include <stdbool.h>
#include <stdint.h>
#define TG_BTN_EDGE_PRESS 1u
#define TG_BTN_EDGE_RELEASE 2u
#define TG_BTN_SHORT 4u
#define TG_BTN_LONG 8u
typedef struct {
    bool stable;
    bool candidate;
    bool armed;
    bool long_sent;
    uint64_t candidate_since;
    uint64_t pressed_since;
    uint32_t held_ms;
    uint32_t debounce_ms;
    uint32_t long_ms;
} tg_button_state_t;
void tg_button_state_init(tg_button_state_t *state, bool pressed, uint64_t now,
                          uint32_t debounce_ms, uint32_t long_ms);
/* Called with monotonic ms. Returns a bitmask; does not allocate or touch GPIO. */
uint8_t tg_button_state_sample(tg_button_state_t *state, bool pressed, uint64_t now);
