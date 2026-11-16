#include <assert.h>
#include <stdio.h>
#include "button_state.h"

static void short_with_bounce(void)
{
    tg_button_state_t s; tg_button_state_init(&s,false,0,30,1000);
    assert(!tg_button_state_sample(&s,true,100));
    assert(!tg_button_state_sample(&s,false,110));
    assert(!tg_button_state_sample(&s,true,120));
    assert(!tg_button_state_sample(&s,true,149));
    assert(tg_button_state_sample(&s,true,150)==TG_BTN_EDGE_PRESS);
    assert(!tg_button_state_sample(&s,false,600));
    assert(!tg_button_state_sample(&s,true,610));
    assert(!tg_button_state_sample(&s,false,620));
    assert(tg_button_state_sample(&s,false,650)==(TG_BTN_SHORT|TG_BTN_EDGE_RELEASE));
    assert(s.held_ms==500);
    assert(!tg_button_state_sample(&s,false,2000));
}
static void one_long_no_short(void)
{
    tg_button_state_t s; tg_button_state_init(&s,false,0,30,1000);
    assert(!tg_button_state_sample(&s,true,100));
    assert(tg_button_state_sample(&s,true,130)==TG_BTN_EDGE_PRESS);
    assert(!tg_button_state_sample(&s,true,1099));
    assert(tg_button_state_sample(&s,true,1100)==TG_BTN_LONG);
    assert(!tg_button_state_sample(&s,true,2000));
    assert(!tg_button_state_sample(&s,false,2100));
    assert(tg_button_state_sample(&s,false,2130)==TG_BTN_EDGE_RELEASE);
    assert(s.held_ms==2000);
}
static void release_near_threshold(void)
{
    tg_button_state_t s; tg_button_state_init(&s,false,0,30,1000);
    tg_button_state_sample(&s,true,100);
    assert(tg_button_state_sample(&s,true,130)==TG_BTN_EDGE_PRESS);
    assert(!tg_button_state_sample(&s,false,1090));
    assert(tg_button_state_sample(&s,false,1120)==(TG_BTN_SHORT|TG_BTN_EDGE_RELEASE));
    assert(s.held_ms==990);
    tg_button_state_sample(&s,true,2000);
    assert(tg_button_state_sample(&s,true,2030)==TG_BTN_EDGE_PRESS);
    assert(!tg_button_state_sample(&s,false,3000));
    assert(tg_button_state_sample(&s,false,3030)==(TG_BTN_LONG|TG_BTN_EDGE_RELEASE));
    assert(!tg_button_state_sample(&s,false,4000));
}
static void boot_held_and_large_clock(void)
{
    tg_button_state_t s; tg_button_state_init(&s,true,0,30,1000);
    assert(!tg_button_state_sample(&s,true,2000));
    tg_button_state_sample(&s,false,2010);
    assert(!tg_button_state_sample(&s,false,2040));
    assert(s.armed && !s.stable);
    const uint64_t base=(uint64_t)UINT32_MAX+100;
    tg_button_state_sample(&s,true,base);
    assert(tg_button_state_sample(&s,true,base+30)==TG_BTN_EDGE_PRESS);
    assert(tg_button_state_sample(&s,true,base+1000)==TG_BTN_LONG);
    tg_button_state_sample(&s,false,base+(uint64_t)UINT32_MAX+100);
    assert(tg_button_state_sample(&s,false,base+(uint64_t)UINT32_MAX+130)==TG_BTN_EDGE_RELEASE);
    assert(s.held_ms==UINT32_MAX);
}
int main(void)
{
    short_with_bounce(); one_long_no_short(); release_near_threshold(); boot_held_and_large_clock();
    puts("Button tests passed: bounce, short/long exclusivity, threshold, startup-held, 64-bit time");
    return 0;
}
