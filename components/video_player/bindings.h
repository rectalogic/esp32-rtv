#include "esp_err.h"

esp_err_t mount_sdcard(void);
esp_err_t unmount_sdcard(void);

esp_err_t mount_spiffs(void);
esp_err_t unmount_spiffs(void);

void run_player(void);
