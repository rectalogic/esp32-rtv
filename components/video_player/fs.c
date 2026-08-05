#include "esp_log.h"
#include "esp_err.h"

#include "esp_board_manager_includes.h"
#include "fs.h"

static const char *TAG = "FS";

esp_err_t mount_sdcard(void)
{
    esp_err_t ret = esp_board_manager_init_device_by_name(ESP_BOARD_DEVICE_NAME_FS_SDCARD);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Failed to init SD card: %s", esp_err_to_name(ret));
    }
    return ret;
}

void unmount_sdcard(void)
{
    esp_board_manager_deinit_device_by_name(ESP_BOARD_DEVICE_NAME_FS_SDCARD);
}

esp_err_t mount_littlefs(void)
{
    esp_err_t ret = esp_board_manager_init_device_by_name(ESP_BOARD_DEVICE_NAME_LITTLEFS);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Failed to init LittleFS: %s", esp_err_to_name(ret));
    }
    return ret;
}

void unmount_littlefs(void)
{
    esp_board_manager_deinit_device_by_name(ESP_BOARD_DEVICE_NAME_LITTLEFS);
}
