#include "tg_rgb.h"
#include "rgb_color.h"
#include "driver/rmt_tx.h"
#include "driver/gpio.h"
static rmt_channel_handle_t s_channel;
static rmt_encoder_handle_t s_encoder;
static bool s_enabled;
static bool s_fault;
static rmt_symbol_word_t s_symbols[25];
esp_err_t tg_rgb_set(uint8_t r,uint8_t g,uint8_t b,uint8_t brightness)
{
    if (!s_channel || !s_encoder || s_fault) return ESP_ERR_INVALID_STATE;
    if (!s_enabled) {
        esp_err_t err=rmt_enable(s_channel);if (err) return err;s_enabled=true;
    }
    uint8_t grb[3];tg_rgb_pack(r,g,b,brightness,grb);
    for (unsigned i=0;i<24;++i) {
        bool bit=(grb[i/8]&(0x80>>(i%8)))!=0;
        s_symbols[i]=(rmt_symbol_word_t){.level0=1,.duration0=bit?7:3,.level1=0,.duration1=bit?6:10};
    }
    s_symbols[24]=(rmt_symbol_word_t){.level0=0,.duration0=1500,.level1=0,.duration1=1500};
    rmt_transmit_config_t tx={.loop_count=0};
    esp_err_t err=rmt_transmit(s_channel,s_encoder,s_symbols,sizeof(s_symbols),&tx);
    if (!err) err=rmt_tx_wait_all_done(s_channel,100);
    if (err) {
        esp_err_t stop=rmt_disable(s_channel);
        if (!stop) s_enabled=false;
        /* Do not alter s_symbols again if disable failed. */
        if (stop) {s_fault=true;return stop;}
    }
    return err;
}
esp_err_t tg_rgb_off(void) {return tg_rgb_set(0,0,0,0);}
esp_err_t tg_rgb_init(void)
{
    if (s_channel) return ESP_ERR_INVALID_STATE;
    rmt_tx_channel_config_t config={.gpio_num=8,.clk_src=RMT_CLK_SRC_DEFAULT,
        .resolution_hz=10000000,.mem_block_symbols=64,.trans_queue_depth=1};
    esp_err_t err=rmt_new_tx_channel(&config,&s_channel);if (err) return err;
    rmt_copy_encoder_config_t encoder={0};
    err=rmt_new_copy_encoder(&encoder,&s_encoder);if (err) goto fail;
    err=rmt_enable(s_channel);if (err) goto fail;s_enabled=true;
    err=tg_rgb_off();if (!err) return ESP_OK;
fail:
    tg_rgb_deinit();return err;
}
esp_err_t tg_rgb_deinit(void)
{
    if (!s_channel) return ESP_ERR_INVALID_STATE;
    if (s_enabled) {
        (void)tg_rgb_off();
        esp_err_t err=rmt_disable(s_channel);if (err) return err;s_enabled=false;
    }
    if (s_encoder) {esp_err_t err=rmt_del_encoder(s_encoder);if (err) return err;s_encoder=NULL;}
    esp_err_t err=rmt_del_channel(s_channel);if (err) return err;s_channel=NULL;
    s_fault=false;(void)gpio_reset_pin(8);return ESP_OK;
}
