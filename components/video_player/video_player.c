/*
 * SPDX-FileCopyrightText: 2026 Espressif Systems (Shanghai) CO., LTD
 *
 * SPDX-License-Identifier: Apache-2.0
 */

// Based on https://github.com/espressif/esp-gmf/tree/81b45bfcfc10daa18d81869ac2fbeebd88ab0876/packages/esp_player/examples/video_player/main

#include <inttypes.h>
#include <string.h>

#include "esp_log.h"
#include "esp_codec_dev.h"
#include "esp_board_manager_includes.h"
#include "media_lib_adapter.h"
#include "esp_extractor_defaults.h"
#include "esp_audio_dec_default.h"
#include "esp_video_dec_default.h"
#include "esp_audio_render.h"
#include "esp_video_render.h"
#include "esp_vui_container.h"
#include "esp_vui_overlay.h"
#include "esp_vui_widget_default.h"
#include "esp_video_render_backend.h"
#include "esp_gmf_pool.h"
#include "esp_gmf_ch_cvt.h"
#include "esp_gmf_bit_cvt.h"
#include "esp_gmf_rate_cvt.h"
#include "esp_gmf_alc.h"
#include "esp_gmf_video_color_convert.h"
#include "esp_gmf_video_crop.h"
#include "esp_gmf_video_scale.h"
#if CONFIG_IDF_TARGET_ESP32P4 || CONFIG_IDF_TARGET_ESP32S31
#include "esp_gmf_video_ppa.h"
#endif  /* CONFIG_IDF_TARGET_ESP32P4 || CONFIG_IDF_TARGET_ESP32S31 */
#include "esp_player.h"
#include "video_player.h"

static const char *TAG = "VIDEO_PLAYER";

static esp_video_render_handle_t s_video_render = NULL;
static esp_gmf_pool_handle_t s_video_pool = NULL;

static esp_audio_render_handle_t s_audio_render = NULL;
static esp_gmf_pool_handle_t s_audio_pool = NULL;

static esp_vui_container_handle_t s_color_overlay_container = NULL;
static esp_vui_container_handle_t s_text_overlay_container = NULL;
static esp_vui_widget_t *s_text_widget = NULL;

static esp_player_handle_t s_player = NULL;

/**
 * @brief  Example playback configuration
 */
#define VIDEO_PLAYER_OUTPUT_VOLUME  80

/**
 * @brief  Shared audio/video render output configuration for the example.
 *
 * @note  LCD geometry comes from board manager. This struct only carries render
 *        parameters under application control.
 */
typedef struct {
    uint32_t  out_sample_rate;      /*!< Audio playback sample rate (Hz) */
    uint8_t   out_bits_per_sample;  /*!< Audio playback bit depth */
    uint8_t   out_channels;         /*!< Audio playback channel count */
    uint32_t  video_fps;            /*!< Video render frame rate */
} video_render_settings_t;

#define VIDEO_RENDER_SETTINGS_DEFAULT()  {  \
    .out_sample_rate     = 16000,           \
    .out_bits_per_sample = 16,              \
    .out_channels        = 1,               \
    .video_fps           = 30,              \
}

static void destroy_audio_render(void);
static void destroy_video_render(void);

#ifdef CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUPPORT
static void video_player_reconfig_lcd(void)
{
    dev_display_lcd_config_t lcd_cfg;
    dev_display_lcd_config_t *origin_cfg = NULL;

    esp_board_manager_get_device_config(ESP_BOARD_DEVICE_NAME_DISPLAY_LCD, (void **)&origin_cfg);
    if (origin_cfg == NULL) {
        ESP_LOGE(TAG, "Failed to get display config");
        return;
    }
    memcpy(&lcd_cfg, origin_cfg, sizeof(dev_display_lcd_config_t));
    if (strcmp(origin_cfg->sub_type, "rgb") == 0) {
#ifdef CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUB_RGB_SUPPORT
        lcd_cfg.sub_cfg.rgb.panel_config.num_fbs = 2;
#endif  /* CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUB_RGB_SUPPORT */
    } else if (strcmp(origin_cfg->sub_type, "dsi") == 0) {
#ifdef CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUB_DSI_SUPPORT
        lcd_cfg.sub_cfg.dsi.dpi_config.num_fbs = 2;
#endif  /* CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUB_DSI_SUPPORT */
    }
    if (lcd_cfg.swap_xy) {
        uint8_t w = lcd_cfg.lcd_width;
        lcd_cfg.lcd_width  = lcd_cfg.lcd_height;
        lcd_cfg.lcd_height = w;
    }
    esp_board_device_override_config(ESP_BOARD_DEVICE_NAME_DISPLAY_LCD, &lcd_cfg, sizeof(dev_display_lcd_config_t));
    ESP_LOGI(TAG, "LCD configuration overridden");
}

static esp_player_err_t fill_lcd_backend_cfg(esp_video_render_lcd_cfg_t *lcd_backend_cfg,
                                             const dev_display_lcd_config_t *lcd_cfg,
                                             const dev_display_lcd_handles_t *lcd_handle)
{
    if (lcd_backend_cfg == NULL || lcd_cfg == NULL || lcd_handle == NULL || lcd_handle->panel_handle == NULL) {
        return ESP_PLAYER_ERR_INVALID_ARG;
    }

    lcd_backend_cfg->width = lcd_cfg->lcd_width;
    lcd_backend_cfg->height = lcd_cfg->lcd_height;
    lcd_backend_cfg->fb_num = 1;
    lcd_backend_cfg->lcd_handle = lcd_handle->panel_handle;
    lcd_backend_cfg->io_handle = lcd_handle->io_handle;

    if (strcmp(lcd_cfg->sub_type, "spi") == 0) {
        lcd_backend_cfg->lcd_type = ESP_VIDEO_RENDER_LCD_TYPE_DVP;
        lcd_backend_cfg->out_format = ESP_VIDEO_RENDER_FORMAT_RGB565_BE;
    } else if (strcmp(lcd_cfg->sub_type, "rgb") == 0) {
        lcd_backend_cfg->lcd_type = ESP_VIDEO_RENDER_LCD_TYPE_RGB;
        lcd_backend_cfg->out_format = ESP_VIDEO_RENDER_FORMAT_RGB565;
#ifdef CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUB_RGB_SUPPORT
        lcd_backend_cfg->fb_num = lcd_cfg->sub_cfg.rgb.panel_config.num_fbs;
#endif  /* CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUB_RGB_SUPPORT */
    } else if (strcmp(lcd_cfg->sub_type, "i80") == 0) {
        lcd_backend_cfg->lcd_type = ESP_VIDEO_RENDER_LCD_TYPE_I80;
        lcd_backend_cfg->out_format = ESP_VIDEO_RENDER_FORMAT_RGB565;
    } else if (strcmp(lcd_cfg->sub_type, "dsi") == 0) {
        lcd_backend_cfg->lcd_type = ESP_VIDEO_RENDER_LCD_TYPE_DPI;
        lcd_backend_cfg->out_format = ESP_VIDEO_RENDER_FORMAT_RGB565;
#ifdef CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUB_DSI_SUPPORT
        lcd_backend_cfg->fb_num = lcd_cfg->sub_cfg.dsi.dpi_config.num_fbs;
#endif  /* CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUB_DSI_SUPPORT */
    } else if (strcmp(lcd_cfg->sub_type, "parlio") == 0) {
        lcd_backend_cfg->lcd_type = ESP_VIDEO_RENDER_LCD_TYPE_I80;
        lcd_backend_cfg->out_format = ESP_VIDEO_RENDER_FORMAT_RGB565;
    } else {
        ESP_LOGE(TAG, "Unsupported LCD sub type: %s", lcd_cfg->sub_type);
        return ESP_PLAYER_ERR_FAIL;
    }

    return ESP_PLAYER_ERR_OK;
}
#endif  /* CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUPPORT */

static void register_media_defaults(void)
{
    media_lib_add_default_adapter();
    esp_extractor_register_default();
    esp_audio_dec_register_default();
    esp_video_dec_register_default();
}

static void unregister_media_defaults(void)
{
    esp_video_dec_unregister_default();
    esp_audio_dec_unregister_default();
    esp_extractor_unregister_default();
}

static esp_codec_dev_handle_t get_playback_codec_dev(void)
{
    dev_audio_codec_handles_t *dac_handles = NULL;
    if (esp_board_manager_get_device_handle(ESP_BOARD_DEVICE_NAME_AUDIO_DAC, (void **)&dac_handles) != ESP_OK
        || dac_handles == NULL) {
        return NULL;
    }
    return dac_handles->codec_dev;
}

static void close_playback_codec(void)
{
    esp_codec_dev_handle_t codec_dev = get_playback_codec_dev();
    if (codec_dev == NULL) {
        return;
    }
    esp_codec_dev_close(codec_dev);
    esp_board_manager_deinit_device_by_name(ESP_BOARD_DEVICE_NAME_AUDIO_DAC);
}

static int render_writer_cb(uint8_t *pcm, uint32_t len, void *ctx)
{
    esp_codec_dev_handle_t dev = (esp_codec_dev_handle_t)ctx;
    if (dev == NULL || pcm == NULL || len == 0) {
        return -1;
    }
    return esp_codec_dev_write(dev, pcm, len);
}

static int register_audio_render_pool(esp_gmf_pool_handle_t *out_pool)
{
    *out_pool = NULL;
    if (esp_gmf_pool_init(out_pool) != ESP_GMF_ERR_OK) {
        return -1;
    }
    esp_gmf_element_handle_t el = NULL;

    esp_ae_ch_cvt_cfg_t ch_cfg = DEFAULT_ESP_GMF_CH_CVT_CONFIG();
    if (esp_gmf_ch_cvt_init(&ch_cfg, &el) == ESP_GMF_ERR_OK) {
        esp_gmf_pool_register_element(*out_pool, el, NULL);
    }

    esp_ae_bit_cvt_cfg_t bit_cfg = DEFAULT_ESP_GMF_BIT_CVT_CONFIG();
    if (esp_gmf_bit_cvt_init(&bit_cfg, &el) == ESP_GMF_ERR_OK) {
        esp_gmf_pool_register_element(*out_pool, el, NULL);
    }

    esp_ae_rate_cvt_cfg_t rate_cfg = DEFAULT_ESP_GMF_RATE_CVT_CONFIG();
    if (esp_gmf_rate_cvt_init(&rate_cfg, &el) == ESP_GMF_ERR_OK) {
        esp_gmf_pool_register_element(*out_pool, el, NULL);
    }

    esp_ae_alc_cfg_t alc_cfg = DEFAULT_ESP_GMF_ALC_CONFIG();
    if (esp_gmf_alc_init(&alc_cfg, &el) == ESP_GMF_ERR_OK) {
        esp_gmf_pool_register_element(*out_pool, el, NULL);
    }
    return 0;
}

static int register_video_render_pool(esp_gmf_pool_handle_t *out_pool)
{
    *out_pool = NULL;
    if (esp_gmf_pool_init(out_pool) != ESP_GMF_ERR_OK) {
        return -1;
    }

    esp_gmf_element_handle_t el = NULL;

#if CONFIG_IDF_TARGET_ESP32P4 || CONFIG_IDF_TARGET_ESP32S31
    if (esp_gmf_video_ppa_init(NULL, &el) == ESP_GMF_ERR_OK) {
        esp_gmf_pool_register_element(*out_pool, el, NULL);
    }
#else
    esp_imgfx_scale_cfg_t scale_cfg = {
        .filter_type = ESP_IMGFX_SCALE_FILTER_TYPE_BILINEAR,
    };
    if (esp_gmf_video_scale_init(&scale_cfg, &el) == ESP_GMF_ERR_OK) {
        esp_gmf_pool_register_element(*out_pool, el, NULL);
    }

    el = NULL;
    esp_imgfx_crop_cfg_t crop_cfg = {};
    if (esp_gmf_video_crop_init(&crop_cfg, &el) == ESP_GMF_ERR_OK) {
        esp_gmf_pool_register_element(*out_pool, el, NULL);
    }
#endif  /* CONFIG_IDF_TARGET_ESP32P4 || CONFIG_IDF_TARGET_ESP32S31 */

    el = NULL;
    esp_imgfx_color_convert_cfg_t color_cfg = {
        .color_space_std = ESP_IMGFX_COLOR_SPACE_STD_BT601,
    };
    if (esp_gmf_video_color_convert_init(&color_cfg, &el) == ESP_GMF_ERR_OK) {
        esp_gmf_pool_register_element(*out_pool, el, NULL);
    }

    return 0;
}

static esp_player_err_t open_playback_codec(uint32_t sample_rate,
                                            uint8_t bits_per_sample,
                                            uint8_t channels,
                                            esp_codec_dev_handle_t *codec_dev)
{
    if (codec_dev == NULL) {
        return ESP_PLAYER_ERR_INVALID_ARG;
    }
    *codec_dev = NULL;

    if (esp_board_manager_init_device_by_name(ESP_BOARD_DEVICE_NAME_AUDIO_DAC) != ESP_OK) {
        ESP_LOGE(TAG, "Failed to init audio DAC");
        return ESP_PLAYER_ERR_FAIL;
    }

    *codec_dev = get_playback_codec_dev();
    if (*codec_dev == NULL) {
        ESP_LOGE(TAG, "Failed to get playback codec handle");
        esp_board_manager_deinit_device_by_name(ESP_BOARD_DEVICE_NAME_AUDIO_DAC);
        return ESP_PLAYER_ERR_FAIL;
    }

    esp_err_t ret = esp_codec_dev_set_out_vol(*codec_dev, VIDEO_PLAYER_OUTPUT_VOLUME);
    if (ret != ESP_CODEC_DEV_OK) {
        ESP_LOGW(TAG, "Failed to set output volume, continue playback");
    }

    esp_codec_dev_sample_info_t fs = {
        .sample_rate = sample_rate,
        .channel = channels,
        .bits_per_sample = bits_per_sample,
    };
    ret = esp_codec_dev_open(*codec_dev, &fs);
    if (ret != ESP_CODEC_DEV_OK) {
        ESP_LOGE(TAG, "Failed to open playback codec");
        *codec_dev = NULL;
        esp_board_manager_deinit_device_by_name(ESP_BOARD_DEVICE_NAME_AUDIO_DAC);
        return ESP_PLAYER_ERR_FAIL;
    }
    return ESP_PLAYER_ERR_OK;
}

static esp_player_err_t create_audio_render(const video_render_settings_t *settings)
{
    esp_codec_dev_handle_t codec_dev = NULL;
    if (open_playback_codec(settings->out_sample_rate,
                            settings->out_bits_per_sample,
                            settings->out_channels,
                            &codec_dev) != ESP_PLAYER_ERR_OK) {
        return ESP_PLAYER_ERR_FAIL;
    }

    if (register_audio_render_pool(&s_audio_pool) != 0) {
        ESP_LOGE(TAG, "Failed to register audio processing pool");
        goto fail;
    }

    esp_audio_render_cfg_t cfg = {
        .max_stream_num = 1,
        .out_writer = render_writer_cb,
        .out_ctx = codec_dev,
        .out_sample_info = {
            .sample_rate = settings->out_sample_rate,
            .bits_per_sample = settings->out_bits_per_sample,
            .channel = settings->out_channels,
        },
        .pool = s_audio_pool,
        .process_period = 20,
    };
    if (esp_audio_render_create(&cfg, &s_audio_render) != ESP_AUDIO_RENDER_ERR_OK) {
        ESP_LOGE(TAG, "Failed to create esp_audio_render");
        goto fail;
    }

    ESP_LOGI(TAG, "Audio render ready: %" PRIu32 " Hz, %u-bit, %u-ch",
             settings->out_sample_rate,
             settings->out_bits_per_sample,
             settings->out_channels);
    return ESP_PLAYER_ERR_OK;

fail:
    destroy_audio_render();
    return ESP_PLAYER_ERR_FAIL;
}

static void destroy_audio_render(void)
{
    if (s_audio_render) {
        esp_audio_render_destroy(s_audio_render);
        s_audio_render = NULL;
    }
    if (s_audio_pool) {
        esp_gmf_pool_deinit(s_audio_pool);
        s_audio_pool = NULL;
    }
    close_playback_codec();
}

#define CHECK_RENDER(func, args)                                             \
    do {                                                                     \
        esp_video_render_err_t ret_ = func args;                             \
        if (ret_ != ESP_VIDEO_RENDER_ERR_OK) {                               \
            ESP_LOGE(TAG, #func " %d", ret_);                        \
            return ret_;                                                     \
        }                                                                    \
    } while (0)


static esp_video_render_err_t create_video_color_overlay(esp_video_render_handle_t render_handle, uint16_t width, uint16_t height) {
    // 1. Open a dedicated stream just to host the UI overlay
    esp_video_render_stream_info_t stream_info = {
        .info = {
            // We won't write actual video frames to this stream,
            // but the renderer requires a valid format/size to initialize it.
            .format = ESP_VIDEO_RENDER_FORMAT_RGB565,
            .width = width,
            .height = height,
            .fps = 0,
        },
        .cached = false, // Cached is efficient for static UI elements, required to set alpha
    };

    esp_video_render_stream_handle_t overlay_stream = NULL;
    CHECK_RENDER(esp_video_render_stream_open, (render_handle, &stream_info, &overlay_stream));

    // 2. Set Z-Order to 1 so this renders ABOVE the esp_player video stream (which is 0)
    CHECK_RENDER(esp_video_render_stream_set_zorder, (overlay_stream, 1));

    // Ensure the stream covers the whole screen
    esp_video_render_rect_t disp_rect = { .x = 0, .y = 0, .width = width, .height = height };
    CHECK_RENDER(esp_video_render_stream_set_disp_rect, (overlay_stream, &disp_rect));

    // 3. Get the overlay handle attached to this new stream
    esp_vui_overlay_handle_t overlay = NULL;
    CHECK_RENDER(esp_video_render_stream_get_overlay, (overlay_stream, &overlay));
    if (overlay == NULL) {
        ESP_LOGE(TAG, "Failed to get overlay handle");
        return ESP_VIDEO_RENDER_ERR_FAIL;
    }

    esp_video_render_frame_info_t container_info = {
        .format = ESP_VIDEO_RENDER_FORMAT_RGB565,
        .width = width,
        .height = height,
    };
    esp_video_render_pos_t container_pos = {.x = 0, .y = 0};
    CHECK_RENDER(esp_vui_container_create, (overlay, &container_info, &container_pos, true, &s_color_overlay_container));
    esp_video_render_clr_t container_bg = {.r = 255, .g = 0, .b = 0};
    CHECK_RENDER(esp_vui_container_set_bg_color, (s_color_overlay_container, &container_bg));
    CHECK_RENDER(esp_vui_container_set_alpha, (s_color_overlay_container, 180));
    CHECK_RENDER(esp_vui_container_set_visible, (s_color_overlay_container, false));

    return ESP_VIDEO_RENDER_ERR_OK;
}

static esp_video_render_err_t create_video_text_overlay(esp_video_render_handle_t render_handle, uint16_t width, uint16_t height, const uint8_t* font_data, int font_data_len) {
    // 1. Open a dedicated stream just to host the UI overlay
    esp_video_render_stream_info_t stream_info = {
        .info = {
            // We won't write actual video frames to this stream,
            // but the renderer requires a valid format/size to initialize it.
            .format = ESP_VIDEO_RENDER_FORMAT_RGB565,
            .width = width,
            .height = height,
            .fps = 0,
        },
    };

    esp_video_render_stream_handle_t overlay_stream = NULL;
    CHECK_RENDER(esp_video_render_stream_open, (render_handle, &stream_info, &overlay_stream));

    // 2. Set Z-Order to 2 so this renders ABOVE the esp_player video stream (which is 0) and the color overlay
    CHECK_RENDER(esp_video_render_stream_set_zorder, (overlay_stream, 2));

    // Ensure the stream covers the whole screen
    esp_video_render_rect_t disp_rect = { .x = 0, .y = 0, .width = width, .height = height };
    CHECK_RENDER(esp_video_render_stream_set_disp_rect, (overlay_stream, &disp_rect));

    // 3. Get the overlay handle attached to this new stream
    esp_vui_overlay_handle_t overlay = NULL;
    CHECK_RENDER(esp_video_render_stream_get_overlay, (overlay_stream, &overlay));
    if (overlay == NULL) {
        ESP_LOGE(TAG, "Failed to get overlay handle");
        return ESP_VIDEO_RENDER_ERR_FAIL;
    }

    esp_video_render_frame_info_t container_info = {
        .format = ESP_VIDEO_RENDER_FORMAT_RGB565,
        .width = width,
        .height = height,
    };
    esp_video_render_pos_t container_pos = {.x = 0, .y = 0};
    CHECK_RENDER(esp_vui_container_create, (overlay, &container_info, &container_pos, false, &s_text_overlay_container));
    s_text_widget = esp_vui_text_widget_init(s_text_overlay_container, &container_info, &container_pos, container_info.width, container_info.height);
    if (s_text_widget == NULL) {
        ESP_LOGE(TAG, "esp_vui_text_widget_init");
        return ESP_VIDEO_RENDER_ERR_FAIL;
    }
    CHECK_RENDER(esp_vui_text_widget_set_font_from_mem, (s_text_widget, "Bitstream Vera Sans Bold", font_data, font_data_len, 32));
    esp_video_render_clr_t bg = {.r = 0, .g = 0, .b = 255};      // Blue background
    esp_video_render_clr_t fg = {.r = 255, .g = 255, .b = 255};  // White text
    CHECK_RENDER(esp_vui_text_widget_set_bg_color, (s_text_widget, &bg, false));
    CHECK_RENDER(esp_vui_text_widget_set_text_color, (s_text_widget, &fg));
    CHECK_RENDER(esp_vui_text_widget_set_align, (s_text_widget, 1 /*center*/, 1 /*middle*/));
    CHECK_RENDER(esp_vui_text_widget_set_overflow, (s_text_widget, 1 /*wrap*/));

    CHECK_RENDER(esp_vui_container_set_visible, (s_text_overlay_container, false));

    return ESP_VIDEO_RENDER_ERR_OK;
}

static esp_player_err_t create_video_render(const video_render_settings_t *settings, const uint8_t* font_data, int font_data_len)
{
#ifdef CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUPPORT
    video_player_reconfig_lcd();
    if (esp_board_manager_init_device_by_name(ESP_BOARD_DEVICE_NAME_DISPLAY_LCD) != ESP_OK) {
        ESP_LOGE(TAG, "Failed to init display LCD");
        return ESP_PLAYER_ERR_FAIL;
    }

    dev_display_lcd_handles_t *lcd_handle = NULL;
    dev_display_lcd_config_t *lcd_cfg = NULL;
    if (esp_board_manager_get_device_handle(ESP_BOARD_DEVICE_NAME_DISPLAY_LCD, (void **)&lcd_handle) != ESP_OK
        || lcd_handle == NULL || lcd_handle->panel_handle == NULL) {
        ESP_LOGE(TAG, "Failed to get LCD panel handle");
        goto fail;
    }
    if (esp_board_manager_get_device_config(ESP_BOARD_DEVICE_NAME_DISPLAY_LCD, (void **)&lcd_cfg) != ESP_OK
        || lcd_cfg == NULL) {
        ESP_LOGE(TAG, "Failed to get LCD config");
        goto fail;
    }

    if (register_video_render_pool(&s_video_pool) != 0) {
        ESP_LOGE(TAG, "Failed to register video processing pool");
        goto fail;
    }

    esp_video_render_cfg_t render_cfg = {
        .pool = s_video_pool,
        .fps = settings->video_fps,
    };
    esp_video_render_handle_t render = NULL;
    if (esp_video_render_create(&render_cfg, &render) != ESP_VIDEO_RENDER_ERR_OK) {
        ESP_LOGE(TAG, "Failed to create esp_video_render");
        goto fail;
    }

    esp_video_render_lcd_cfg_t lcd_backend_cfg = {};
    if (fill_lcd_backend_cfg(&lcd_backend_cfg, lcd_cfg, lcd_handle) != ESP_PLAYER_ERR_OK) {
        ESP_LOGE(TAG, "Failed to fill LCD backend config");
        goto fail;
    }

    esp_video_render_backend_cfg_t backend_cfg = {
        .ops = esp_video_render_get_lcd_backend(),
        .cfg = &lcd_backend_cfg,
        .cfg_size = sizeof(lcd_backend_cfg),
    };
    if (esp_video_render_set_display(render, &backend_cfg) != ESP_VIDEO_RENDER_ERR_OK) {
        ESP_LOGE(TAG, "Failed to set LCD display backend");
        goto fail;
    }

    s_video_render = render;
    ESP_LOGI(TAG, "Video render ready: %ux%u, out_format=0x%" PRIx32 "",
             (unsigned)lcd_cfg->lcd_width, (unsigned)lcd_cfg->lcd_height,
             (uint32_t)lcd_backend_cfg.out_format);

    if (create_video_color_overlay(render, lcd_cfg->lcd_width, lcd_cfg->lcd_height) != ESP_VIDEO_RENDER_ERR_OK) {
        goto fail;
    }

    if (create_video_text_overlay(render, lcd_cfg->lcd_width, lcd_cfg->lcd_height, font_data, font_data_len) != ESP_VIDEO_RENDER_ERR_OK) {
        goto fail;
    }

    return ESP_PLAYER_ERR_OK;

fail:
    destroy_video_render();
    return ESP_PLAYER_ERR_FAIL;
#else
    ESP_LOGE(TAG, "Board has no display_lcd device (CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUPPORT)");
    return ESP_PLAYER_ERR_FAIL;
#endif  /* CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUPPORT */
}

static void destroy_video_render(void)
{
    s_color_overlay_container = NULL;
    s_text_overlay_container = NULL;
    s_text_widget = NULL;

    if (s_video_render) {
        esp_video_render_destroy((esp_video_render_handle_t)s_video_render);
        s_video_render = NULL;
    }
    if (s_video_pool) {
        esp_gmf_pool_deinit(s_video_pool);
        s_video_pool = NULL;
    }
#ifdef CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUPPORT
    esp_board_manager_deinit_device_by_name(ESP_BOARD_DEVICE_NAME_DISPLAY_LCD);
#endif  /* CONFIG_ESP_BOARD_DEV_DISPLAY_LCD_SUPPORT */
}

static esp_player_err_t video_player_setup(const video_render_settings_t *render_settings, const uint8_t* font_data, int font_data_len)
{
    if (render_settings == NULL || render_settings->video_fps == 0) {
        return ESP_PLAYER_ERR_INVALID_ARG;
    }
    if (s_audio_render != NULL || s_video_render != NULL) {
        ESP_LOGE(TAG, "video_player_setup already called");
        return ESP_PLAYER_ERR_FAIL;
    }

    register_media_defaults();

    if (create_audio_render(render_settings) != ESP_PLAYER_ERR_OK) {
        ESP_LOGE(TAG, "create_audio_render failed");
        unregister_media_defaults();
        return ESP_PLAYER_ERR_FAIL;
    }

    if (create_video_render(render_settings, font_data, font_data_len) != ESP_PLAYER_ERR_OK) {
        ESP_LOGE(TAG, "create_video_render failed");
        destroy_audio_render();
        unregister_media_defaults();
        return ESP_PLAYER_ERR_FAIL;
    }

    return ESP_PLAYER_ERR_OK;
}

static esp_player_err_t create_video_player()
{
    if (s_player != NULL) {
        ESP_LOGE(TAG, "create_video_player already called");
        return ESP_PLAYER_ERR_FAIL;
    }

    if (s_audio_render == NULL || s_video_render == NULL) {
        ESP_LOGE(TAG, "Call video_player_setup() first");
        return ESP_PLAYER_ERR_FAIL;
    }

    esp_audio_render_err_t aret = ESP_AUDIO_RENDER_ERR_OK;
    esp_audio_render_stream_handle_t audio_stream = NULL;
    if ((aret = esp_audio_render_stream_get(s_audio_render, ESP_AUDIO_RENDER_FIRST_STREAM,
                                    &audio_stream)) != ESP_AUDIO_RENDER_ERR_OK) {
        ESP_LOGE(TAG, "esp_audio_render_stream_get %d", aret);
        return ESP_PLAYER_ERR_FAIL;
    }

    esp_player_config_t player_cfg = ESP_PLAYER_CONFIG_DEFAULT();
    player_cfg.audio_render_hd = audio_stream;
    player_cfg.video_render_hd = s_video_render;

    esp_player_err_t ret = ESP_PLAYER_ERR_OK;
    if ((ret = esp_player_init(&player_cfg, &s_player)) != ESP_PLAYER_ERR_OK) {
        ESP_LOGE(TAG, "esp_player_init %d", ret);
        return ret;
    }

    return ESP_PLAYER_ERR_OK;
}

static void destroy_video_player()
{
    if (s_player == NULL) {
        return;
    }

    esp_player_set_event_cb(s_player, NULL, NULL);
    esp_player_deinit(s_player);
    s_player = NULL;
}

static void video_player_teardown(void)
{
    if (s_audio_render != NULL || s_video_render != NULL) {
        destroy_audio_render();
        destroy_video_render();
        unregister_media_defaults();
    }
}


esp_player_err_t initialize_video_system(const uint8_t* font_data, int font_data_len)
{
    esp_player_err_t ret = ESP_PLAYER_ERR_OK;
    video_render_settings_t render_settings = VIDEO_RENDER_SETTINGS_DEFAULT();
    if ((ret = video_player_setup(&render_settings, font_data, font_data_len)) != ESP_PLAYER_ERR_OK) {
        ESP_LOGE(TAG, "video_player_setup %d", ret);
        goto out;
    }
    if ((ret = create_video_player()) != ESP_PLAYER_ERR_OK) {
        ESP_LOGE(TAG, "video_player_new %d", ret);
        goto out;
    }
    return ret;

    out:
        destroy_video_player();
        video_player_teardown();
        return ret;
}

esp_player_err_t play_url(const char* url) {
    if (s_player == NULL) {
        ESP_LOGE(TAG, "Initialize video system first");
        return ESP_PLAYER_ERR_FAIL;
    }
    esp_player_err_t ret = ESP_PLAYER_ERR_OK;
    // Work around https://github.com/espressif/esp-gmf/issues/60
    if ((ret = esp_player_stop(s_player)) != ESP_PLAYER_ERR_OK) {
        ESP_LOGE(TAG, "Stop failed: %d", ret);
        return ret;
    }
    esp_player_data_src_t src = ESP_PLAYER_DATA_SRC(url, ESP_PLAYER_MASK_AV);
    if ((ret = esp_player_set_data_src(s_player, &src)) != ESP_PLAYER_ERR_OK) {
        ESP_LOGE(TAG, "Set URL failed: %d", ret);
        return ret;
    }
    if ((ret = esp_player_run_to_end(s_player)) != ESP_PLAYER_ERR_OK) {
        ESP_LOGE(TAG, "Playback failed: %d", ret);
        return ret;
    }
    return ret;
}

esp_err_t hilite_video(bool hilite) {
    if (s_color_overlay_container == NULL) return ESP_FAIL;

    if (esp_vui_container_set_visible(s_color_overlay_container, hilite) != ESP_VIDEO_RENDER_ERR_OK) {
        return ESP_FAIL;
    }
    return ESP_OK;
}

esp_err_t display_text(const char* text) {
    if (s_text_widget == NULL) return ESP_FAIL;

    esp_video_render_err_t ret = ESP_VIDEO_RENDER_ERR_OK;

    if ((ret = esp_vui_text_widget_set_text(s_text_widget, text)) != ESP_VIDEO_RENDER_ERR_OK) {
        ESP_LOGE(TAG, "esp_vui_text_widget_set_text %d", ret);
        return ESP_FAIL;
    }
    if ((ret = esp_vui_container_set_visible(s_text_overlay_container, text != NULL)) != ESP_VIDEO_RENDER_ERR_OK) {
        ESP_LOGE(TAG, "esp_vui_container_set_visible %d", ret);
        return ESP_FAIL;
    }
    return ESP_OK;
}

void deinitialize_video_system(void)
{
    destroy_video_player();
    video_player_teardown();
}
