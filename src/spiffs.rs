use esp_idf_svc::sys::video_player::{mount_spiffs, unmount_spiffs};
use esp_idf_svc::sys::{ESP_OK, esp_err_t};

pub struct Spiffs {
    _private: (),
}

impl Spiffs {
    pub fn new() -> Result<Self, esp_err_t> {
        let ret = unsafe { mount_spiffs() };
        if ret != ESP_OK {
            return Err(ret);
        }
        Ok(Self { _private: () })
    }
}

impl Drop for Spiffs {
    fn drop(&mut self) {
        unsafe { unmount_spiffs() };
    }
}
