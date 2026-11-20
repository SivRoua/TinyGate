use anyhow::{bail, Result};
use log::{info, warn};
use std::time::{Duration, Instant};

use crate::drivers::{button, rgb};
use crate::hardware::Hardware;
use crate::platform;
use crate::radio::ble::{Event, Scanner};
use crate::radio::{zigbee, zigbee_check};
use crate::ui::Ui;
use crate::{idle_with_button, poll_button, update_status_page};

pub fn zigbee_demo(
    ui: &mut Option<Ui>,
    button: &mut Option<button::Button>,
    rgb: &mut Option<rgb::Rgb>,
) -> Result<()> {
    use esp_idf_svc::sys;
    let mut zb = zigbee::Coordinator::new()?;
    let started = Instant::now();
    let mut opened = false;
    let mut peer = 0xffff;
    let mut temp_ep = None;
    let mut onoff_ep = None;
    let mut temperature = None;
    let mut actual = None;
    let mut last_seen = Instant::now();
    let mut next_command = Instant::now() + Duration::from_secs(10);
    let mut next_display = Instant::now();
    let mut id = 0u32;
    let mut pending: Option<(zigbee_check::Pending, Instant)> = None;
    let mut completed = 0;
    let mut test_failed = false;
    update_status_page(ui, "ZIGBEE STARTING", "CHANNEL 15");
    loop {
        poll_button(button);
        let status = zb.status()?;
        if !status.ready && started.elapsed() > Duration::from_secs(30) {
            bail!("Zigbee startup timeout; last_error={}", status.last_error);
        }
        if status.ready && !opened {
            zb.permit_join(60)?;
            opened = true;
            info!(
                "Zigbee ready PAN={:04X} channel={}; joining open for 60s",
                status.pan_id, status.channel
            );
        }
        if let Some(e) = zb.poll()? {
            info!("ZB kind={} addr={:04X} ep={} cluster={:04X} attr={:04X} value={} status={} req={} tsn={} report={}",
                e.kind,e.short_address,e.endpoint,e.cluster,e.attribute,e.value,e.status,e.request_id,e.tsn,e.is_report);
            if u32::from(e.kind) == sys::TG_ZB_JOIN {
                info!("Peer IEEE (little endian)={:02X?}", e.ieee_address);
            }
            if e.short_address > 0
                && e.short_address < 0xfff8
                && matches!(u32::from(e.kind), sys::TG_ZB_JOIN | sys::TG_ZB_ATTRIBUTE)
            {
                if peer != e.short_address {
                    peer = e.short_address;
                    temp_ep = None;
                    onoff_ep = None;
                    temperature = None;
                    actual = None;
                    pending = None;
                    completed = 0;
                    test_failed = false;
                    zb.discover(peer)?;
                }
            }
            if e.short_address == peer {
                match u32::from(e.kind) {
                    sys::TG_ZB_ENDPOINT if e.profile == 0x0104 && e.status == 0 => {
                        if e.endpoint == 1 && e.cluster == 0x0402 && temp_ep.is_none() {
                            temp_ep = Some(e.endpoint);
                            id += 1;
                            zb.read(peer, e.endpoint, e.cluster, id)?;
                            id += 1;
                            zb.reporting(peer, e.endpoint, e.cluster, id)?;
                        }
                        if e.endpoint == 2 && e.cluster == 0x0006 && onoff_ep.is_none() {
                            onoff_ep = Some(e.endpoint);
                            id += 1;
                            zb.read(peer, e.endpoint, e.cluster, id)?;
                            id += 1;
                            zb.reporting(peer, e.endpoint, e.cluster, id)?;
                        }
                    }
                    sys::TG_ZB_ATTRIBUTE if e.status == 0 && e.attribute == 0 => {
                        last_seen = Instant::now();
                        if e.cluster == 0x0402 && e.endpoint == 1 {
                            temperature = Some(e.value);
                        }
                        if e.cluster == 0x0006 && e.endpoint == 2 {
                            actual = Some(e.value != 0);
                            if let Some((p, _)) = &mut pending {
                                p.observed(e.value != 0);
                            }
                        }
                    }
                    sys::TG_ZB_TX_CONFIRM if e.endpoint == 2 && e.cluster == 6 => {
                        if let Some((p, _)) = &mut pending {
                            p.tx(e.request_id, e.tsn, e.status == 0);
                        }
                    }
                    sys::TG_ZB_DEFAULT_RESPONSE if e.endpoint == 2 && e.cluster == 6 => {
                        if let Some((p, _)) = &mut pending {
                            p.response(e.tsn, e.value as u8, e.status == 0);
                        }
                    }
                    sys::TG_ZB_LEAVE => {
                        actual = None;
                        temperature = None;
                        temp_ep = None;
                        onoff_ep = None;
                        pending = None;
                    }
                    _ => {}
                }
            }
        }
        if let Some((p, time)) = &pending {
            if let Some(ok) = p.verdict() {
                info!(
                    "TEST request={} desired={} verified={}",
                    p.id, p.desired, ok
                );
                completed += 1;
                test_failed |= !ok;
                pending = None;
                next_command = Instant::now() + Duration::from_secs(10);
            } else if time.elapsed() > Duration::from_secs(12) {
                warn!("TEST request={} timeout: delivery/default response/state incomplete; auto-test stopped",p.id);
                test_failed = true;
                pending = None;
            }
        }
        let online = actual.is_some() && last_seen.elapsed() < Duration::from_secs(15);
        if opened
            && !test_failed
            && completed < 4
            && pending.is_none()
            && online
            && temp_ep.is_some()
            && onoff_ep.is_some()
            && Instant::now() >= next_command
        {
            id += 1;
            let desired = !actual.unwrap();
            match zb.onoff(peer, 2, desired, id) {
                Ok(()) => pending = Some((zigbee_check::Pending::new(id, desired), Instant::now())),
                Err(err) => {
                    warn!("Command submit failed: {err:#}");
                    test_failed = true;
                }
            }
        }
        if Instant::now() >= next_display {
            if let Some(led) = rgb {
                let color = if test_failed {
                    (255, 0, 0)
                } else if online {
                    (0, 255, 0)
                } else {
                    (0, 0, 255)
                };
                if let Err(err) = led.set(color.0, color.1, color.2, 24) {
                    warn!("RGB update: {err:#}");
                }
            }
            let text = temperature
                .map(|v| format!("TEMP {:.2} C", v as f32 / 100.0))
                .unwrap_or("WAIT SENSOR".into());
            let state = if !online {
                "NODE OFFLINE".into()
            } else {
                format!(
                    "SW {} TEST {}/4",
                    if actual == Some(true) { "ON" } else { "OFF" },
                    completed
                )
            };
            update_status_page(
                ui,
                &text,
                if test_failed { "TEST FAILED" } else { &state },
            );
            info!("ZB peer={peer:04X} online={online} dropped={} join_open={} tests={completed}/4 failed={test_failed}",status.dropped_events,status.join_open);
            if let Some(button) = button.as_ref() {
                let input = button.status()?;
                if input.dropped_events > 0 {
                    warn!("Button dropped events={}", input.dropped_events);
                }
            }
            next_display = Instant::now() + Duration::from_secs(1);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn ble_demo(button: &mut Option<button::Button>) -> Result<()> {
    info!("TinyGate C6 BLE scan example: C driver -> Rust events");
    // No external peripherals are initialized in this wireless-only example.
    let mut scanner = Scanner::new()?;
    let ready_deadline = Instant::now() + Duration::from_secs(10);
    while !scanner.status()?.ready {
        poll_button(button);
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
        poll_button(button);
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

pub fn wifi_demo(button: &mut Option<button::Button>) -> Result<()> {
    let mut wifi = platform::Wifi::new()?;
    wifi.scan()?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        poll_button(button);
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

/// Legacy no-feature bench: chip temperature, sequential radio scan demos,
/// then an uptime/heap dashboard loop until reset.
pub fn radio_bench(ui: &mut Option<Ui>, hw: &mut Hardware) -> Result<()> {
    let button = &mut hw.button;
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
    // Test radios sequentially; simultaneous coexistence is a separate
    // validation. Screen pages go through the UI status pair.
    update_status_page(ui, "WIFI SCANNING", "PLEASE WAIT");
    if let Err(err) = wifi_demo(button) {
        warn!("Wi-Fi demo: {err:#}");
    }
    update_status_page(ui, "BLE SCANNING", "15 SECONDS");
    if let Err(err) = ble_demo(button) {
        warn!("BLE demo: {err:#}");
    }
    if let Some(sensor) = &mut temperature {
        sensor.shutdown()?;
    }
    info!("Radio demos finished; OLED now shows uptime and heap. Reset to repeat scans.");
    loop {
        let system = platform::system_info()?;
        update_status_page(
            ui,
            &format!("UP {} S", system.uptime_ms / 1000),
            &format!("HEAP {} B", system.free_heap_bytes),
        );
        info!(
            "Uptime={} ms free_heap={}",
            system.uptime_ms, system.free_heap_bytes
        );
        idle_with_button(button, Duration::from_secs(10));
    }
}
