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

/// Display text encoding: U+E080..U+E0FF map onto the C canvas icon code
/// space (single bytes 0x80..0xFF, paired halves form 11x7 icons); all other
/// text passes through as UTF-8 and unknown code points render as '?'.
/// Returns Err on interior NUL, mirroring `CString::new`.
fn encode_display_text(text: &str) -> Result<CString> {
    let mut bytes = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let code = ch as u32;
        if (0xE080..=0xE0FF).contains(&code) {
            bytes.push((code - 0xE000) as u8);
        } else {
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
        }
    }
    Ok(CString::new(bytes)?)
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
        let title = encode_display_text(title)?;
        let line1 = encode_display_text(line1)?;
        let line2 = encode_display_text(line2)?;
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
        let text = encode_display_text(text)?;
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
    /// Refresh a single text row (rows map 1:1 to SSD1306 pages).
    pub fn flush_page(&mut self, row: u8) -> Result<()> {
        self.ensure()?;
        check(unsafe { sys::tg_display_flush_page(row) })
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
/// Drop is the clean-shutdown path, not a bug to dodge: when the single
/// long-lived owner goes away, the panel powers off and the I2C bus is
/// released. Keep the `Display` in one owner for the whole firmware run
/// (an app loop, or later a dedicated UI task). To freeze on a frame for
/// visual inspection, park that owner in a `loop { sleep }` — never leak it.
impl Drop for Display {
    fn drop(&mut self) {
        if self.active {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::encode_display_text;

    #[test]
    fn pua_icons_encode_as_single_high_bytes() {
        let encoded = encode_display_text("\u{E080}\u{E08F} OK").unwrap();
        assert_eq!(encoded.to_bytes(), [0x80, 0x8F, b' ', b'O', b'K']);
    }

    #[test]
    fn plain_utf8_passes_through_unchanged() {
        let text = "TinyGate 100%";
        assert_eq!(
            encode_display_text(text).unwrap().to_bytes(),
            text.as_bytes()
        );
        // Non-icon private-use characters stay as their UTF-8 bytes.
        assert_eq!(
            encode_display_text("\u{E07F}").unwrap().to_bytes(),
            "\u{E07F}".as_bytes()
        );
    }

    #[test]
    fn interior_nul_is_rejected_like_cstring() {
        assert!(encode_display_text("a\0b").is_err());
    }
}
