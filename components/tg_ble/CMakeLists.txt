idf_component_register(SRCS "src/tg_ble.c" "src/tg_ble_gatt.c" INCLUDE_DIRS "include"
    REQUIRES bt nvs_flash freertos)
