idf_component_register(SRCS "src/tg_display.c" "src/display_canvas.c"
    INCLUDE_DIRS "include" PRIV_INCLUDE_DIRS "src"
    REQUIRES esp_driver_i2c esp_driver_gpio)
