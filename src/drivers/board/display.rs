//! Single-owner OLED facade. C owns I2C and framebuffer; Rust owns presentation.
use crate::drivers::lease::{check, Lease};
use anyhow::Result;
use esp_idf_svc::sys;
use std::ffi::CString;
use std::sync::atomic::AtomicBool;

static OWNED: AtomicBool = AtomicBool::new(false);

/// `Send` by design: exactly one instance exists (the `OWNED` guard) and all
/// calls must stay on the owning UI task — the C driver serializes nothing
/// itself. The task structure enforces the single-caller contract; the
/// executor never migrates the task between threads.
pub struct Display {
    lease: Lease,
}

impl Display {
    pub fn new() -> Result<Self> {
        let mut lease = Lease::acquire(&OWNED, "OLED")?;
        if let Err(err) = check(unsafe { sys::tg_display_init() }, "OLED init") {
            let cleanup = unsafe { sys::tg_display_deinit() };
            if cleanup == sys::ESP_OK || cleanup == sys::ESP_ERR_INVALID_STATE {
                lease.release();
            }
            return Err(err);
        }
        Ok(Self { lease })
    }

    fn ensure(&self) -> Result<()> {
        self.lease.ensure("OLED")
    }

    pub fn address(&self) -> Result<u8> {
        self.ensure()?;
        let mut address = 0;
        check(unsafe { sys::tg_display_get_address(&mut address) }, "OLED address")?;
        Ok(address)
    }

    pub fn clear(&mut self) -> Result<()> {
        self.ensure()?;
        check(unsafe { sys::tg_display_clear() }, "OLED clear")
    }

    pub fn write_line(&mut self, row: u8, text: &str) -> Result<()> {
        self.ensure()?;
        let text = encode_display_text(text)?;
        check(
            unsafe { sys::tg_display_write_line(row, text.as_ptr()) },
            "OLED write line",
        )
    }

    /// Refresh a single text row (rows map 1:1 to SSD1306 pages).
    pub fn flush_page(&mut self, row: u8) -> Result<()> {
        self.ensure()?;
        check(
            unsafe { sys::tg_display_flush_page(row) },
            "OLED flush page",
        )
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.ensure()?;
        check(unsafe { sys::tg_display_deinit() }, "OLED deinit")?;
        self.lease.release();
        Ok(())
    }
}

/// Drop is the clean-shutdown path, not a bug to dodge: when the single
/// long-lived owner goes away, the panel powers off and the I2C bus is
/// released. Keep the `Display` in one owner for the whole firmware run
/// (the UI task). To freeze on a frame for visual inspection, park that
/// owner in a `loop { sleep }` — never leak it.
impl Drop for Display {
    fn drop(&mut self) {
        if self.lease.is_active() {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
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
