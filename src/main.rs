use esp32_rtv::spiffs::Spiffs;

fn main() -> anyhow::Result<()>{
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    if let Ok(spiffs) = Spiffs::new("/spiffs") {
        for entry in std::fs::read_dir("/spiffs")? {
            let entry = entry?;
            log::info!("{:?}", entry.path());
        }
    }

    unsafe {
        esp_idf_svc::sys::video_player::run_player()
    };
    Ok(())
}
