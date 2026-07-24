use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;

fn main() -> anyhow::Result<()> {
    // 1. Core linking configuration (Mandatory)
    esp_idf_svc::sys::link_patches();

    // 2. Clear out standard peripherals & NVS parameters
    let _peripherals = Peripherals::take().unwrap();
    let _nvs = EspDefaultNvsPartition::take().unwrap();

    println!("Initializing native C component esp_wifi_config...");

    // 3. Mirroring: esp_bus_init();
    // Access through your auto-generated bindgen bindings module space
    unsafe {
        esp_idf_svc::sys::esp_wifi_config::esp_bus_init();
    }

    // 5. Construct the primary configuration blueprint block
    let config = esp_idf_svc::sys::esp_wifi_config::wifi_cfg_config_t {
        // Maps cleanly to C's enum variants (WIFI_PROV_ON_FAILURE)
        provisioning_mode:
            esp_idf_svc::sys::esp_wifi_config::wifi_provisioning_mode_t_WIFI_PROV_ON_FAILURE,
        stop_provisioning_on_connect: true,
        enable_ap: true,
        ..Default::default()
    };

    unsafe {
        // 6. Execute initialization matching: wifi_cfg_init(&config);
        esp_idf_svc::sys::esp_wifi_config::wifi_cfg_init(&config);

        println!("Provisioning engine running. Awaiting association validation...");

        // 7. Mirroring: wifi_cfg_wait_connected(30000);
        esp_idf_svc::sys::esp_wifi_config::wifi_cfg_wait_connected(30000);
    }
    // XXX see examples - need to register callbacks esp_bus_sub?
    // https://github.com/thorrak/esp_wifi_config/blob/main/examples/with_ble/main/main.c

    // Keep the main thread alive while the C component's background
    // FreeRTOS tasks run the provisioning state machine.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
