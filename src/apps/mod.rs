//! Validation benches and demos, driven over the serial console.
pub mod coexist;
pub mod coexist_stats;
pub mod connectivity;
pub mod console_parse;
pub mod demo;

/// The app selected at compile time via Cargo features. Exactly one runs.
pub enum App {
    /// `coexist-test`: bounded Wi-Fi/Zigbee radio bench.
    CoexistBench,
    /// `connectivity-demo`: serial console command bench.
    ConnectivityBench,
    /// `zigbee-demo` (default): coordinator bring-up and self-test.
    ZigbeeDemo,
    /// `--no-default-features`: temperature read, radio scan demos, uptime loop.
    RadioBench,
}

/// Resolve the compile-time feature selection into an explicit app choice.
pub fn select() -> App {
    if cfg!(feature = "coexist-test") {
        App::CoexistBench
    } else if cfg!(feature = "connectivity-demo") {
        App::ConnectivityBench
    } else if cfg!(feature = "zigbee-demo") {
        App::ZigbeeDemo
    } else {
        App::RadioBench
    }
}
