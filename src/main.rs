mod ble;
mod ble_ad;
#[allow(dead_code)]
mod button;
#[allow(dead_code)]
mod c_drivers;
mod display;
mod platform;
#[allow(dead_code)]
mod rgb;
#[allow(dead_code)]
mod sdcard;
#[allow(dead_code)]
mod zigbee;

use anyhow::{bail, Result};
use ble::{Event, Scanner};
use log::{info, warn};
use std::time::{Duration, Instant};

fn update_oled(display: &mut Option<display::Display>, line1: &str, line2: &str) {
    if let Some(screen) = display {
        if let Err(err) = screen.show_status("TinyGate", line1, line2) {
            warn!("OLED update: {err:#}");
        }
    }
}

fn ble_demo() -> Result<()> {
    info!("TinyGate C6 BLE scan example: C driver -> Rust events");
    // No external peripherals are initialized in this wireless-only example.
    let mut scanner = Scanner::new()?;
    let ready_deadline = Instant::now() + Duration::from_secs(10);
    while !scanner.status()?.ready {
        if Instant::now() >= ready_deadline {
            bail!("BLE host sync timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    scanner.start(15_000)?;
    info!("Scanning for 15 seconds (active scan, duplicate filter enabled)");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut reports = 0_u32;
    loop {
        match scanner.poll()? {
            Some(Event::Report(report)) => {
                reports += 1;
                info!(
                    "{} type={} RSSI={} dBm response={} name={:?} data={:02X?}",
                    report.address_string(),
                    report.address_type,
                    report.rssi,
                    report.scan_response,
                    report.name(),
                    report.data
                );
            }
            Some(Event::Complete(reason)) => {
                info!("Scan complete: native reason={reason}");
                break;
            }
            Some(Event::Reset(reason)) => bail!("BLE host reset: {reason}"),
            None => std::thread::sleep(Duration::from_millis(10)),
        }
        if Instant::now() >= deadline {
            scanner.stop()?;
            bail!("Scan completion timed out");
        }
    }
    let status = scanner.status()?;
    info!(
        "Reports={reports} dropped={} host_error={}",
        status.dropped_reports, status.last_host_error
    );
    scanner.shutdown()?;
    info!("BLE stopped and deinitialized.");
    Ok(())
}

fn wifi_demo() -> Result<()> {
    let mut wifi = platform::Wifi::new()?;
    wifi.scan()?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let status = wifi.status()?;
        if status.scan_ready {
            if status.last_error != 0 {
                bail!("Wi-Fi scan failed: {}", status.last_error);
            }
            info!(
                "Wi-Fi found {} APs; retaining {}",
                status.total_aps, status.stored_aps
            );
            for index in 0..status.stored_aps {
                let ap = wifi.ap(index)?;
                info!(
                    "AP {:?} channel={} RSSI={} auth={}",
                    String::from_utf8_lossy(&ap.ssid[..usize::from(ap.ssid_len)]),
                    ap.channel,
                    ap.rssi,
                    ap.auth_mode
                );
            }
            break;
        }
        if Instant::now() >= deadline {
            bail!("Wi-Fi scan timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    // No credentials are embedded; connect()/disconnect() are available to the core.
    wifi.shutdown()?;
    Ok(())
}

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let mut display = match display::Display::new() {
        Ok(screen) => {
            info!(
                "SSD1306 ready: SDA=20 SCL=19 address=0x{:02X}",
                screen.address()?
            );
            Some(screen)
        }
        Err(err) => {
            warn!("OLED unavailable (SDA20/SCL19): {err:#}");
            None
        }
    };
    update_oled(&mut display, "BOOT OK", "SDA20 SCL19");
    let system = platform::system_info()?;
    info!(
        "Chip model={} revision={} cores={} reset={} heap={} min_heap={} largest={}",
        system.chip_model,
        system.revision,
        system.cores,
        system.reset_reason,
        system.free_heap_bytes,
        system.minimum_free_heap_bytes,
        system.largest_free_block_bytes
    );
    let mut temperature = match platform::Temperature::new() {
        Ok(sensor) => Some(sensor),
        Err(err) => {
            warn!("Temperature unavailable: {err:#}");
            None
        }
    };
    if let Some(sensor) = &temperature {
        match sensor.read() {
            Ok(value) => info!("Chip temperature: {value:.1} C (not ambient)"),
            Err(err) => warn!("Temperature read: {err:#}"),
        }
    }
    // Test radios sequentially; simultaneous coexistence is a separate validation.
    update_oled(&mut display, "WIFI SCANNING", "PLEASE WAIT");
    if let Err(err) = wifi_demo() {
        warn!("Wi-Fi demo: {err:#}");
    }
    update_oled(&mut display, "BLE SCANNING", "15 SECONDS");
    if let Err(err) = ble_demo() {
        warn!("BLE demo: {err:#}");
    }
    if let Some(sensor) = &mut temperature {
        sensor.shutdown()?;
    }
    info!("Radio demos finished; OLED now shows uptime and heap. Reset to repeat scans.");
    loop {
        let system = platform::system_info()?;
        update_oled(
            &mut display,
            &format!("UP {} S", system.uptime_ms / 1000),
            &format!("HEAP {} B", system.free_heap_bytes),
        );
        info!(
            "Uptime={} ms free_heap={}",
            system.uptime_ms, system.free_heap_bytes
        );
        std::thread::sleep(Duration::from_secs(10));
    }
}
