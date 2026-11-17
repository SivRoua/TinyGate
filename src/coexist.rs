//! Finite, explicit Wi-Fi/Zigbee bench. BLE test orchestration is not integrated yet.
use crate::{
    coexist_stats::{Latency, Sequence},
    platform::Wifi,
    zigbee::Coordinator,
    zigbee_check::{Pending, Poll},
};
use anyhow::{bail, Result};
use esp_idf_svc::sys;
use log::{info, warn};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};
struct Traffic {
    socket: UdpSocket,
    remote: SocketAddr,
    hz: u16,
    next: Instant,
    pending: HashMap<u32, (Instant, u64)>,
}
const CONTROL_DEADLINE_MS: u64 = 2000;
#[derive(Default)]
struct Counters {
    udp_attempt: u64,
    udp_sent: u64,
    udp_recv: u64,
    udp_error: u64,
    udp_timeout: u64,
    unfinished: u64,
    ctrl_sent: u64,
    ctrl_ok: u64,
    ctrl_fail: u64,
    ctrl_timeout: u64,
    submit_error: u64,
    ctrl_late_ok: u64,
    ctrl_late_fail: u64,
    ctrl_drained: u64,
}
fn snapshot(
    id: u32,
    active: bool,
    s: &Sequence,
    c: &Counters,
    u: &Latency,
    z: &Latency,
    dropped: u32,
    elapsed: u64,
    heap: u32,
    wifi: Option<sys::tg_wifi_status_t>,
    ctrl_inflight: bool,
) {
    let wifi_ip = wifi.as_ref().map(|s| s.has_ip).unwrap_or(false);
    let wifi_reason = wifi.as_ref().map(|s| s.disconnect_reason).unwrap_or(0);
    let wifi_rssi = wifi.as_ref().map(|s| s.rssi).unwrap_or(0);
    info!("COEX {{\"run\":{id},\"active\":{active},\"elapsed_ms\":{elapsed},\"ble_tested\":false,\"sessions\":{},\"zb_unique\":{},\"zb_span\":{},\"zb_missing\":{},\"zb_duplicate\":{},\"zb_reordered\":{},\"zb_too_old\":{},\"queue_drop_total\":{dropped},\"heap\":{heap},\"wifi_has_ip\":{wifi_ip},\"wifi_reason\":{wifi_reason},\"wifi_rssi\":{wifi_rssi},\"udp_attempt\":{},\"udp_sent\":{},\"udp_recv\":{},\"udp_error\":{},\"udp_timeout\":{},\"unfinished\":{},\"ctrl_sent\":{},\"ctrl_ok\":{},\"ctrl_fail\":{},\"ctrl_timeout\":{},\"submit_error\":{},\"udp_hist\":{:?},\"udp_max_ms\":{},\"ctrl_hist\":{:?},\"ctrl_max_ms\":{},\"ctrl_late_ok\":{},\"ctrl_late_fail\":{},\"ctrl_drained\":{},\"ctrl_inflight\":{},\"ctrl_deadline_ms\":{}}}",
        s.sessions,s.unique,s.span,s.missing(),s.duplicates,s.reordered,s.too_old,c.udp_attempt,c.udp_sent,c.udp_recv,
        c.udp_error,c.udp_timeout,c.unfinished,c.ctrl_sent,c.ctrl_ok,c.ctrl_fail,c.ctrl_timeout,c.submit_error,
        u.buckets,u.max_ms,z.buckets,z.max_ms,c.ctrl_late_ok,c.ctrl_late_fail,c.ctrl_drained,ctrl_inflight,CONTROL_DEADLINE_MS);
}
pub fn run(
    display: &mut Option<crate::display::Display>,
    button: &mut Option<crate::button::Button>,
) -> Result<()> {
    let mut zb = Coordinator::new()?;
    let input = crate::connectivity::input()?;
    let mut wifi: Option<Wifi> = None;
    let mut wifi_scan_pending: Option<Instant> = None;
    let mut traffic: Option<Traffic> = None;
    let mut sequence = Sequence::default();
    let mut counters = Counters::default();
    let mut udp_latency = Latency::default();
    let mut ctrl_latency = Latency::default();
    let mut active = false;
    let mut run_id = 0u32;
    let mut start = Instant::now();
    let mut end = start;
    let mut elapsed_ms = 0;
    let mut seq = 0u32;
    let mut request = 0u32;
    let mut peer = 0xffff;
    let mut actual = None;
    let mut last_report = Instant::now();
    let mut pending: Option<(Pending, Instant)> = None;
    let mut control_blocked_logged = false;
    let mut control_secs = 10u64;
    let mut next_control = Instant::now();
    let mut opened = false;
    let mut ui = false;
    let mut next_stats = Instant::now();
    let boot = Instant::now();
    info!("COEX ready: help; BLE disabled; no Wi-Fi until explicitly requested");
    loop {
        crate::poll_button(button);
        let state = zb.status()?;
        if !state.ready && boot.elapsed() > Duration::from_secs(30) {
            bail!("Zigbee startup failed: {}", state.last_error);
        }
        if state.ready && !opened {
            zb.permit_join(60)?;
            opened = true;
        }
        if let Ok(line) = input.try_recv() {
            let outcome = (|| -> Result<()> {
                if let Some(text) = line.strip_prefix("wifi connect ") {
                    let (s, p) = crate::console_parse::credentials(text)?;
                    if wifi.is_none() {
                        wifi = Some(Wifi::new()?);
                    }
                    wifi.as_mut().unwrap().connect(s, p)?;
                    return Ok(());
                }
                let args: Vec<_> = line.split_whitespace().collect();
                match args.as_slice(){
                    ["help"]=>info!("COEX: start RUN_ID SECONDS(1..86400); stop; status; control SECONDS(0=off); ui on/off; wifi status/scan/disconnect/off; wifi connect SSID|PASS; udp IP PORT HZ(1..1000); udp off; join SECONDS"),
                    ["start",id,seconds]=>{
                        if active{bail!("Stop current run first");}
                        if pending.is_some(){bail!("Previous control still settling in SDK; wait for settlement or reboot");}
                        let seconds:u64=seconds.parse()?;if seconds==0||seconds>86400{bail!("Invalid duration");}
                        run_id=id.parse()?;sequence=Sequence::default();counters=Counters::default();udp_latency=Latency::default();ctrl_latency=Latency::default();
                        start=Instant::now();end=start+Duration::from_secs(seconds);active=true;seq=0;
                        next_control=start+Duration::from_secs(control_secs.max(1));
                        if let Some(t)=&mut traffic{t.pending.clear();t.next=start;}
                    },
                    ["stop"]=>{active=false;},
                    ["status"]=>{next_stats=Instant::now();},
                    ["control",seconds]=>{let n:u64=seconds.parse()?;if n>3600{bail!("Control interval <=3600");}control_secs=n;next_control=Instant::now()+Duration::from_secs(n.max(1));},
                    ["ui","on"]=>ui=true,["ui","off"]=>ui=false,
                    ["wifi","status"]=>{if let Some(w)=&wifi{let s=w.status()?;info!("WIFI_STATUS initialized={} connecting={} associated={} has_ip={} reason={} error={} scan_ready={} scan_total={} scan_stored={}",s.initialized,s.connecting,s.associated,s.has_ip,s.disconnect_reason,s.last_error,s.scan_ready,s.total_aps,s.stored_aps);}else{info!("WIFI_STATUS initialized=false");}},
                    ["wifi","scan"]=>{if wifi.is_none(){wifi=Some(Wifi::new()?);}wifi.as_mut().unwrap().scan()?;wifi_scan_pending=Some(Instant::now());},
                    ["wifi","disconnect"]=>{if let Some(w)=&mut wifi{w.disconnect()?;}},
                    ["wifi","off"]=>{if let Some(w)=&mut wifi{w.shutdown()?;}wifi=None;wifi_scan_pending=None;if let Some(t)=&traffic{counters.unfinished+=t.pending.len()as u64;}traffic=None;},
                    ["udp","off"]=>{if let Some(t)=&traffic{counters.unfinished+=t.pending.len() as u64;}traffic=None;},
                    ["udp",ip,port,hz]=>{
                        if active{bail!("Configure UDP before starting run");}
                        if wifi.is_none(){bail!("Initialize Wi-Fi first");}
                        let ip:Ipv4Addr=ip.parse()?;let port:u16=port.parse()?;let hz:u16=hz.parse()?;
                        if !((ip.is_private()||ip.is_loopback()||ip.is_link_local())&&port!=0&&hz>0&&hz<=1000){bail!("Use LAN address, nonzero port, rate 1..1000");}
                        let socket=UdpSocket::bind("0.0.0.0:0")?;socket.set_nonblocking(true)?;
                        traffic=Some(Traffic{socket,remote:SocketAddr::from((ip,port)),hz,next:Instant::now(),pending:HashMap::new()});
                    },
                    ["join",seconds]=>zb.permit_join(seconds.parse()?)?,
                    _=>bail!("Unknown command; help"),
                }
                Ok(())
            })();
            if let Err(err) = outcome {
                warn!("COEX command rejected: {err:#}");
            }
        }
        if let (Some(started), Some(w)) = (wifi_scan_pending, &wifi) {
            let status = w.status()?;
            if status.scan_ready {
                info!(
                    "WIFI_SCAN complete error={} total={} stored={}",
                    status.last_error, status.total_aps, status.stored_aps
                );
                for index in 0..status.stored_aps {
                    let ap = w.ap(index)?;
                    info!(
                        "WIFI_AP ssid={:?} channel={} rssi={} auth={}",
                        String::from_utf8_lossy(&ap.ssid[..usize::from(ap.ssid_len)]),
                        ap.channel,
                        ap.rssi,
                        ap.auth_mode
                    );
                }
                wifi_scan_pending = None;
            } else if started.elapsed() >= Duration::from_secs(15) {
                warn!("WIFI_SCAN timeout; wait for completion or use wifi off before retrying");
                wifi_scan_pending = None;
            }
        }
        for _ in 0..32 {
            let Some(e) = zb.poll()? else {
                break;
            };
            if (active || pending.is_some())
                && (e.cluster == 6
                    || u32::from(e.kind) == sys::TG_ZB_TX_CONFIRM
                    || u32::from(e.kind) == sys::TG_ZB_ERROR)
            {
                info!("CTRL_EVT run={} ms={} kind={} addr={:04x} ep={} req={} tsn={} status={} value={} report={} pending={}",
                    run_id, start.elapsed().as_millis(), e.kind, e.short_address,
                    e.endpoint, e.request_id, e.tsn, e.status, e.value, e.is_report,
                    pending.as_ref().map(|(p, _)| p.id).unwrap_or(0));
            }
            if u32::from(e.kind) == sys::TG_ZB_ATTRIBUTE && e.status == 0 {
                if e.endpoint == 1 && e.cluster == 0x0402 && e.attribute == 0xf000 && e.is_report {
                    if active {
                        sequence.add((e.test_payload >> 32) as u32, e.test_payload as u32);
                    }
                }
                if e.endpoint == 2 && e.cluster == 6 && e.attribute == 0 {
                    if peer != e.short_address {
                        if let Some((p, _)) = &mut pending {
                            if p.cancel() {
                                counters.unfinished += 1;
                            }
                        }
                    }
                    peer = e.short_address;
                    actual = Some(e.value != 0);
                    last_report = Instant::now();
                    if let Some((p, _)) = &mut pending {
                        p.observed(e.value != 0);
                    }
                }
            }
            if e.endpoint == 2 && e.cluster == 6 {
                if let Some((p, _)) = &mut pending {
                    match u32::from(e.kind) {
                        sys::TG_ZB_TX_CONFIRM => p.tx(e.request_id, e.tsn, e.status == 0),
                        sys::TG_ZB_DEFAULT_RESPONSE if e.short_address == peer => {
                            p.response(e.tsn, e.value as u8, e.status == 0)
                        }
                        _ => {}
                    }
                }
            }
        }
        if active {
            elapsed_ms = start.elapsed().as_millis() as u64;
        }
        if active && Instant::now() >= end {
            active = false;
        }
        if !active {
            if let Some(t) = &mut traffic {
                counters.unfinished += t.pending.len() as u64;
                t.pending.clear();
            }
            if let Some((p, _)) = &mut pending {
                if p.cancel() {
                    counters.unfinished += 1;
                }
            }
        }
        if let Some((p, sent)) = &mut pending {
            let age = sent.elapsed().as_millis() as u64;
            let outcome = p.poll(age, CONTROL_DEADLINE_MS);
            if outcome != Poll::Waiting {
                info!(
                    "CTRL_RESULT run={} ms={} outcome={outcome:?} age_ms={} state={p:?}",
                    run_id,
                    start.elapsed().as_millis(),
                    age
                );
            }
            let settled = match outcome {
                Poll::Complete(ok) => {
                    if ok {
                        counters.ctrl_ok += 1;
                        ctrl_latency.add(age);
                    } else {
                        counters.ctrl_fail += 1;
                    }
                    true
                }
                Poll::Timeout => {
                    counters.ctrl_timeout += 1;
                    false
                }
                Poll::Late(ok) => {
                    if ok {
                        counters.ctrl_late_ok += 1;
                    } else {
                        counters.ctrl_late_fail += 1;
                    }
                    true
                }
                Poll::Drained => {
                    counters.ctrl_drained += 1;
                    true
                }
                Poll::Cancelled => true,
                Poll::Waiting => false,
            };
            if settled {
                pending = None;
                control_blocked_logged = false;
            } else if age >= 10000 && !control_blocked_logged {
                warn!("CTRL_BLOCKED run={} req={} SDK request unresolved; no further control until settlement",run_id,p.id);
                control_blocked_logged = true;
            }
        }
        if active
            && control_secs > 0
            && pending.is_none()
            && actual.is_some()
            && last_report.elapsed() < Duration::from_secs(15)
            && Instant::now() >= next_control
        {
            request = request.wrapping_add(1).max(1);
            let desired = !actual.unwrap();
            match zb.onoff(peer, 2, desired, request) {
                Ok(()) => {
                    info!(
                        "CTRL_SEND run={} ms={} req={} addr={:04x} desired={}",
                        run_id,
                        start.elapsed().as_millis(),
                        request,
                        peer,
                        desired
                    );
                    counters.ctrl_sent += 1;
                    pending = Some((Pending::new(request, desired), Instant::now()));
                }
                Err(err) => {
                    counters.submit_error += 1;
                    warn!(
                        "CTRL_SUBMIT_ERROR run={} req={} error={err:#}",
                        run_id, request
                    );
                }
            }
            next_control = Instant::now() + Duration::from_secs(control_secs);
        }
        if let Some(t) = &mut traffic {
            let now = Instant::now();
            if active && now >= t.next {
                counters.udp_attempt += 1;
                seq = seq.wrapping_add(1);
                let stamp = start.elapsed().as_millis() as u64;
                let mut data = [0xa5; 256];
                data[..4].copy_from_slice(b"TGW1");
                data[4..8].copy_from_slice(&run_id.to_le_bytes());
                data[8..12].copy_from_slice(&seq.to_le_bytes());
                data[12..20].copy_from_slice(&stamp.to_le_bytes());
                if t.pending.len() >= 256 {
                    counters.udp_error += 1;
                } else {
                    match t.socket.send_to(&data, t.remote) {
                        Ok(256) => {
                            t.pending.insert(seq, (now, stamp));
                            counters.udp_sent += 1;
                        }
                        _ => counters.udp_error += 1,
                    }
                }
                t.next = now + Duration::from_micros(1_000_000 / u64::from(t.hz));
            }
            for _ in 0..32 {
                let mut data = [0; 257];
                match t.socket.recv_from(&mut data) {
                    Ok((256, source))
                        if source == t.remote
                            && &data[..4] == b"TGW1"
                            && u32::from_le_bytes(data[4..8].try_into().unwrap()) == run_id
                            && data[20..256].iter().all(|&b| b == 0xa5) =>
                    {
                        let seq = u32::from_le_bytes(data[8..12].try_into().unwrap());
                        if let Some(&(sent, stamp)) = t.pending.get(&seq) {
                            if data[12..20] == stamp.to_le_bytes() {
                                udp_latency.add(sent.elapsed().as_millis() as u64);
                                counters.udp_recv += 1;
                                t.pending.remove(&seq);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
            let expired: Vec<_> = t
                .pending
                .iter()
                .filter(|(_, (time, _))| time.elapsed() >= Duration::from_secs(2))
                .map(|(&seq, _)| seq)
                .collect();
            for seq in expired {
                t.pending.remove(&seq);
                counters.udp_timeout += 1;
            }
        }
        if Instant::now() >= next_stats {
            let system = crate::platform::system_info()?;
            snapshot(
                run_id,
                active,
                &sequence,
                &counters,
                &udp_latency,
                &ctrl_latency,
                state.dropped_events,
                elapsed_ms,
                system.free_heap_bytes,
                wifi.as_ref().map(|w| w.status()).transpose()?,
                pending.is_some(),
            );
            if ui {
                crate::update_oled(
                    display,
                    &format!("RUN {} {}", run_id, active),
                    &format!("RX {} GAP {}", sequence.unique, sequence.missing()),
                );
            }
            next_stats = Instant::now() + Duration::from_secs(1);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
