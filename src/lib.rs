//! TinyGate gateway.
//!
//! Layering: C components (`components/`) own every hardware read/write;
//! `drivers/`, `radio/`, `platform/` are thin FFI facades over their public
//! functions; `gateway/`, `ui/` and the executor assembly are Rust business
//! logic. Benches and validation apps live on the `demo` branch.
pub mod drivers;
pub mod gateway;
pub mod hardware;
pub mod platform;
pub mod radio;
pub mod runtime;
pub mod ui;
