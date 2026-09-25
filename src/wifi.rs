use esp_idf_svc::{
    sys::{
        EspError, esp, network_prov_event_handler_t, network_prov_mgr_config_t,
        network_prov_mgr_deinit, network_prov_mgr_init, network_prov_mgr_is_wifi_provisioned,
        network_prov_mgr_reset_wifi_provisioning, network_prov_mgr_start_provisioning,
        network_prov_mgr_stop_provisioning, network_prov_mgr_wait, network_prov_scheme_ble,
        network_prov_scheme_ble_event_cb_free_btdm, network_prov_security_NETWORK_PROV_SECURITY_1,
        network_prov_security_t,
    },
    wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use std::{ffi::CString, ptr};

pub const SERVICE_NAME: &str = "PROV_MINITV";

pub struct WifiProvisioning(());

impl WifiProvisioning {
    pub fn new() -> Result<Self, EspError> {
        unsafe {
            let config = network_prov_mgr_config_t {
                scheme: network_prov_scheme_ble,
                scheme_event_handler: network_prov_event_handler_t {
                    event_cb: Some(network_prov_scheme_ble_event_cb_free_btdm),
                    user_data: ptr::null_mut(),
                },
                ..Default::default()
            };
            esp!(network_prov_mgr_init(config))?;
        }
        Ok(WifiProvisioning(()))
    }

    pub fn ensure_provisioned(&self, wifi: &mut BlockingWifi<EspWifi>) -> Result<(), EspError> {
        if !self.is_provisioned()? {
            wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;
            wifi.start()?;
            self.start_provisioning(network_prov_security_NETWORK_PROV_SECURITY_1, SERVICE_NAME)?;
            self.wait();
            self.stop();
        } else {
            wifi.start()?;
            if let Err(e) = wifi.connect() {
                self.reset()?;
                return Err(e);
            }
        }
        wifi.wait_netif_up()?;
        Ok(())
    }

    fn start_provisioning(
        &self,
        security: network_prov_security_t,
        service_name: &str,
    ) -> Result<(), EspError> {
        let service_name = CString::new(service_name).unwrap();
        unsafe {
            esp!(network_prov_mgr_start_provisioning(
                security,
                ptr::null(),
                service_name.as_ptr(),
                ptr::null(),
            ))?;
        }
        Ok(())
    }

    fn wait(&self) {
        unsafe {
            network_prov_mgr_wait();
        }
    }

    pub fn is_provisioned(&self) -> Result<bool, EspError> {
        let mut provisioned: bool = false;
        esp!(unsafe { network_prov_mgr_is_wifi_provisioned(&mut provisioned) })?;
        Ok(provisioned)
    }

    fn stop(&self) {
        unsafe {
            network_prov_mgr_stop_provisioning();
        }
    }

    fn reset(&self) -> Result<(), EspError> {
        esp!(unsafe { network_prov_mgr_reset_wifi_provisioning() })
    }
}

impl Drop for WifiProvisioning {
    fn drop(&mut self) {
        unsafe {
            network_prov_mgr_deinit();
        }
    }
}
