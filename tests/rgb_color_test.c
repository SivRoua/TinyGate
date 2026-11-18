#include <assert.h>
#include <stdio.h>
#include "rgb_color.h"
int main(void){
    uint8_t data[3];tg_rgb_pack(255,0,128,255,data);
    assert(data[0]==0 && data[1]==255 && data[2]==128);
    tg_rgb_pack(255,255,255,24,data);assert(data[0]==24 && data[1]==24 && data[2]==24);
    tg_rgb_pack(255,128,1,0,data);assert(!data[0]&&!data[1]&&!data[2]);
    for(unsigned v=0;v<256;++v){tg_rgb_pack(v,v,v,128,data);assert(data[0]<=128 && data[0]==data[1] && data[1]==data[2]);}
    puts("RGB color/brightness tests passed");return 0;
}
