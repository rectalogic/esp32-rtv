use esp32_rtv::{
    littlefs::Littlefs, sdcard::SdCard, video_player::VideoPlayer, videos::find_videos,
};

const INTERSTITIAL: &str = "/littlefs/interstitial.mp4";

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    let _logger = esp_idf_svc::log::init_from_esp_idf();
    // logger
    //     .filter()
    //     .set_target_level("ESP_PLAYER", log::LevelFilter::Debug)?;

    let video_player = VideoPlayer::new()?;

    let mut interstitial = None;
    let mut videos = None;

    let sdcard = SdCard::new();
    let littlefs = Littlefs::new();

    if littlefs.is_ok() {
        if matches!(std::fs::exists(INTERSTITIAL), Ok(true)) {
            interstitial = Some(INTERSTITIAL);
        }
        if sdcard.is_err() {
            let mut all_videos = find_videos("/littlefs")?;
            if interstitial.is_some() {
                all_videos.retain(|f| f != INTERSTITIAL);
            }
            videos = Some(all_videos);
        }
    }

    if sdcard.is_ok() {
        videos = Some(find_videos("/sdcard/rtv")?);
    }

    if let Some(mut videos) = videos {
        videos.sort();
        for video in videos.into_iter().cycle() {
            video_player.play(&video)?;
            if let Some(interstitial) = interstitial {
                video_player.play(interstitial)?;
            }
        }
    }

    Ok(())
}
