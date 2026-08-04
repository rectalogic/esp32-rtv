use esp32_rtv::{sdcard::SdCard, spiffs::Spiffs, video_player::VideoPlayer};

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    if let Ok(_spiffs) = Spiffs::new() {
        for entry in std::fs::read_dir("/spiffs")? {
            let entry = entry?;
            log::info!("{:?}", entry.path());
        }
    }

    if let Ok(_sdcard) = SdCard::new() {
        let video_player = VideoPlayer::new()?;
        video_player.play("/sdcard/itysl.mp4")?;
    }

    Ok(())
}
