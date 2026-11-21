//! The UI task: sole screen writer, on its own low-priority executor thread.
//!
//! DESIGN.md mandates a low-priority background task driving the OLED so
//! I2C traffic never stalls the radio services. Everything it shows comes
//! from the bus; it holds no other capability. Event-to-text mapping lives
//! in [`crate::ui::view`].
use crate::gateway::bus::BusReceiver;
use crate::ui::Ui;
use crate::ui::view::ViewState;
use embassy_time::{Duration, Ticker};

/// Drop this thread below the main executor's task so display work yields
/// to radio timing. FreeRTOS idle is 0; the main task starts at 1.
fn lower_own_priority() {
    use esp_idf_svc::sys;
    unsafe {
        let handle = sys::xTaskGetCurrentTaskHandle();
        sys::vTaskPrioritySet(handle, 1);
    }
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
