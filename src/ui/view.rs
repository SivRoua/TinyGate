//! Presentation mapping: gateway events to status-page text.
//!
//! Nothing here touches the display; [`ViewState`] is pure state and
//! [`ViewState::render`] hands finished strings to the [`Ui`] facade.
use crate::drivers::sys;
use crate::gateway::device::{Property, Value};
use crate::gateway::event::GatewayEvent;
use crate::ui::{Links, Ui};

#[derive(Default)]
pub struct ViewState {
    zigbee_ready: bool,
    ble_ready: bool,
    wifi_ready: bool,
    peer: Option<(u16, bool)>,
    latest: Option<String>,
}

impl ViewState {
    pub fn apply(&mut self, event: GatewayEvent) {
        match event {
            GatewayEvent::RadioZigbee { ready } => self.zigbee_ready = ready,
            GatewayEvent::PeerPresence { address, online } => self.peer = Some((address, online)),
            GatewayEvent::Property {
                property, value, ..
            } => {
                // Short 5x7-font labels; "TEMP 25.50 C" / "PWR ON".
                let (label, unit) = match property {
                    Property::Temperature => ("TEMP", " C"),
                    Property::Power => ("PWR", ""),
                };
                self.latest = Some(format!("{label} {}{unit}", render_value(&value)));
            }
        }
    }

    pub fn render(&self, ui: &mut Ui) {
        let links = Links {
            wifi: self.wifi_ready,
            ble: self.ble_ready,
            zigbee: self.zigbee_ready,
        };
        let _ = ui.draw_top_bar(links, &uptime_clock());
        let line1 = match self.peer {
            Some((address, true)) => format!("PEER {address:#06X} UP"),
            Some((address, false)) => format!("PEER {address:#06X} DOWN"),
            None if self.zigbee_ready => "ZB READY NO DEV".to_string(),
            None => "ZB OFFLINE".to_string(),
        };
        let line2 = self.latest.clone().unwrap_or_else(|| "NO DATA".to_string());
        if let Err(err) = ui.draw_status_pair(&line1, &line2) {
            log::warn!("OLED: {err:#}");
        }
    }
}

/// Short rendering for the status page (5x7 font: ASCII, no unit glyph).
pub fn render_value(value: &Value) -> String {
    match value {
        Value::Bool(on) => if *on { "ON" } else { "OFF" }.to_string(),
        Value::Number(value) => format!("{value:.2}"),
        Value::Text(text) => (*text).to_string(),
    }
}

/// No wall clock exists yet (SNTP is not wired): the top bar's time slot
/// shows an honest uptime clock instead of a fake time.
fn uptime_clock() -> String {
    let uptime_ms = sys::system_info().map(|info| info.uptime_ms).unwrap_or(0);
    let total_seconds = uptime_ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    format!("{hours:02}:{minutes:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_render_for_the_status_page() {
        assert_eq!(render_value(&Value::Bool(true)), "ON");
        assert_eq!(render_value(&Value::Number(25.5)), "25.50");
        assert_eq!(render_value(&Value::Text("BOOT")), "BOOT");
    }
}
