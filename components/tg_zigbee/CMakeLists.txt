idf_component_register(SRCS "src/tg_zigbee.c" INCLUDE_DIRS "include"
    REQUIRES esp-zigbee-lib nvs_flash freertos esp_timer)
