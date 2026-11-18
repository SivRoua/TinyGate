idf_component_register(SRCS "src/tg_sdcard.c" INCLUDE_DIRS "include"
    REQUIRES esp_common
    PRIV_REQUIRES esp_driver_spi esp_driver_sdspi sdmmc fatfs log)
