use anyhow::{bail, Result};

#[allow(dead_code)]
unsafe extern "C" {
    pub fn tg_c_drivers_init() -> i32;
    pub fn tg_ups_init() -> i32;
    pub fn tg_ups_get_battery_voltage() -> f32;
    pub fn tg_ups_is_mains_power_loss() -> bool;
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
}
