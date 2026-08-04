#pragma once

#include "esp_err.h"

esp_err_t mount_sdcard(void);
void unmount_sdcard(void);
esp_err_t mount_spiffs(void);
void unmount_spiffs(void);
