//! TinyGate gateway library: board drivers, wireless radios, and validation apps.
pub mod apps;
pub mod drivers;
pub mod hardware;
pub mod platform;
pub mod radio;
pub mod ui;

use crate::drivers::button::Button;
use crate::ui::Ui;
use log::{info, warn};
use std::time::{Duration, Instant};

pub fn poll_button(button: &mut Option<Button>) {
    if let Some(button) = button {
        // Limit each batch, so a busy input never monopolizes the application loop.
        for _ in 0..16 {
            match button.poll() {
                Ok(Some(event)) => info!(
                    "BUTTON GPIO9 {} held={}ms at={}ms",
                    event.kind, event.held_ms, event.timestamp_ms
                ),
                Ok(None) => break,
                Err(err) => {
                    warn!("Button event: {err:#}");
                    break;
                }
            }
        }
    }
}

pub fn idle_with_button(button: &mut Option<Button>, duration: Duration) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        poll_button(button);
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Screen updates flow through the UI layer only: this helper hides the
/// probe-degraded `Option<Ui>` for app loops, the way `poll_button` does for
/// the button slot. Writes the standard status pair (canvas rows 2 and 4)
/// and leaves the top bar alone.
pub fn update_status_page(ui: &mut Option<Ui>, line1: &str, line2: &str) {
    if let Some(ui) = ui {
        if let Err(err) = ui.update_status_pair(line1, line2) {
            warn!("OLED update: {err:#}");
        }
    }
}
