//! The service bus: one static embassy channel for the whole firmware.
//!
//! Services publish with `try_send` (never blocking the radio loop); the UI
//! task drains with `try_recv` on its own tick. Capacity 32 matches the C
//! event queues; an overflow is dropped and counted by embassy, visible
//! as `send` failures in service logs.
use crate::gateway::device::GatewayEvent;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};

/// Mirrors the C components' queue depth (32 events).
pub const CAPACITY: usize = 32;

pub type GatewayBus = Channel<CriticalSectionRawMutex, GatewayEvent, CAPACITY>;
pub type BusSender<'a> = Sender<'a, CriticalSectionRawMutex, GatewayEvent, CAPACITY>;
pub type BusReceiver<'a> = Receiver<'a, CriticalSectionRawMutex, GatewayEvent, CAPACITY>;

/// The one bus instance; static so every task can hold a typed handle.
pub static BUS: GatewayBus = GatewayBus::new();

pub fn sender() -> BusSender<'static> {
    BUS.sender()
}

pub fn receiver() -> BusReceiver<'static> {
    BUS.receiver()
}
