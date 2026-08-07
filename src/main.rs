use esp32_rtv::app::app;

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    let _logger = esp_idf_svc::log::init_from_esp_idf();
    // logger
    //     .filter()
    //     .set_target_level("ESP_PLAYER", log::LevelFilter::Debug)?;

    app()
}
