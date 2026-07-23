//! HTTP Server with custom WiFi SSID
//! based on https://github.com/esp-rs/esp-idf-svc/blob/master/examples/http_server.rs
//!
//! Go to 192.168.71.1 to test

#![allow(unknown_lints)]

fn main() -> anyhow::Result<()> {
    example::main()
}

mod example {
    use core::convert::TryInto;

    use esp_idf_svc::{
        eventloop::EspSystemEventLoop,
        hal::peripherals::Peripherals,
        http::{Method, server::EspHttpServer},
        io::Write,
        nvs::EspDefaultNvsPartition,
        wifi::{AccessPointConfiguration, BlockingWifi, Configuration, EspWifi},
    };

    use log::*;

    const SSID: &str = "ESP-Provision";
    static INDEX_HTML: &str = "This is a test document";

    // Need lots of stack to parse JSON
    const STACK_SIZE: usize = 10240;

    pub fn main() -> anyhow::Result<()> {
        esp_idf_svc::sys::link_patches();
        esp_idf_svc::log::EspLogger::initialize_default();

        // Setup Wifi

        let peripherals = Peripherals::take()?;
        let sys_loop = EspSystemEventLoop::take()?;
        let nvs = EspDefaultNvsPartition::take()?;

        let mut wifi = BlockingWifi::wrap(
            EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
            sys_loop,
        )?;

        connect_wifi(&mut wifi)?;

        let mut server = create_server()?;

        server.fn_handler("/", Method::Get, |req| {
            req.into_ok_response()?
                .write_all(INDEX_HTML.as_bytes())
                .map(|_| ())
        })?;

        // Keep wifi and the server running beyond when main() returns (forever)
        // Do not call this if you ever want to stop or access them later.
        // Otherwise you can either add an infinite loop so the main task
        // never returns, or you can move them to another thread.
        // https://doc.rust-lang.org/stable/core/mem/fn.forget.html
        core::mem::forget(wifi);
        core::mem::forget(server);

        // Main task no longer needed, free up some memory
        Ok(())
    }

    fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
        // If instead of creating a new network you want to serve the page
        // on your local network, you can replace this configuration with
        // the client configuration from the http_client example.
        let wifi_configuration = Configuration::AccessPoint(AccessPointConfiguration {
            ssid: SSID.try_into().unwrap(),
            ..Default::default()
        });

        wifi.set_configuration(&wifi_configuration)?;

        wifi.start()?;
        info!("Wifi started");

        // If using a client configuration you need
        // to connect to the network with:
        //
        //  ```
        //  wifi.connect()?;
        //  info!("Wifi connected");
        // ```

        wifi.wait_netif_up()?;
        info!("Wifi netif up");

        info!("Created Wi-Fi with WIFI_SSID `{SSID}`");

        Ok(())
    }

    fn create_server() -> anyhow::Result<EspHttpServer<'static>> {
        let server_configuration = esp_idf_svc::http::server::Configuration {
            stack_size: STACK_SIZE,
            ..Default::default()
        };

        Ok(EspHttpServer::new(&server_configuration)?)
    }
}
