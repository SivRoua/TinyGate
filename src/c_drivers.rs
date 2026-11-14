use anyhow::{bail, Result};
use std::ffi::CString;

#[allow(dead_code)]
extern "C" {
    pub fn tg_c_drivers_init() -> i32;
    pub fn tg_ups_init() -> i32;
    pub fn tg_ups_get_battery_voltage() -> f32;
    pub fn tg_ups_is_mains_power_loss() -> bool;
    pub fn tg_display_init() -> i32;
    pub fn tg_display_clear() -> i32;
    pub fn tg_display_show_status(
        title: *const std::ffi::c_char,
        line1: *const std::ffi::c_char,
        line2: *const std::ffi::c_char,
    ) -> i32;
    pub fn tg_sdcard_init() -> i32;
    pub fn tg_sdcard_is_mounted() -> bool;
    pub fn tg_zigbee_coord_init() -> i32;
}

pub struct CDrivers;

impl CDrivers {
    /// Initialize all low-level C peripheral drivers
    pub fn init_all() -> Result<()> {
        let ret = unsafe { tg_c_drivers_init() };
        if ret != 0 {
            bail!("tg_c_drivers_init failed with error code: {}", ret);
        }
        Ok(())
    }

    /// Read UPS battery voltage (V)
    pub fn battery_voltage() -> f32 {
        unsafe { tg_ups_get_battery_voltage() }
    }

    /// Check whether external power has dropped (UPS state)
    pub fn is_power_loss() -> bool {
        unsafe { tg_ups_is_mains_power_loss() }
    }

    /// Check if MicroSD card is mounted
    pub fn is_sd_mounted() -> bool {
        unsafe { tg_sdcard_is_mounted() }
    }

    /// Display status lines on OLED SSD1306
    pub fn display_status(title: &str, line1: &str, line2: &str) -> Result<()> {
        let c_title = CString::new(title)?;
        let c_l1 = CString::new(line1)?;
        let c_l2 = CString::new(line2)?;
        let ret = unsafe { tg_display_show_status(c_title.as_ptr(), c_l1.as_ptr(), c_l2.as_ptr()) };
        if ret != 0 {
            bail!("tg_display_show_status failed with code: {}", ret);
        }
        Ok(())
    }
}
