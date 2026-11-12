mod c_drivers;

use anyhow::Result;
use log::info;
use c_drivers::CDrivers;

fn main() -> Result<()> {
    // Essential for linking ESP-IDF patches
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to ESP-IDF logger
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("================================================");
    info!(" TinyGate Gateway Booting (Rust Base Architecture) ");
    info!(" Target: ESP32-C6 (RISC-V) | Framework: ESP-IDF ");
    info!("================================================");

    // 1. Initialize C Driver Subsystem (UPS, OLED, SDCard, Zigbee, BLE)
    info!("Initializing C Driver subsystem...");
    CDrivers::init_all()?;

    // 2. Query UPS battery status
    let vbat = CDrivers::battery_voltage();
    let power_loss = CDrivers::is_power_loss();
    info!("UPS Status: Voltage = {:.2}V, Mains Power Loss = {}", vbat, power_loss);

    // 3. Update OLED Display
    CDrivers::display_status("TinyGate v0.1", "Status: Ready", "Net: Initializing")?;

    info!("TinyGate gateway base initialization complete.");

    // Gateway heartbeat loop
    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        info!(
            "Heartbeat: Battery = {:.2}V | SD Mounted = {}",
            CDrivers::battery_voltage(),
            CDrivers::is_sd_mounted()
        );
    }
}
