#pragma once
#include <stdbool.h>
#include <stdint.h>
#define TG_CANVAS_SIZE 1024
/* Private, hardware-independent page-layout renderer. */
bool tg_canvas_pixel(uint8_t *buffer, uint16_t x, uint16_t y, bool on);
bool tg_canvas_line(uint8_t *buffer, uint8_t row, const char *text);
