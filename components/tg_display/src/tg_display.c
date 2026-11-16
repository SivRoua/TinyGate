#include "tg_display.h"
#include "display_canvas.h"
#include <string.h>
#include "sdkconfig.h"
#include "driver/gpio.h"
#include "driver/i2c_master.h"
#include "esp_log.h"

static const char *TAG = "tg_display";

static i2c_master_bus_handle_t s_bus;
static i2c_master_dev_handle_t s_device;
static bool s_ready;
static uint8_t s_address;
static uint8_t s_frame[TG_CANVAS_SIZE];

static esp_err_t commands(const uint8_t *bytes, size_t count)
{
    if (!s_device) return ESP_ERR_INVALID_STATE;
    if (!bytes || count > 32) return ESP_ERR_INVALID_ARG;
    uint8_t packet[33] = {0x00}; /* Co=0, D/C#=0: command stream */
    memcpy(packet + 1, bytes, count);
    return i2c_master_transmit(s_device, packet, count + 1, 100);
}

esp_err_t tg_display_flush(void)
{
    if (!s_ready) return ESP_ERR_INVALID_STATE;
    for (unsigned page = 0; page < 8; ++page) {
        const uint8_t position[] = {(uint8_t)(0xb0 + page), 0x00, 0x10};
        esp_err_t err = commands(position, sizeof(position));
        if (err != ESP_OK) return err;
        uint8_t packet[129] = {0x40}; /* D/C#=1: page data */
        memcpy(packet + 1, s_frame + page * 128, 128);
        err = i2c_master_transmit(s_device, packet, sizeof(packet), 100);
        if (err != ESP_OK) return err;
    }
    return ESP_OK;
}

esp_err_t tg_display_init(void)
{
    const tg_display_config_t config = {
        .sda_gpio = CONFIG_TG_DISPLAY_SDA_GPIO,
        .scl_gpio = CONFIG_TG_DISPLAY_SCL_GPIO,
        .clock_hz = 100000, .address = CONFIG_TG_DISPLAY_ADDRESS,
    };
    return tg_display_init_with_config(&config);
}

esp_err_t tg_display_init_with_config(const tg_display_config_t *config)
{
    if (!config || !GPIO_IS_VALID_OUTPUT_GPIO(config->sda_gpio) ||
        !GPIO_IS_VALID_OUTPUT_GPIO(config->scl_gpio) || config->sda_gpio == config->scl_gpio ||
        config->clock_hz < 10000 || config->clock_hz > 400000 ||
        (config->address && config->address != 0x3c && config->address != 0x3d))
        return ESP_ERR_INVALID_ARG;
    if (s_bus || s_device) return ESP_ERR_INVALID_STATE;
    ESP_LOGI(TAG, "SSD1306 init: SDA=%ld SCL=%ld clock=%lu address=%s",
             (long)config->sda_gpio, (long)config->scl_gpio,
             (unsigned long)config->clock_hz, config->address ? "fixed" : "auto");
    const i2c_master_bus_config_t bus_config = {
        .i2c_port = -1, .sda_io_num = config->sda_gpio, .scl_io_num = config->scl_gpio,
        .clk_source = I2C_CLK_SRC_DEFAULT, .glitch_ignore_cnt = 7,
        .flags.enable_internal_pullup = true,
    };
    esp_err_t err = i2c_new_master_bus(&bus_config, &s_bus);
    if (err != ESP_OK) return err;
    uint8_t address = config->address ? config->address : 0x3c;
    ESP_LOGI(TAG, "Probing 7-bit I2C address 0x%02x", address);
    err = i2c_master_probe(s_bus, address, 100);
    if (err == ESP_ERR_NOT_FOUND && !config->address) {
        address = 0x3d;
        ESP_LOGI(TAG, "Probing 7-bit I2C address 0x%02x", address);
        err = i2c_master_probe(s_bus, address, 100);
    }
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "OLED probe failed: %s (check VCC/GND/SDA/SCL)", esp_err_to_name(err));
        goto fail;
    }
    ESP_LOGI(TAG, "OLED acknowledged at 0x%02x", address);
    const i2c_device_config_t device_config = {
        .dev_addr_length = I2C_ADDR_BIT_LEN_7, .device_address = address,
        .scl_speed_hz = config->clock_hz,
    };
    err = i2c_master_bus_add_device(s_bus, &device_config, &s_device);
    if (err != ESP_OK) goto fail;
    /* SSD1306 charge-pump 128x64, page mode, normal display, mirrored orientation.
     * Four-pin modules have no separate reset input. */
    const uint8_t init[] = {
        0xae, 0xd5,0x80, 0xa8,0x3f, 0xd3,0x00, 0x40,
        0x8d,0x14, 0x20,0x02, 0xa1,0xc8, 0xda,0x12,
        0x81,0x7f, 0xd9,0xf1, 0xdb,0x20, 0xa4,0xa6, 0x2e
    };
    err = commands(init, sizeof(init));
    if (err != ESP_OK) goto fail;
    s_ready = true;
    s_address = address;
    memset(s_frame, 0, sizeof(s_frame));
    err = tg_display_flush();
    if (err == ESP_OK) err = tg_display_set_power(true);
    if (err == ESP_OK) {
        ESP_LOGI(TAG, "SSD1306 initialized and display enabled");
        return ESP_OK;
    }
fail:
    ESP_LOGE(TAG, "SSD1306 init failed: %s", esp_err_to_name(err));
    tg_display_deinit(); /* Keep handles if cleanup fails, allowing retry. */
    return err;
}

esp_err_t tg_display_clear(void)
{
    if (!s_ready) return ESP_ERR_INVALID_STATE;
    memset(s_frame, 0, sizeof(s_frame));
    return tg_display_flush();
}
esp_err_t tg_display_write_line(uint8_t row, const char *text)
{
    if (!text || row >= TG_DISPLAY_ROWS) return ESP_ERR_INVALID_ARG;
    if (!s_ready) return ESP_ERR_INVALID_STATE;
    tg_canvas_line(s_frame, row, text);
    return ESP_OK;
}
esp_err_t tg_display_set_pixel(uint16_t x, uint16_t y, bool on)
{
    if (x >= TG_DISPLAY_WIDTH || y >= TG_DISPLAY_HEIGHT) return ESP_ERR_INVALID_ARG;
    if (!s_ready) return ESP_ERR_INVALID_STATE;
    tg_canvas_pixel(s_frame, x, y, on);
    return ESP_OK;
}
esp_err_t tg_display_show_status(const char *title, const char *line1, const char *line2)
{
    if (!title || !line1 || !line2) return ESP_ERR_INVALID_ARG;
    if (!s_ready) return ESP_ERR_INVALID_STATE;
    memset(s_frame, 0, sizeof(s_frame));
    tg_canvas_line(s_frame, 0, title);
    tg_canvas_line(s_frame, 2, line1);
    tg_canvas_line(s_frame, 4, line2);
    return tg_display_flush();
}
esp_err_t tg_display_get_address(uint8_t *address)
{
    if (!address) return ESP_ERR_INVALID_ARG;
    if (!s_ready) return ESP_ERR_INVALID_STATE;
    *address = s_address;
    return ESP_OK;
}
esp_err_t tg_display_set_power(bool on)
{
    if (!s_ready) return ESP_ERR_INVALID_STATE;
    const uint8_t cmd = on ? 0xaf : 0xae;
    return commands(&cmd, 1);
}
esp_err_t tg_display_set_contrast(uint8_t contrast)
{
    if (!s_ready) return ESP_ERR_INVALID_STATE;
    const uint8_t cmd[] = {0x81, contrast};
    return commands(cmd, sizeof(cmd));
}
esp_err_t tg_display_deinit(void)
{
    if (!s_bus && !s_device) return ESP_ERR_INVALID_STATE;
    /* Best effort blanking: a disconnected display must not prevent bus release. */
    if (s_ready) (void)tg_display_set_power(false);
    s_ready = false;
    if (s_device) {
        esp_err_t err = i2c_master_bus_rm_device(s_device);
        if (err != ESP_OK) return err;
        s_device = NULL;
    }
    if (s_bus) {
        esp_err_t err = i2c_del_master_bus(s_bus);
        if (err != ESP_OK) return err;
        s_bus = NULL;
    }
    s_address = 0;
    return ESP_OK;
}
