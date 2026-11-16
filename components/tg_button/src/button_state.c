#include "button_state.h"
#include <limits.h>
static uint32_t duration(uint64_t elapsed)
{
    return elapsed > UINT32_MAX ? UINT32_MAX : (uint32_t)elapsed;
}
void tg_button_state_init(tg_button_state_t *s,bool pressed,uint64_t now,uint32_t debounce,uint32_t long_ms)
{
    *s=(tg_button_state_t){.stable=pressed,.candidate=pressed,.armed=!pressed,
        .candidate_since=now,.pressed_since=now,.debounce_ms=debounce,.long_ms=long_ms};
}
uint8_t tg_button_state_sample(tg_button_state_t *s,bool pressed,uint64_t now)
{
    uint8_t events=0;
    if (pressed!=s->candidate) {s->candidate=pressed;s->candidate_since=now;}
    if (s->stable!=s->candidate && now-s->candidate_since>=s->debounce_ms) {
        s->stable=s->candidate;
        if (!s->armed) {
            if (!s->stable) s->armed=true;
            return 0;
        }
        if (s->stable) {
            s->pressed_since=s->candidate_since;
            s->held_ms=0;
            s->long_sent=false;
            events|=TG_BTN_EDGE_PRESS;
        } else {
            s->held_ms=duration(s->candidate_since-s->pressed_since);
            if (!s->long_sent) events|=s->held_ms>=s->long_ms ? TG_BTN_LONG : TG_BTN_SHORT;
            events|=TG_BTN_EDGE_RELEASE;
        }
    }
    /* A possible release must not turn a just-short-of-threshold press into a long press. */
    if (s->armed && s->stable && pressed && !s->long_sent && now-s->pressed_since>=s->long_ms) {
        s->long_sent=true;
        s->held_ms=duration(now-s->pressed_since);
        events|=TG_BTN_LONG;
    }
    return events;
}
