#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "display_canvas.h"

int main(void)
{
    struct { unsigned char before[16], frame[TG_CANVAS_SIZE], after[16]; } guarded;
    memset(&guarded, 0xa5, sizeof(guarded));
    memset(guarded.frame, 0, sizeof(guarded.frame));
    /* SSD1306 page boundaries, including the very last display pixel. */
    assert(tg_canvas_pixel(guarded.frame, 0, 7, true));
    assert(tg_canvas_pixel(guarded.frame, 0, 8, true));
    assert(tg_canvas_pixel(guarded.frame, 127, 63, true));
    assert(guarded.frame[0] == 0x80);
    assert(guarded.frame[128] == 1);
    assert(guarded.frame[1023] == 0x80);
    assert(tg_canvas_pixel(guarded.frame, 127, 63, false));
    assert(guarded.frame[1023] == 0);
    assert(!tg_canvas_pixel(guarded.frame, 128, 0, true));
    assert(!tg_canvas_pixel(guarded.frame, 0, 64, true));
    assert(!tg_canvas_pixel(NULL, 0, 0, true));

    /* A long bottom row must clip without touching the surrounding memory. */
    char long_text[1000];
    memset(long_text, 'A', sizeof(long_text)-1);
    long_text[sizeof(long_text)-1] = 0;
    assert(tg_canvas_line(guarded.frame, 7, long_text));
    assert(guarded.frame[7*128 + 120] != 0);
    assert(guarded.frame[7*128 + 125] == 0); /* column gap */
    assert(guarded.frame[7*128 + 126] == 0);
    assert(guarded.frame[7*128 + 127] == 0);
    assert(tg_canvas_line(guarded.frame, 7, ""));
    for (unsigned i = 7*128; i < 1024; ++i) assert(guarded.frame[i] == 0);
    assert(guarded.frame[0] == 0x80 && guarded.frame[128] == 1);
    assert(!tg_canvas_line(guarded.frame, 8, "INVALID"));
    assert(!tg_canvas_line(guarded.frame, 0, NULL));
    assert(tg_canvas_line(guarded.frame, 2, "abc123"));
    assert(tg_canvas_line(guarded.frame, 3, "ABC123"));
    assert(memcmp(guarded.frame + 2*128, guarded.frame + 3*128, 128) == 0);
    assert(tg_canvas_line(guarded.frame, 4, "\xff"));
    assert(tg_canvas_line(guarded.frame, 5, "?"));
    assert(memcmp(guarded.frame + 4*128, guarded.frame + 5*128, 128) == 0);
    for (unsigned i = 0; i < 16; ++i) {
        assert(guarded.before[i] == 0xa5 && guarded.after[i] == 0xa5);
    }
    puts("OLED canvas tests passed: pages, clipping, line replacement, glyph fallback, guards");
    return 0;
}
