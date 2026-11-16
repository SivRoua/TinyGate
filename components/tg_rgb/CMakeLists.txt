idf_component_register(SRCS "src/tg_rgb.c" "src/rgb_color.c" INCLUDE_DIRS "include"
    PRIV_INCLUDE_DIRS "src" REQUIRES esp_driver_rmt esp_driver_gpio)
