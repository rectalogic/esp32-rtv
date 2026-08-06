use esp32_rtv::{littlefs::Littlefs, sdcard::SdCard, video_player::VideoPlayer};

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
    let littlefs = Littlefs::new();
    let sdcard = SdCard::new();
    if littlefs.is_ok() && matches!(std::fs::exists("/littlefs/interstitial.mp4"), Ok(true)) {
        interstitial = Some("/littlefs/interstitial.mp4");
    }

    if sdcard.is_ok() {
        video_player.play("/sdcard/rtv/itysl.mp4")?;
        if let Some(interstitial) = interstitial {
            video_player.play(interstitial)?;
        }
        video_player.play("/sdcard/rtv/brian.mp4")?;
    }

    Ok(())
}
