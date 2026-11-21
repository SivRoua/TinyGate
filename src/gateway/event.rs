//! Facts the gateway publishes on its internal bus.
//!
//! An event is deliberately protocol-neutral: access services translate
//! Zigbee/BLE observations into this vocabulary, and consumers (UI today;
//! uplink and rules later) never see C types.
use crate::gateway::device::{Property, Value};

/// A radio observation worth showing, and later, uploading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GatewayEvent {
    /// Coordinator lifecycle; BLE joins this in its milestone.
    RadioZigbee { ready: bool },
    /// A peer crossed the presence line (join or 15 s silence).
    PeerPresence { address: u16, online: bool },
    /// Observed property value (`endpoint` is the protocol endpoint).
    Property {
        address: u16,
        endpoint: u8,
        property: Property,
        value: Value,
    },
}
