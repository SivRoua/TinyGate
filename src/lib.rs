//! TinyGate gateway.
//!
//! Product-oriented layering:
//! - `app/` is the composition root: board bring-up and executor assembly.
//! - `drivers/` are thin, typed adapters over the `tg_*` C components; they
//!   own hardware, never product decisions.
//! - `gateway/` is the product: device model, identity, access services,
//!   and (in later milestones) uplink, rules and offline storage.
//! - `ui/` is the only screen writer.
//!
//! Benches and validation apps live on the `demo` branch.
pub mod app;
pub mod drivers;
pub mod gateway;
pub mod ui;
