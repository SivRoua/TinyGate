idf_component_register(SRCS "src/tg_button.c" "src/button_state.c"
    INCLUDE_DIRS "include" PRIV_INCLUDE_DIRS "src"
    REQUIRES esp_driver_gpio esp_timer freertos)
