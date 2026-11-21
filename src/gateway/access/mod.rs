//! Device access: the protocols the gateway speaks to field devices.
//!
//! Each access service owns exactly one C radio driver for the whole
//! firmware run, serializes every call through one task, and publishes
//! typed observations on the gateway bus.
pub mod zigbee;
