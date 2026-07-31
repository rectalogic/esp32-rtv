use std::{ffi, ptr};

use esp_idf_svc::sys::{
    ESP_OK, esp_err_to_name, esp_vfs_spiffs_conf_t, esp_vfs_spiffs_register, esp_vfs_unregister,
};

pub struct Spiffs {
    base_path: ffi::CString,
}

impl Spiffs {
    pub fn new(base_path: &str) -> anyhow::Result<Self> {
        let base_path = ffi::CString::new(base_path)?;
        let conf = esp_vfs_spiffs_conf_t {
            base_path: base_path.as_ptr(),
            partition_label: ptr::null(), // uses first partition with subtype=spiffs
            max_files: 5,
            format_if_mount_failed: false,
        };

        let ret = unsafe { esp_vfs_spiffs_register(&conf) };
        if ret != ESP_OK {
            unsafe {
                esp_vfs_unregister(conf.base_path);
            }
            return Err(anyhow::anyhow!("Failed to mount SPIFFS: {:?}", unsafe {
                ffi::CStr::from_ptr(esp_err_to_name(ret))
            }));
        }
        Ok(Self { base_path })
    }
}

impl Drop for Spiffs {
    fn drop(&mut self) {
        unsafe { esp_vfs_unregister(self.base_path.as_ptr()) };
    }
}
