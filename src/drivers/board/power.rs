//! Power-fail hold-up over a supercapacitor.
//!
//! The supercap does not run the gateway; it only keeps the rail alive long
//! enough to flush critical state after mains loss. There is deliberately no
//! battery voltage, state-of-charge or remaining-runtime API: the only
//! product questions are "did mains drop?" and "how much hold-up is left?".
use crate::drivers::lease::check;
use anyhow::Result;
use esp_idf_svc::sys;

pub struct Power;

impl Power {
    /// Arm mains-loss detection and supercap rail sampling.
    pub fn init() -> Result<()> {
        check(unsafe { sys::tg_power_init() }, "Power init")
    }

    /// Whether external power has dropped.
    pub fn mains_lost() -> bool {
        unsafe { sys::tg_power_mains_lost() }
    }

    /// Supercap rail voltage in volts. Use it only to bound the save window;
    /// it is not a battery state-of-charge estimate.
    pub fn rail_voltage() -> f32 {
        unsafe { sys::tg_power_rail_voltage() }
    }
}
