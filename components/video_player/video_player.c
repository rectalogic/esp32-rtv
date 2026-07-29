#include "esp_log.h"
#include "esp_err.h"

#include "esp_board_manager_includes.h"

static const char *TAG = "VIDEO_PLAYER";

esp_err_t mount_sdcard(void)
{
    esp_err_t ret = esp_board_manager_init_device_by_name(ESP_BOARD_DEVICE_NAME_FS_SDCARD);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Failed to init SD card");
    }
    return ret;
}
