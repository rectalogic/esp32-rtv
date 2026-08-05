use esp_idf_svc::sys::video_player::{mount_littlefs, unmount_littlefs};
use esp_idf_svc::sys::{EspError, esp_result};

pub struct Littlefs {
    _private: (),
}

impl Littlefs {
    pub fn new() -> Result<Self, EspError> {
        esp_result!(unsafe { mount_littlefs() }, Self { _private: () })
    }
}

impl Drop for Littlefs {
    fn drop(&mut self) {
        unsafe { unmount_littlefs() };
    }
}
