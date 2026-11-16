#include "rgb_color.h"
static uint8_t scale(uint8_t value,uint8_t brightness) {return ((unsigned)value*brightness+127)/255;}
void tg_rgb_pack(uint8_t r,uint8_t g,uint8_t b,uint8_t brightness,uint8_t out[3])
{out[0]=scale(g,brightness);out[1]=scale(r,brightness);out[2]=scale(b,brightness);}
