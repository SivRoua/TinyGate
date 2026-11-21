//! Chip-level services: no board wiring, no protocol stack.
pub mod system;
pub mod temperature;

pub use system::system_info;
pub use temperature::Temperature;
