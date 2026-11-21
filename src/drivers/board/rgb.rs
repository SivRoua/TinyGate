use anyhow::{bail, Result};
use esp_idf_svc::sys;
use std::{
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
static OWNED: AtomicBool = AtomicBool::new(false);
pub struct Rgb {
    active: bool,
    _thread: PhantomData<Rc<()>>,
}
fn check(code: i32) -> Result<()> {
    if code != 0 {
        bail!("RGB ESP error 0x{code:X}");
    }
    Ok(())
}
impl Rgb {
    pub fn new() -> Result<Self> {
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            bail!("RGB already owned");
        }
        if let Err(err) = check(unsafe { sys::tg_rgb_init() }) {
            let cleanup = unsafe { sys::tg_rgb_deinit() };
            if cleanup == 0 || cleanup == sys::ESP_ERR_INVALID_STATE {
                OWNED.store(false, Ordering::Release);
            }
            return Err(err);
        }
        Ok(Self {
            active: true,
            _thread: PhantomData,
        })
    }
    pub fn set(&mut self, r: u8, g: u8, b: u8, brightness: u8) -> Result<()> {
        if !self.active {
            bail!("RGB shut down");
        }
        check(unsafe { sys::tg_rgb_set(r, g, b, brightness) })
    }
    pub fn shutdown(&mut self) -> Result<()> {
        if !self.active {
            bail!("RGB shut down");
        }
        check(unsafe { sys::tg_rgb_deinit() })?;
        self.active = false;
        OWNED.store(false, Ordering::Release);
        Ok(())
    }
}
impl Drop for Rgb {
    fn drop(&mut self) {
        if self.active {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}
