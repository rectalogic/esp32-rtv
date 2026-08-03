use std::ffi;

use esp_idf_svc::sys::video_player::{mount_sdcard, unmount_sdcard};
use esp_idf_svc::sys::{ESP_OK, esp_err_to_name};

pub struct SdCard {
    _private: (),
}

impl SdCard {
    pub fn new() -> anyhow::Result<Self> {
        let result = unsafe { mount_sdcard() };
        if result != ESP_OK {
            return Err(anyhow::anyhow!("Failed to mount SD card: {:?}", unsafe {
                ffi::CStr::from_ptr(esp_err_to_name(result))
            }));
        }
        Ok(Self { _private: () })
    }
}

impl Drop for SdCard {
    fn drop(&mut self) {
        unsafe { unmount_sdcard() };
    }
}
