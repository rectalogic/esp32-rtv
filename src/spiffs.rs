use esp_idf_svc::sys::video_player::{mount_spiffs, unmount_spiffs};
use esp_idf_svc::sys::{EspError, esp_result};

pub struct Spiffs {
    _private: (),
}

impl Spiffs {
    pub fn new() -> Result<Self, EspError> {
        esp_result!(unsafe { mount_spiffs() }, Self { _private: () })
    }
}

impl Drop for Spiffs {
    fn drop(&mut self) {
        unsafe { unmount_spiffs() };
    }
}
