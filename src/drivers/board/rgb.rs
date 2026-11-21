//! GPIO8 addressable RGB indicator.
use crate::drivers::lease::{Lease, check};
use anyhow::Result;
use esp_idf_svc::sys;
use std::sync::atomic::AtomicBool;

static OWNED: AtomicBool = AtomicBool::new(false);

pub struct Rgb {
    lease: Lease,
}

impl Rgb {
    pub fn new() -> Result<Self> {
        let mut lease = Lease::acquire(&OWNED, "RGB")?;
        if let Err(err) = check(unsafe { sys::tg_rgb_init() }, "RGB init") {
            let cleanup = unsafe { sys::tg_rgb_deinit() };
            if cleanup == sys::ESP_OK || cleanup == sys::ESP_ERR_INVALID_STATE {
                lease.release();
            }
            return Err(err);
        }
        Ok(Self { lease })
    }

    pub fn set(&mut self, r: u8, g: u8, b: u8, brightness: u8) -> Result<()> {
        self.lease.ensure("RGB")?;
        check(unsafe { sys::tg_rgb_set(r, g, b, brightness) }, "RGB set")
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.lease.ensure("RGB")?;
        check(unsafe { sys::tg_rgb_deinit() }, "RGB deinit")?;
        self.lease.release();
        Ok(())
    }
}

impl Drop for Rgb {
    fn drop(&mut self) {
        if self.lease.is_active() {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}
