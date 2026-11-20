//! The screen gateway: every OLED update goes through this module.
//!
//! `Ui::new` probes the display and takes sole ownership of it for the whole
//! firmware run — no other code can reach the `Display`. Business modules
//! update the screen through `Ui` methods (or the `update_status_page`
//! helper that hides the probe-degraded `Option`), never through the driver.
use crate::drivers::display::Display;
use crate::println_at;
use crate::ui::console::{Console, Icon};
use anyhow::Result;
use log::{info, warn};

pub mod console;

/// Text columns of the OLED canvas: 21 glyph cells of 6 px.
const WIDTH: usize = 21;

pub mod task;

pub struct Ui {
    console: console::Console,
    top_bar: Option<String>,
    status: Option<String>,
}

impl Ui {
    /// Probe the OLED and take it over, or `None` when absent — the firmware
    /// keeps running degraded, like every other board probe. This is the
    /// only place the display driver is constructed.
    pub fn new() -> Option<Ui> {
        match Display::new() {
            Ok(screen) => {
                match screen.address() {
                    Ok(address) => {
                        info!("SSD1306 ready: SDA=20 SCL=19 address=0x{address:02X}")
                    }
                    Err(err) => warn!("OLED address probe: {err:#}"),
                }
                Some(Ui {
                    console: Console::new(screen),
                    top_bar: None,
                    status: None,
                })
            }
            Err(err) => {
                warn!("OLED unavailable (SDA20/SCL19): {err:#}");
                None
            }
        }
    }

    pub fn update_and_fmt_top_bar(&mut self,
                              zigbee_status: bool,
                              ble_status: bool,
                              wifi_status: bool,
                              time: &str,
    ) -> Result<()> {
        let zgbe_icn = if zigbee_status {Icon::ZB} else {Icon::WARN};
        let ble_icn = if ble_status {Icon::BLE} else {Icon::WARN};
        let wifi_icn = if wifi_status {Icon::WIFI} else {Icon::WARN};
        // Three icon pairs occupy 6 of the 21 canvas columns; right-align the
        // time inside the remaining field so it stays on-screen.
        let time_field = WIDTH - 6;
        self.top_bar = Some(format!(
            "{wifi_icn}{ble_icn}{zgbe_icn}{:>time_field$}",
            time,
            time_field = time_field
        ));
        println_at!(self.console, 0, "{}", self.top_bar.as_ref().unwrap().as_str())
    }

    /// Overwrite one status row; `line` 1..3 map to canvas rows 2/4/6.
    pub fn update_status(&mut self, msg: &str, line: u8) -> Result<()> {
        self.status = Some(msg.to_owned());
        println_at!(self.console, line * 2, "{}", self.status.as_ref().unwrap())
    }

    /// The standard status pair: canvas rows 2 and 4 in one call.
    pub fn update_status_pair(&mut self, line1: &str, line2: &str) -> Result<()> {
        self.update_status(line1, 1)?;
        self.update_status(line2, 2)
    }
}
