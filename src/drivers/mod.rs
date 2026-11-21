//! Hardware adapters: thin, typed facades over the `tg_*` C components.
//!
//! Grouped by where the hardware sits, not by product capability: `board/`
//! on-board peripherals, `radio/` wireless protocols, `net/` Wi-Fi, `sys/`
//! chip services. Product flows are assembled in `gateway/`.
pub mod board;
pub mod net;
pub mod radio;
pub mod sys;

pub(crate) mod lease;
