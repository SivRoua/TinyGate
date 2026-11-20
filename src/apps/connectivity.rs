//! Explicit serial command bench. Never auto-connects or persists secrets.
use crate::apps::console_parse as parse;
use crate::drivers::rgb::Rgb;
use crate::platform::Wifi;
use crate::radio::ble::{Event, Scanner};
use anyhow::{bail, Result};
use log::{info, warn};
use std::{
    io::Read,
    sync::mpsc,
    time::{Duration, Instant},
};
fn command(
    line: &str,
    wifi: &mut Wifi,
    ble: &mut Scanner,
    rgb: &mut Option<Rgb>,
    id: u32,
) -> Result<()> {
    if let Some(text) = line.strip_prefix("wifi connect ") {
        let (s, p) = parse::credentials(text)?;
        return wifi.connect(s, p);
    }
    if let Some(text) = line.strip_prefix("wifi save ") {
        let (s, p) = parse::credentials(text)?;
        return wifi.save_credentials(s, p);
    }
    if let Some(text) = line.strip_prefix("wifi ap ") {
        let (s, p) = parse::credentials(text)?;
        return wifi.start_ap(s, p, 1);
    }
    let args: Vec<_> = line.split_whitespace().collect();
    let num = |i: usize| -> Result<u16> {
        parse::number(
            args.get(i)
                .ok_or_else(|| anyhow::anyhow!("Missing argument"))?,
        )
    };
    match args.as_slice() {
        ["help"]=>info!("Commands: wifi scan/connect SSID|PASS/save SSID|PASS/saved/forget/disconnect/ap SSID|PASS/ap-off; ble scan SECONDS/connect TYPE ADDRESS/cancel/disconnect CONN/services CONN/chars CONN START END/descs CONN START END/read CONN HANDLE/write CONN HANDLE HEX/subscribe CONN CCCD MODE; rgb R G B BRIGHTNESS"),
        ["wifi","status"]=>{let s=wifi.status()?;info!("WiFi associated={} IP={:?} has_ip={} AP={} clients={} reason={}",s.associated,s.ipv4,s.has_ip,s.ap_active,s.ap_clients,s.disconnect_reason);},
        ["wifi","scan"]=>wifi.scan()?,
        ["wifi","saved"]=>{let(s,p)=wifi.load_credentials()?;wifi.connect(&s,&p)?;},
        ["wifi","forget"]=>wifi.forget_credentials()?,
        ["wifi","disconnect"]=>wifi.disconnect()?,
        ["wifi","ap-off"]=>wifi.stop_ap()?,
        ["ble","scan",_]=>ble.start(u32::from(num(2)?).checked_mul(1000).unwrap())?,
        ["ble","connect",_,address]=>ble.connect(parse::address(address)?,u8::try_from(num(2)?)?)?,
        ["ble","status"]=>{let s=ble.links()?;info!("BLE links={} handles={:?} connecting={} dropped={}",s.links,s.handles,s.connecting,s.dropped_events);},
        ["ble","cancel"]=>ble.cancel_connect()?,
        ["ble","disconnect",_]=>ble.disconnect(num(2)?)?,
        ["ble","services",_]=>ble.services(num(2)?,id)?,
        ["ble","chars",_,_,_]=>ble.characteristics(num(2)?,num(3)?,num(4)?,id)?,
        ["ble","descs",_,_,_]=>ble.descriptors(num(2)?,num(3)?,num(4)?,id)?,
        ["ble","read",_,_]=>ble.read(num(2)?,num(3)?,id)?,
        ["ble","write",_,_,hex]=>ble.write(num(2)?,num(3)?,&parse::bytes(hex)?,id)?,
        ["ble","subscribe",_,_,_]=>ble.subscribe(num(2)?,num(3)?,u8::try_from(num(4)?)?,id)?,
        ["rgb",_,_,_,_]=>{let led=rgb.as_mut().ok_or_else(||anyhow::anyhow!("RGB unavailable"))?;
            led.set(u8::try_from(num(1)?)?,u8::try_from(num(2)?)?,u8::try_from(num(3)?)?,u8::try_from(num(4)?)?)?;},
        _=>bail!("Unknown command; enter help"),
    }
    Ok(())
}
pub(crate) fn input() -> Result<mpsc::Receiver<String>> {
    let (tx, rx) = mpsc::sync_channel::<String>(8);
    std::thread::Builder::new()
        .name("serial_input".into())
        .stack_size(4096)
        .spawn(move || {
            let mut input = std::io::stdin();
            let mut byte = [0];
            let mut line = Vec::new();
            let mut overflow = false;
            loop {
                match input.read(&mut byte) {
                    Ok(1) if byte[0] == b'\n' || byte[0] == b'\r' => {
                        if !overflow && !line.is_empty() {
                            if let Ok(text) = String::from_utf8(std::mem::take(&mut line)) {
                                if tx.send(text).is_err() {
                                    break;
                                }
                            }
                        }
                        line.clear();
                        overflow = false;
                    }
                    Ok(1) => {
                        if line.len() < 256 {
                            line.push(byte[0]);
                        } else {
                            overflow = true;
                        }
                    }
                    _ => std::thread::sleep(Duration::from_millis(20)),
                }
            }
        })?;
    Ok(rx)
}
pub fn run(
    rgb: &mut Option<Rgb>,
    button: &mut Option<crate::drivers::button::Button>,
) -> Result<()> {
    let mut wifi = Wifi::new()?;
    let mut ble = Scanner::new()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ble.status()?.ready {
        if Instant::now() > deadline {
            bail!("BLE sync timeout");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let rx = input()?;
    info!("Connectivity command bench ready; enter help. Zigbee is not started.");
    let mut id = 0u32;
    let mut generation = 0;
    let mut scan_reported = false;
    loop {
        crate::poll_button(button);
        if let Ok(line) = rx.try_recv() {
            id = id.wrapping_add(1).max(1);
            match command(&line, &mut wifi, &mut ble, rgb, id) {
                Ok(()) => info!("Command accepted id={id}"),
                Err(err) => warn!("Command rejected: {err:#}"),
            }
        }
        let status = wifi.status()?;
        if status.generation != generation {
            generation = status.generation;
            info!(
                "WiFi associated={} IP={:?} has_ip={} reason={} ap={} clients={}",
                status.associated,
                status.ipv4,
                status.has_ip,
                status.disconnect_reason,
                status.ap_active,
                status.ap_clients
            );
        }
        if !status.scan_ready {
            scan_reported = false;
        }
        if status.scan_ready && !scan_reported {
            scan_reported = true;
            info!(
                "WiFi scan status={} found={} kept={}",
                status.last_error, status.total_aps, status.stored_aps
            );
            for i in 0..status.stored_aps {
                let ap = wifi.ap(i)?;
                info!(
                    "AP {:?} RSSI={} channel={}",
                    String::from_utf8_lossy(&ap.ssid[..usize::from(ap.ssid_len)]),
                    ap.rssi,
                    ap.channel
                );
            }
        }
        if let Some(event) = ble.poll()? {
            match event {
                Event::Report(r) => info!(
                    "BLE {} type={} RSSI={} name={:?}",
                    r.address_string(),
                    r.address_type,
                    r.rssi,
                    r.name()
                ),
                Event::Complete(reason) => info!("BLE scan complete reason={reason}"),
                Event::Reset(reason) => warn!("BLE reset={reason}; invalidate handles"),
            }
        }
        if let Some(event) = ble.poll_gatt()? {
            let uuid: Vec<u8> = event
                .uuid
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as u8)
                .collect();
            info!("GATT kind={} conn={} handle={} end={} props={} uuid={} req={} status={} truncated={} data={:02X?}",event.kind,event.connection,event.handle,event.end_handle,event.properties,String::from_utf8_lossy(&uuid),event.request_id,event.status,event.truncated,&event.data[..usize::from(event.data_len).min(128)]);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
