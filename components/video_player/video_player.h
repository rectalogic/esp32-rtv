#pragma once

#include "esp_player_types.h"

esp_player_err_t initialize_video_system(void);
esp_player_err_t play_url(const char* url);
void deinitialize_video_system(void);
esp_err_t hilite_video(bool hilite);
