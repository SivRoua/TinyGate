#include "tg_wifi.h"
#include <string.h>
#include "nvs.h"
#include "nvs_flash.h"
static bool valid(const char *ssid,const char *password)
{
    if(!ssid || !password) return false;
    size_t s=strnlen(ssid,33),p=strnlen(password,64);
    return s>0 && s<=32 && p<=63 && (p==0 || p>=8);
}
esp_err_t tg_wifi_save_credentials(const char *ssid,const char *password)
{
    if(!valid(ssid,password)) return ESP_ERR_INVALID_ARG;
    esp_err_t err=nvs_flash_init();if(err) return err;
    tg_wifi_credentials_t config={.version=1};
    memcpy(config.ssid,ssid,strlen(ssid));memcpy(config.password,password,strlen(password));
    nvs_handle_t handle;err=nvs_open("tg_wifi",NVS_READWRITE,&handle);if(err) return err;
    err=nvs_set_blob(handle,"credentials",&config,sizeof(config));
    if(!err) err=nvs_commit(handle);nvs_close(handle);return err;
}
esp_err_t tg_wifi_load_credentials(tg_wifi_credentials_t *config)
{
    if(!config) return ESP_ERR_INVALID_ARG;
    memset(config,0,sizeof(*config));
    esp_err_t err=nvs_flash_init();if(err) return err;
    nvs_handle_t handle;err=nvs_open("tg_wifi",NVS_READONLY,&handle);if(err) return err;
    size_t length=sizeof(*config);err=nvs_get_blob(handle,"credentials",config,&length);nvs_close(handle);
    if(!err && (length!=sizeof(*config) || config->version!=1 || config->ssid[32]!=0 ||
        config->password[63]!=0 || !valid(config->ssid,config->password))) err=ESP_ERR_INVALID_RESPONSE;
    if(err) memset(config,0,sizeof(*config));return err;
}
esp_err_t tg_wifi_forget_credentials(void)
{
    esp_err_t err=nvs_flash_init();if(err) return err;
    nvs_handle_t handle;err=nvs_open("tg_wifi",NVS_READWRITE,&handle);if(err) return err;
    err=nvs_erase_key(handle,"credentials");if(err==ESP_ERR_NVS_NOT_FOUND) err=ESP_OK;
    if(!err) err=nvs_commit(handle);nvs_close(handle);return err;
}
