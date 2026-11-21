//! Safe single-owner facade over the C BLE stack. Bindings come from tg_ble.h.
use crate::drivers::lease::{check, Lease};
use anyhow::{bail, Result};
use esp_idf_svc::sys;
use std::sync::atomic::AtomicBool;

static OWNED: AtomicBool = AtomicBool::new(false);

pub struct Scanner {
    lease: Lease,
}
pub enum Event {
    Report(Report),
    Complete(i32),
    Reset(i32),
}
pub struct Report {
    pub address: [u8; 6],
    pub address_type: u8,
    pub rssi: i8,
    pub scan_response: bool,
    pub data: Vec<u8>,
}
impl Report {
    pub fn address_string(&self) -> String {
        self.address
            .iter()
            .rev()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }
    pub fn name(&self) -> Option<String> {
        crate::drivers::radio::ble_ad::local_name(&self.data)
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }
}
#[allow(dead_code)]
impl Scanner {
    pub fn connect(&mut self, address: [u8; 6], address_type: u8) -> Result<()> {
        self.ensure_active()?;
        check(
            unsafe { sys::tg_ble_connect(address.as_ptr(), address_type) },
            "BLE connect",
        )
    }
    pub fn cancel_connect(&mut self) -> Result<()> {
        self.ensure_active()?;
        check(
            unsafe { sys::tg_ble_connect_cancel() },
            "BLE connect cancel",
        )
    }
    pub fn disconnect(&mut self, conn: u16) -> Result<()> {
        self.ensure_active()?;
        check(unsafe { sys::tg_ble_disconnect(conn) }, "BLE disconnect")
    }
    pub fn links(&self) -> Result<sys::tg_ble_links_t> {
        self.ensure_active()?;
        let mut status = unsafe { std::mem::zeroed() };
        check(unsafe { sys::tg_ble_get_links(&mut status) }, "BLE links")?;
        Ok(status)
    }
    pub fn services(&mut self, conn: u16, id: u32) -> Result<()> {
        self.ensure_active()?;
        check(unsafe { sys::tg_ble_services(conn, id) }, "BLE services")
    }
    pub fn characteristics(&mut self, conn: u16, start: u16, end: u16, id: u32) -> Result<()> {
        self.ensure_active()?;
        check(
            unsafe { sys::tg_ble_characteristics(conn, start, end, id) },
            "BLE characteristics",
        )
    }
    pub fn descriptors(&mut self, conn: u16, start: u16, end: u16, id: u32) -> Result<()> {
        self.ensure_active()?;
        check(
            unsafe { sys::tg_ble_descriptors(conn, start, end, id) },
            "BLE descriptors",
        )
    }
    pub fn read(&mut self, conn: u16, handle: u16, id: u32) -> Result<()> {
        self.ensure_active()?;
        check(unsafe { sys::tg_ble_read(conn, handle, id) }, "BLE read")
    }
    pub fn write(&mut self, conn: u16, handle: u16, data: &[u8], id: u32) -> Result<()> {
        self.ensure_active()?;
        if data.is_empty() || data.len() > 128 {
            bail!("BLE write length must be 1..128");
        }
        check(
            unsafe { sys::tg_ble_write(conn, handle, data.as_ptr(), data.len() as u16, id) },
            "BLE write",
        )
    }
    pub fn subscribe(&mut self, conn: u16, cccd: u16, mode: u8, id: u32) -> Result<()> {
        self.ensure_active()?;
        check(
            unsafe { sys::tg_ble_subscribe(conn, cccd, mode, id) },
            "BLE subscribe",
        )
    }
    pub fn poll_gatt(&mut self) -> Result<Option<sys::tg_ble_gatt_event_t>> {
        self.ensure_active()?;
        let mut event = unsafe { std::mem::zeroed() };
        let code = unsafe { sys::tg_ble_poll_gatt(&mut event) };
        if code == sys::ESP_ERR_NOT_FOUND {
            return Ok(None);
        }
        check(code, "BLE GATT event")?;
        Ok(Some(event))
    }
    pub fn new() -> Result<Self> {
        let mut lease = Lease::acquire(&OWNED, "BLE scanner")?;
        if let Err(err) = check(unsafe { sys::tg_ble_init() }, "BLE init") {
            lease.release();
            return Err(err);
        }
        Ok(Self { lease })
    }
    pub fn status(&self) -> Result<sys::tg_ble_status_t> {
        self.ensure_active()?;
        let mut status = unsafe { std::mem::zeroed() };
        check(unsafe { sys::tg_ble_get_status(&mut status) }, "BLE status")?;
        Ok(status)
    }
    pub fn start(&mut self, duration_ms: u32) -> Result<()> {
        self.ensure_active()?;
        let config = sys::tg_ble_scan_config_t {
            duration_ms,
            interval_units: 160,
            window_units: 80,
            active: true,
            filter_duplicates: true,
        };
        check(unsafe { sys::tg_ble_scan_start(&config) }, "BLE scan start")
    }
    pub fn poll(&mut self) -> Result<Option<Event>> {
        self.ensure_active()?;
        let mut raw: sys::tg_ble_event_t = unsafe { std::mem::zeroed() };
        let code = unsafe { sys::tg_ble_poll_event(&mut raw) };
        if code == sys::ESP_ERR_NOT_FOUND {
            return Ok(None);
        }
        check(code, "BLE poll")?;
        let event = match u32::from(raw.kind) {
            sys::TG_BLE_EVENT_REPORT => {
                let len = usize::from(raw.data_len);
                if len > raw.data.len() {
                    bail!("Invalid C BLE report length: {len}");
                }
                Event::Report(Report {
                    address: raw.address,
                    address_type: raw.address_type,
                    rssi: raw.rssi,
                    scan_response: raw.advertising_type == 4,
                    data: raw.data[..len].to_vec(),
                })
            }
            sys::TG_BLE_EVENT_COMPLETE => Event::Complete(raw.reason),
            sys::TG_BLE_EVENT_RESET => Event::Reset(raw.reason),
            kind => bail!("Unknown BLE event kind: {kind}"),
        };
        Ok(Some(event))
    }
    pub fn stop(&mut self) -> Result<()> {
        self.ensure_active()?;
        check(unsafe { sys::tg_ble_scan_stop() }, "BLE scan stop")
    }
    pub fn shutdown(&mut self) -> Result<()> {
        self.ensure_active()?;
        check(unsafe { sys::tg_ble_deinit() }, "BLE deinit")?;
        self.lease.release();
        Ok(())
    }
    fn ensure_active(&self) -> Result<()> {
        self.lease.ensure("BLE scanner")
    }
}
impl Drop for Scanner {
    fn drop(&mut self) {
        if self.lease.is_active() {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}
