//! Firmware entry: start the ESP-IDF runtime, then hand over to the app.
use anyhow::Result;

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    tiny_gate::app::run()
}
