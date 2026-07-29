/*
 * SPDX-FileCopyrightText: 2026 Espressif Systems (Shanghai) CO., LTD
 * SPDX-License-Identifier: LicenseRef-Espressif-Modified-MIT
 *
 * See LICENSE file for details.
 */

/*
 * NOTE: This file is auto-generated and is only a template implementation of setup_device.
 * Manual review and debugging may be required before it can compile and run on real hardware.
 */

#include "esp_err.h"
#include "esp_log.h"
#include "esp_board_manager_includes.h"

#if __has_include(<esp_lcd_ili9341.h>)
#define HAS_ILI9341  1
#include "esp_lcd_ili9341.h"
#endif  /* __has_include(<esp_lcd_ili9341.h>) */

#if __has_include(<esp_lcd_touch_ft5x06.h>)
#define HAS_FT5X06  1
#include "esp_lcd_touch_ft5x06.h"
#endif  /* __has_include(<esp_lcd_touch_ft5x06.h>) */

static const char *TAG = "E32C28P_SETUP_DEVICE";

#if defined(HAS_ILI9341)
__attribute__((weak)) esp_err_t lcd_panel_factory_entry_t(esp_lcd_panel_io_handle_t io,
                                                          const esp_lcd_panel_dev_config_t *panel_dev_config,
                                                          esp_lcd_panel_handle_t *ret_panel)
{
    esp_err_t ret = esp_lcd_new_panel_ili9341(io, panel_dev_config, ret_panel);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Failed to create ili9341 panel: %s", esp_err_to_name(ret));
        return ret;
    }

    return ESP_OK;
}
#endif  /* defined(HAS_ILI9341) */

#if defined(HAS_FT5X06)
__attribute__((weak)) esp_err_t lcd_touch_factory_entry_t(esp_lcd_panel_io_handle_t io,
                                                          const esp_lcd_touch_config_t *touch_dev_config,
                                                          esp_lcd_touch_handle_t *ret_touch)
{
    esp_err_t ret = esp_lcd_touch_new_i2c_ft5x06(io, touch_dev_config, ret_touch);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Failed to create ft5x06 touch driver: %s", esp_err_to_name(ret));
        return ret;
    }

    return ESP_OK;
}
#endif  /* defined(HAS_FT5X06) */
