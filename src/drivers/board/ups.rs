//! UPS power path: battery voltage and mains-loss flag.
//!
//! The C component still reports placeholder values (see
//! `docs/onboard-drivers.md`); the Rust facade already exposes the product
//! shape the power milestone will consume.
use crate::drivers::lease::check;
use anyhow::Result;
use esp_idf_svc::sys;

pub struct Ups;

impl Ups {
    /// Initialize the UPS sampling path. No owner guard: the C component
    /// exposes no deinit, so the device is initialized once per boot.
    pub fn init() -> Result<()> {
        check(unsafe { sys::tg_ups_init() }, "UPS init")
    }

    /// Battery voltage in volts.
    pub fn battery_voltage() -> f32 {
        unsafe { sys::tg_ups_get_battery_voltage() }
    }

    /// Whether external power has dropped.
    pub fn is_power_loss() -> bool {
        unsafe { sys::tg_ups_is_mains_power_loss() }
    }
}
