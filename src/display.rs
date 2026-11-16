//! Single-owner OLED facade. C owns I2C and framebuffer; Rust owns presentation.
use anyhow::{bail, Result};
use esp_idf_svc::sys;
use std::ffi::CString;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

static OWNED: AtomicBool = AtomicBool::new(false);
pub struct Display {
    active: bool,
    _thread: PhantomData<Rc<()>>,
}
fn check(code: i32) -> Result<()> {
    if code != sys::ESP_OK {
        bail!("OLED: ESP error 0x{code:X}");
    }
    Ok(())
}
impl Display {
    pub fn new() -> Result<Self> {
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            bail!("OLED already owned");
        }
        if let Err(err) = check(unsafe { sys::tg_display_init() }) {
            let cleanup = unsafe { sys::tg_display_deinit() };
            if cleanup == sys::ESP_OK || cleanup == sys::ESP_ERR_INVALID_STATE {
                OWNED.store(false, Ordering::Release);
            }
            return Err(err);
        }
        Ok(Self {
            active: true,
            _thread: PhantomData,
        })
    }
    fn ensure(&self) -> Result<()> {
        if !self.active {
            bail!("OLED already shut down");
        }
        Ok(())
    }
    pub fn address(&self) -> Result<u8> {
        self.ensure()?;
        let mut address = 0;
        check(unsafe { sys::tg_display_get_address(&mut address) })?;
        Ok(address)
    }
    pub fn show_status(&mut self, title: &str, line1: &str, line2: &str) -> Result<()> {
        self.ensure()?;
        let title = CString::new(title)?;
        let line1 = CString::new(line1)?;
        let line2 = CString::new(line2)?;
        check(unsafe {
            sys::tg_display_show_status(title.as_ptr(), line1.as_ptr(), line2.as_ptr())
        })
    }
    #[allow(dead_code)]
    pub fn clear(&mut self) -> Result<()> {
        self.ensure()?;
        check(unsafe { sys::tg_display_clear() })
    }
    #[allow(dead_code)]
    pub fn write_line(&mut self, row: u8, text: &str) -> Result<()> {
        self.ensure()?;
        let text = CString::new(text)?;
        check(unsafe { sys::tg_display_write_line(row, text.as_ptr()) })
    }
    #[allow(dead_code)]
    pub fn set_pixel(&mut self, x: u16, y: u16, on: bool) -> Result<()> {
        self.ensure()?;
        check(unsafe { sys::tg_display_set_pixel(x, y, on) })
    }
    #[allow(dead_code)]
    pub fn flush(&mut self) -> Result<()> {
        self.ensure()?;
        check(unsafe { sys::tg_display_flush() })
    }
    #[allow(dead_code)]
    pub fn set_power(&mut self, on: bool) -> Result<()> {
        self.ensure()?;
        check(unsafe { sys::tg_display_set_power(on) })
    }
    #[allow(dead_code)]
    pub fn set_contrast(&mut self, value: u8) -> Result<()> {
        self.ensure()?;
        check(unsafe { sys::tg_display_set_contrast(value) })
    }
    pub fn shutdown(&mut self) -> Result<()> {
        self.ensure()?;
        check(unsafe { sys::tg_display_deinit() })?;
        self.active = false;
        OWNED.store(false, Ordering::Release);
        Ok(())
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        if self.active {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}
