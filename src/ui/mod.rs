//! The screen gateway: every OLED update goes through this module.
//!
//! `Ui::new` probes the display and takes sole ownership of it for the whole
//! firmware run — no other code can reach the `Display`. The console state
//! machine is backend-agnostic; this module is the only place that attaches
//! the real OLED to it.
use crate::drivers::board::display::Display;
use crate::ui::console::{Backend, Console};
use crate::ui::icon::Icon;
use anyhow::Result;
use log::{info, warn};

pub mod console;
pub mod icon;
mod macros;
pub mod task;
pub mod view;

/// Text columns of the OLED canvas: 21 glyph cells of 6 px.
const WIDTH: usize = 21;

/// Link state for the persistent top bar.
#[derive(Clone, Copy, Debug, Default)]
pub struct Links {
    pub wifi: bool,
    pub ble: bool,
    pub zigbee: bool,
}

impl Backend for Display {
    fn write_line(&mut self, row: u8, text: &str) -> Result<()> {
        Display::write_line(self, row, text)
    }
    fn flush_page(&mut self, row: u8) -> Result<()> {
        Display::flush_page(self, row)
    }
    fn clear(&mut self) -> Result<()> {
        Display::clear(self)
    }
}

pub struct Ui {
    console: Console<Display>,
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
                    console: Console::with_backend(screen),
                })
            }
            Err(err) => {
                warn!("OLED unavailable (SDA20/SCL19): {err:#}");
                None
            }
        }
    }

    /// Draw the persistent top bar: three link icons plus a right-aligned
    /// clock. Icons occupy 6 of the 21 canvas columns.
    pub fn draw_top_bar(&mut self, links: Links, time: &str) -> Result<()> {
        let wifi = if links.wifi { Icon::WIFI } else { Icon::WARN };
        let ble = if links.ble { Icon::BLE } else { Icon::WARN };
        let zb = if links.zigbee { Icon::ZB } else { Icon::WARN };
        let width = WIDTH - 6;
        crate::println_at!(self.console, 0, "{wifi}{ble}{zb}{time:>width$}")
    }

    /// Overwrite one canvas row (0..8), leaving the top bar to its owner.
    pub fn draw_row(&mut self, row: u8, text: &str) -> Result<()> {
        crate::println_at!(self.console, row, "{text}")
    }

    /// The standard status pair: canvas rows 2 and 4 in one call.
    pub fn draw_status_pair(&mut self, line1: &str, line2: &str) -> Result<()> {
        self.draw_row(2, line1)?;
        self.draw_row(4, line2)
    }

    /// Blank the panel and reset the status-page cursor.
    pub fn clear(&mut self) -> Result<()> {
        self.console.clear()
    }
}
