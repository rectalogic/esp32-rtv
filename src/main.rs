use esp32_rtv::{littlefs::Littlefs, sdcard::SdCard, video_player::VideoPlayer};

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let video_player = VideoPlayer::new()?;

    if let Ok(_littlefs) = Littlefs::new() {
        for entry in std::fs::read_dir("/littlefs")? {
            let entry = entry?;
            log::info!("{:?}", entry.path());
        }

        video_player.play("/littlefs/interstitial.mp4")?;
    }

    if let Ok(_sdcard) = SdCard::new() {
        video_player.play("/sdcard/itysl.mp4")?;
    }

    Ok(())
}
