#pragma once
#include "host/ble_gap.h"
void tg_gatt_reset(void);
void tg_gatt_gap(struct ble_gap_event *event);
