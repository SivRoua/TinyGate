#include "tg_sdcard.h"

#include "driver/sdspi_host.h"
#include "driver/spi_master.h"
#include "esp_log.h"
#include "esp_vfs_fat.h"
#include "diskio_impl.h"
#include "diskio_sdmmc.h"
#include "sdmmc_cmd.h"

#define TF_HOST SPI2_HOST
#define TF_CS GPIO_NUM_0
#define TF_SCK GPIO_NUM_1
#define TF_MOSI GPIO_NUM_2
#define TF_MISO GPIO_NUM_3
#define TF_MAX_CLOCK_KHZ 4000

static const char *TAG = "tg_sdcard";
static bool s_bus_initialized, s_device_attached, s_vfs_registered, s_mounted;
static sdspi_dev_handle_t s_device;
static sdmmc_card_t s_card;
static BYTE s_drive;
static char s_drive_path[3];

static esp_err_t close_spi_card(void)
{
    if (s_device_attached) {
        esp_err_t err = sdspi_host_remove_device(s_device);
        if (err != ESP_OK) return err;
        s_device_attached = false;
    }
    if (s_bus_initialized) {
        esp_err_t err = spi_bus_free(TF_HOST);
        if (err != ESP_OK) return err;
        s_bus_initialized = false;
    }
    return ESP_OK;
}

static esp_err_t report_failure(const char *step, esp_err_t err, esp_err_t cleanup)
{
    ESP_LOGE(TAG, "%s: %s", step, esp_err_to_name(err));
    if (cleanup != ESP_OK) {
        ESP_LOGE(TAG, "cleanup failed: %s", esp_err_to_name(cleanup));
    }
    return err;
}

static esp_err_t open_spi_card(void)
{
    if (s_bus_initialized) return ESP_ERR_INVALID_STATE;

    const spi_bus_config_t bus = {
        .mosi_io_num = TF_MOSI, .miso_io_num = TF_MISO, .sclk_io_num = TF_SCK,
        .quadwp_io_num = -1, .quadhd_io_num = -1,
        .max_transfer_sz = 4096,
    };
    const char *step = "initialize SPI2";
    esp_err_t err = spi_bus_initialize(TF_HOST, &bus, SPI_DMA_CH_AUTO);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "%s: %s", step, esp_err_to_name(err));
        return err;
    }
    s_bus_initialized = true;

    step = "initialize SDSPI host";
    err = sdspi_host_init();
    if (err != ESP_OK) goto fail;

    sdspi_device_config_t device = SDSPI_DEVICE_CONFIG_DEFAULT();
    device.host_id = TF_HOST;
    device.gpio_cs = TF_CS;
    step = "attach SDSPI device";
    err = sdspi_host_init_device(&device, &s_device);
    if (err != ESP_OK) goto fail;
    s_device_attached = true;

    sdmmc_host_t host = SDSPI_HOST_DEFAULT();
    host.slot = s_device;
    host.max_freq_khz = TF_MAX_CLOCK_KHZ;
    step = "identify card over SPI";
    err = sdmmc_card_init(&host, &s_card);
    if (err != ESP_OK) goto fail;
    return ESP_OK;

fail:
    return report_failure(step, err, close_spi_card());
}

esp_err_t tg_sdcard_init(void)
{
    esp_err_t err = open_spi_card();
    if (err != ESP_OK) return err;

    const char *step = "allocate FAT drive";
    err = ff_diskio_get_drive(&s_drive);
    if (err != ESP_OK) goto fail;
    s_drive_path[0] = (char)('0' + s_drive);
    s_drive_path[1] = ':';

    FATFS *fs;
    const esp_vfs_fat_conf_t vfs = {
        .base_path = TG_SDCARD_MOUNT_PATH, .fat_drive = s_drive_path, .max_files = 4,
    };
    step = "register FAT VFS";
    err = esp_vfs_fat_register(&vfs, &fs);
    if (err != ESP_OK) goto fail;
    s_vfs_registered = true;
    ff_diskio_register_sdmmc(s_drive, &s_card);

    FRESULT result = f_mount(fs, s_drive_path, 1);
    if (result != FR_OK) {
        ESP_LOGE(TAG, "FAT mount failed (FatFs code %d)", result);
        step = "mount FAT volume";
        err = result == FR_NO_FILESYSTEM ? ESP_ERR_NOT_SUPPORTED : ESP_FAIL;
        goto fail;
    }
    s_mounted = true;
    return ESP_OK;

fail:
    return report_failure(step, err, tg_sdcard_deinit());
}

bool tg_sdcard_is_mounted(void)
{
    return s_mounted;
}

static void fill_card_info(tg_sdcard_info_t *info)
{
    *info = (tg_sdcard_info_t) {
        .capacity_bytes = (uint64_t)s_card.csd.capacity * s_card.csd.sector_size,
        .sector_size_bytes = s_card.csd.sector_size,
        .clock_khz = s_card.real_freq_khz,
    };
}

esp_err_t tg_sdcard_get_info(tg_sdcard_info_t *info)
{
    if (!info) return ESP_ERR_INVALID_ARG;
    if (!s_mounted) return ESP_ERR_INVALID_STATE;
    fill_card_info(info);
    return ESP_OK;
}

esp_err_t tg_sdcard_deinit(void)
{
    if (s_vfs_registered) {
        if (f_mount(NULL, s_drive_path, 0) != FR_OK) return ESP_FAIL;
        s_mounted = false;
        esp_err_t err = esp_vfs_fat_unregister_path(TG_SDCARD_MOUNT_PATH);
        if (err != ESP_OK) return err;
        ff_diskio_unregister(s_drive);
        s_vfs_registered = false;
    }
    return close_spi_card();
}
