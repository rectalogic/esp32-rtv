use esp_idf_svc::sys::video_player::{mount_sdcard, unmount_sdcard};
use esp_idf_svc::sys::{ESP_OK, esp_err_t};

pub struct SdCard {
    _private: (),
}

impl SdCard {
    pub fn new() -> Result<Self, esp_err_t> {
        let ret = unsafe { mount_sdcard() };
        if ret != ESP_OK {
            return Err(ret);
        }
        Ok(Self { _private: () })
    }
}

impl Drop for SdCard {
    fn drop(&mut self) {
        unsafe { unmount_sdcard() };
    }
}
