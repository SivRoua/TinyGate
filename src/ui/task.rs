//! The UI task: sole screen writer, on its own low-priority executor thread.
//!
//! DESIGN.md mandates a low-priority background task driving the OLED so
//! I2C traffic never stalls the radio services. Everything it shows comes
//! from the bus; it holds no other capability.
use crate::drivers::sys;
use crate::gateway::bus::BusReceiver;
use crate::gateway::device::{GatewayEvent, Property};
use crate::ui::Ui;
use embassy_time::{Duration, Ticker};
use log::warn;

/// Drop this thread below the main executor's task so display work yields
/// to radio timing. FreeRTOS idle is 0; the main task starts at 1.
fn lower_own_priority() {
    use esp_idf_svc::sys;
    unsafe {
        let handle = sys::xTaskGetCurrentTaskHandle();
        sys::vTaskPrioritySet(handle, 1);
    }
}

#[derive(Default)]
struct ViewState {
    zigbee_ready: bool,
    peer: Option<(u16, bool)>,
    latest: Option<String>,
}

impl ViewState {
    fn apply(&mut self, event: GatewayEvent) {
        match event {
            GatewayEvent::RadioZigbee { ready } => self.zigbee_ready = ready,
            GatewayEvent::PeerPresence { address, online } => self.peer = Some((address, online)),
            GatewayEvent::Property { property, value, .. } => {
                // Short 5x7-font labels; "TEMP 25.50 C" / "PWR ON".
                let (label, unit) = match property {
                    Property::Temperature => ("TEMP", " C"),
                    Property::Power => ("PWR", ""),
                };
                self.latest = Some(format!("{label} {}{unit}", value.display()));
            }
        }
    }

    fn render(&self, ui: &mut Ui) {
        let _ = ui.update_and_fmt_top_bar(self.zigbee_ready, false, false, &uptime_clock());
        let line1 = match self.peer {
            Some((address, true)) => format!("PEER {address:#06X} UP"),
            Some((address, false)) => format!("PEER {address:#06X} DOWN"),
            None if self.zigbee_ready => "ZB READY NO DEV".to_string(),
            None => "ZB OFFLINE".to_string(),
        };
        let line2 = self
            .latest
            .clone()
            .unwrap_or_else(|| "NO DATA".to_string());
        if let Err(err) = ui.update_status_pair(&line1, &line2) {
            warn!("OLED: {err:#}");
        }
    }
}

/// No wall clock exists yet (SNTP is not wired): the top bar's time slot
/// shows an honest uptime clock instead of a fake time.
fn uptime_clock() -> String {
    let uptime_ms = sys::system_info()
        .map(|info| info.uptime_ms)
        .unwrap_or(0);
    let total_seconds = uptime_ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    format!("{hours:02}:{minutes:02}")
}

#[embassy_executor::task]
pub async fn ui_task(mut ui: Ui, bus: BusReceiver<'static>) {
    lower_own_priority();
    let mut state = ViewState::default();
    state.render(&mut ui); // boot frame while services come up

    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
        while let Ok(event) = bus.try_receive() {
            state.apply(event);
        }
        state.render(&mut ui);
        ticker.next().await;
    }
}
