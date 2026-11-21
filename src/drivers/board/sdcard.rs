//! Owns the FAT mount while Rust uses std::fs through ESP-IDF VFS.
use anyhow::{bail, Result};
use esp_idf_svc::sys;
use std::{
    marker::PhantomData,
    path::Path,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

static OWNED: AtomicBool = AtomicBool::new(false);

pub struct SdCard {
    mounted: bool,
    _task: PhantomData<Rc<()>>,
}

fn check(code: sys::esp_err_t, operation: &str) -> Result<()> {
    if code != sys::ESP_OK {
        bail!("TF card {operation}: ESP error 0x{code:X}");
    }
    Ok(())
}

impl SdCard {
    pub fn mount() -> Result<Self> {
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            bail!("TF card driver already has an owner");
        }
        if let Err(err) = check(unsafe { sys::tg_sdcard_init() }, "mount") {
            if unsafe { sys::tg_sdcard_deinit() } == sys::ESP_OK {
                OWNED.store(false, Ordering::Release);
            }
            return Err(err);
        }
        Ok(Self {
            mounted: true,
            _task: PhantomData,
        })
    }

    pub fn info(&self) -> Result<sys::tg_sdcard_info_t> {
        let mut info = sys::tg_sdcard_info_t {
            capacity_bytes: 0,
            sector_size_bytes: 0,
            clock_khz: 0,
        };
        check(unsafe { sys::tg_sdcard_get_info(&mut info) }, "info")?;
        Ok(info)
    }

    pub fn path(&self) -> &Path {
        let path = std::ffi::CStr::from_bytes_with_nul(sys::TG_SDCARD_MOUNT_PATH)
            .expect("mount path is a C string");
        Path::new(path.to_str().expect("mount path is UTF-8"))
    }

    /// All files opened below path() must be closed first.
    pub fn unmount(&mut self) -> Result<()> {
        if self.mounted {
            check(unsafe { sys::tg_sdcard_deinit() }, "unmount")?;
            self.mounted = false;
            OWNED.store(false, Ordering::Release);
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
