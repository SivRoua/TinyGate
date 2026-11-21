//! Owns the FAT mount while Rust uses std::fs through ESP-IDF VFS.
use crate::drivers::lease::{Lease, check};
use anyhow::Result;
use esp_idf_svc::sys;
use std::path::Path;
use std::sync::atomic::AtomicBool;

static OWNED: AtomicBool = AtomicBool::new(false);

pub struct SdCard {
    lease: Lease,
}

impl SdCard {
    pub fn mount() -> Result<Self> {
        let mut lease = Lease::acquire(&OWNED, "TF card driver")?;
        if let Err(err) = check(unsafe { sys::tg_sdcard_init() }, "TF card mount") {
            if unsafe { sys::tg_sdcard_deinit() } == sys::ESP_OK {
                lease.release();
            }
            return Err(err);
        }
        Ok(Self { lease })
    }

    pub fn info(&self) -> Result<sys::tg_sdcard_info_t> {
        let mut info = sys::tg_sdcard_info_t {
            capacity_bytes: 0,
            sector_size_bytes: 0,
            clock_khz: 0,
        };
        check(
            unsafe { sys::tg_sdcard_get_info(&mut info) },
            "TF card info",
        )?;
        Ok(info)
    }

    pub fn path(&self) -> &Path {
        let path = std::ffi::CStr::from_bytes_with_nul(sys::TG_SDCARD_MOUNT_PATH)
            .expect("mount path is a C string");
        Path::new(path.to_str().expect("mount path is UTF-8"))
    }

    /// All files opened below path() must be closed first.
    pub fn unmount(&mut self) -> Result<()> {
        if self.lease.is_active() {
            check(unsafe { sys::tg_sdcard_deinit() }, "TF card unmount")?;
            self.lease.release();
        }
        Ok(())
    }
}

impl Drop for SdCard {
    fn drop(&mut self) {
        if let Err(err) = self.unmount() {
            log::warn!("{err:#}");
        }
    }
}
