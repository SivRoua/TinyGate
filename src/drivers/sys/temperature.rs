//! On-chip temperature sensor (wraps tg_temperature).
use anyhow::Result;
use esp_idf_svc::sys;
use std::sync::atomic::AtomicBool;

use crate::drivers::lease::{Lease, check};

static TEMPERATURE_OWNED: AtomicBool = AtomicBool::new(false);

pub struct Temperature {
    lease: Lease,
}
impl Temperature {
    pub fn new() -> Result<Self> {
        let mut lease = Lease::acquire(&TEMPERATURE_OWNED, "Temperature")?;
        if let Err(err) = check(
            unsafe { sys::tg_temperature_init(-10, 80) },
            "temperature init",
        ) {
            // init normally rolls back itself; deinit also covers partial enable failure.
            let cleanup = unsafe { sys::tg_temperature_deinit() };
            if cleanup == sys::ESP_OK || cleanup == sys::ESP_ERR_INVALID_STATE {
                lease.release();
            }
            return Err(err);
        }
        Ok(Self { lease })
    }
    pub fn read(&self) -> Result<f32> {
        self.lease.ensure("Temperature")?;
        let mut value = 0.0;
        check(
            unsafe { sys::tg_temperature_read(&mut value) },
            "temperature read",
        )?;
        Ok(value)
    }
    pub fn shutdown(&mut self) -> Result<()> {
        self.lease.ensure("Temperature")?;
        check(
            unsafe { sys::tg_temperature_deinit() },
            "temperature deinit",
        )?;
        self.lease.release();
        Ok(())
    }
}
impl Drop for Temperature {
    fn drop(&mut self) {
        if self.lease.is_active() {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}
