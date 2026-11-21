//! Firmware assembly: the composition root.
//!
//! This is the only module that decides which capability runs on which
//! executor. Everything it assembles comes from two sides: product
//! capabilities in `gateway/` and their hardware adapters in `drivers/`.
use crate::gateway::{self, bus, identity};
use crate::ui::{self, Ui};
use anyhow::Result;
use log::info;

pub mod board;
mod runtime;

/// Assemble the gateway and run the dual-executor loop. Never returns while
/// the firmware is healthy.
pub fn run() -> Result<()> {
    // Board assembly: each probe degrades individually and never blocks.
    let hw = board::Hardware::probe();
    hw.log_system()?;
    let _ = hw; // lives for the whole firmware run; the loop below never exits

    // Connection identity per the contracts: persistent boot counter,
    // boot-scoped boot_id, climbing seq.
    let ident = identity::Identity::load();
    info!(
        "Identity: gateway_id={} boot_id={} boot_counter={}",
        ident.gateway_id(),
        ident.boot_id,
        ident.boot_counter
    );
    let _ = ident; // consumed by the uplink milestone

    // The screen runs on its own low-priority executor thread (DESIGN.md:
    // never block radio timing); it receives everything over the bus.
    // Executors are firmware-lifetime objects: leaked on purpose.
    let screen = Ui::new();
    std::thread::Builder::new()
        .name("ui".into())
        .stack_size(12 * 1024)
        .spawn(move || {
            let Some(screen) = screen else {
                return; // no OLED probed: keep running degraded
            };
            let executor = Box::leak(Box::new(embassy_executor::Executor::new()));
            executor.run(|spawner| {
                let token = ui::task::ui_task(screen, bus::receiver())
                    .expect("spawn UI task");
                spawner.spawn(token);
            });
        })
        .map_err(|err| anyhow::anyhow!("spawn UI thread: {err}"))?;

    // Main executor: radio access services, timers and later the uplink.
    let executor = Box::leak(Box::new(embassy_executor::Executor::new()));
    executor.run(|spawner| {
        let token = gateway::access::zigbee::coordinator_service(bus::sender())
            .expect("spawn Zigbee service");
        spawner.spawn(token);
    })
}
