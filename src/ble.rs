//! Safe single-threaded owner of the C BLE scanner. Bindings come from tg_ble.h.
use anyhow::{bail, Result};
use esp_idf_svc::sys;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{marker::PhantomData, rc::Rc};

static OWNED: AtomicBool = AtomicBool::new(false);

pub struct Scanner {
    active: bool,
    _single_thread: PhantomData<Rc<()>>,
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
        crate::ble_ad::local_name(&self.data).map(|b| String::from_utf8_lossy(b).into_owned())
    }
}
fn check(code: i32, operation: &str) -> Result<()> {
    if code != sys::ESP_OK {
        bail!("{operation}: ESP error 0x{code:X}");
    }
    Ok(())
}
impl Scanner {
    pub fn new() -> Result<Self> {
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            bail!("BLE scanner already owned");
        }
        if let Err(err) = check(unsafe { sys::tg_ble_init() }, "BLE init") {
            OWNED.store(false, Ordering::Release);
            return Err(err);
        }
        Ok(Self {
            active: true,
            _single_thread: PhantomData,
        })
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
        self.active = false;
        OWNED.store(false, Ordering::Release);
        Ok(())
    }
    fn ensure_active(&self) -> Result<()> {
        if !self.active {
            bail!("BLE scanner has been shut down");
        }
        Ok(())
    }
}
impl Drop for Scanner {
    fn drop(&mut self) {
        if self.active {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}
