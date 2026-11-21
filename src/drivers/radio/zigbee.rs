//! Coordinator lifetime is the firmware lifetime; no implicit reset or Drop teardown.
use crate::drivers::lease::{Lease, check};
use anyhow::Result;
use esp_idf_svc::sys;
use std::sync::atomic::AtomicBool;

static OWNED: AtomicBool = AtomicBool::new(false);

/// `Send` by design: one instance per boot (`OWNED`) and every call must
/// originate from the single owning service task — the C component's
/// serialization contract, enforced by the gateway task structure.
pub struct Coordinator {
    _lease: Lease,
}

impl Coordinator {
    pub fn new() -> Result<Self> {
        let mut lease = Lease::acquire(&OWNED, "Zigbee coordinator")?;
        if let Err(error) = check(unsafe { sys::tg_zigbee_init() }, "Zigbee init") {
            lease.release();
            return Err(error);
        }
        Ok(Self { _lease: lease })
    }

    pub fn status(&self) -> Result<sys::tg_zigbee_status_t> {
        let mut status = unsafe { std::mem::zeroed() };
        check(
            unsafe { sys::tg_zigbee_get_status(&mut status) },
            "Zigbee status",
        )?;
        Ok(status)
    }

    pub fn poll(&mut self) -> Result<Option<sys::tg_zigbee_event_t>> {
        let mut event = unsafe { std::mem::zeroed() };
        let code = unsafe { sys::tg_zigbee_poll_event(&mut event) };
        if code == sys::ESP_ERR_NOT_FOUND {
            return Ok(None);
        }
        check(code, "Zigbee poll")?;
        Ok(Some(event))
    }

    pub fn permit_join(&mut self, seconds: u8) -> Result<()> {
        check(
            unsafe { sys::tg_zigbee_permit_join(seconds) },
            "Zigbee permit join",
        )
    }

    pub fn discover(&mut self, address: u16) -> Result<()> {
        check(
            unsafe { sys::tg_zigbee_discover(address) },
            "Zigbee discover",
        )
    }

    pub fn read(&mut self, address: u16, endpoint: u8, cluster: u16, id: u32) -> Result<()> {
        check(
            unsafe { sys::tg_zigbee_read_attribute(address, endpoint, cluster, id) },
            "Zigbee read",
        )
    }

    pub fn onoff(&mut self, address: u16, endpoint: u8, on: bool, id: u32) -> Result<()> {
        check(
            unsafe { sys::tg_zigbee_send_onoff(address, endpoint, u8::from(on), id) },
            "Zigbee on/off",
        )
    }

    pub fn reporting(&mut self, address: u16, endpoint: u8, cluster: u16, id: u32) -> Result<()> {
        check(
            unsafe { sys::tg_zigbee_configure_reporting(address, endpoint, cluster, id) },
            "Zigbee reporting",
        )
    }
}
