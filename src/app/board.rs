//! Board assembly: probe every peripheral and degrade individually.
//!
//! A missing or failed peripheral never blocks the firmware: each probe logs a
//! warning and leaves that slot as `None`. Apps treat `Option` as "feature
//! unavailable on this board". The OLED is deliberately **not** a slot here:
//! the UI layer probes and owns it (`crate::ui::Ui::new`), because the screen
//! may only be updated through the UI — no business code holds the driver.
use crate::drivers::board::{button, rgb};
use crate::drivers::sys;
use log::{info, warn};

pub struct Hardware {
    pub rgb: Option<rgb::Rgb>,
    pub button: Option<button::Button>,
}

impl Hardware {
    /// Probe all board peripherals, degrading each one on failure.
    pub fn probe() -> Self {
        Self {
            rgb: probe_rgb(),
            button: probe_button(),
        }
    }

    /// Log chip, reset and heap state once at boot.
    pub fn log_system(&self) -> anyhow::Result<()> {
        let system = sys::system_info()?;
        info!(
            "Chip model={} revision={} cores={} reset={} heap={} min_heap={} largest={}",
            system.chip_model,
            system.revision,
            system.cores,
            system.reset_reason,
            system.free_heap_bytes,
            system.minimum_free_heap_bytes,
            system.largest_free_block_bytes
        );
        Ok(())
    }
}

fn probe_rgb() -> Option<rgb::Rgb> {
    match rgb::Rgb::new() {
        Ok(mut led) => {
            if let Err(err) = led.set(0, 0, 255, 24) {
                warn!("RGB init color: {err:#}");
            }
            Some(led)
        }
        Err(err) => {
            warn!("RGB unavailable: {err:#}");
            None
        }
    }
}

fn probe_button() -> Option<button::Button> {
    match button::Button::new() {
        Ok(button) => {
            info!("Button ready: GPIO9 -> GND, pull-up, debounce=30ms long=1000ms");
            Some(button)
        }
        Err(err) => {
            warn!("Button unavailable: {err:#}");
            None
        }
    }
}
