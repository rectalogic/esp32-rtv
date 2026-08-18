use esp_idf_svc::sys::{
    EspError, esp_result,
    video_player::{mount_sdcard, unmount_sdcard},
};

pub struct SdCard {
    _private: (),
}

impl SdCard {
    pub fn new() -> Result<Self, EspError> {
        esp_result!(unsafe { mount_sdcard() }, Self { _private: () })
    }
}

impl Drop for SdCard {
    fn drop(&mut self) {
        unsafe { unmount_sdcard() };
    }
}
