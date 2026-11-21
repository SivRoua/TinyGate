//! Coordinator lifetime is the firmware lifetime; no implicit reset or Drop teardown.
use anyhow::{bail, Result};
use esp_idf_svc::sys;
use std::sync::atomic::{AtomicBool, Ordering};
static OWNED: AtomicBool = AtomicBool::new(false);

/// `Send` by design: one instance per boot (`OWNED`) and every call must
/// originate from the single owning service task — the C component's
/// serialization contract, enforced by the gateway task structure.
pub struct Coordinator {}
fn check(code: i32) -> Result<()> {
    if code != sys::ESP_OK {
        bail!("Zigbee ESP error 0x{code:X}");
    }
    Ok(())
}
impl Coordinator {
    pub fn new() -> Result<Self> {
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            bail!("Zigbee coordinator already started; reboot required");
        }
        if let Err(error) = check(unsafe { sys::tg_zigbee_init() }) {
            OWNED.store(false, Ordering::Release);
            return Err(error);
        }
        Ok(Self {})
    }
    pub fn status(&self) -> Result<sys::tg_zigbee_status_t> {
        let mut status = unsafe { std::mem::zeroed() };
        check(unsafe { sys::tg_zigbee_get_status(&mut status) })?;
        Ok(status)
    }
    pub fn poll(&mut self) -> Result<Option<sys::tg_zigbee_event_t>> {
        let mut event = unsafe { std::mem::zeroed() };
        let code = unsafe { sys::tg_zigbee_poll_event(&mut event) };
        if code == sys::ESP_ERR_NOT_FOUND {
            return Ok(None);
        }
        check(code)?;
        Ok(Some(event))
    }
    pub fn permit_join(&mut self, seconds: u8) -> Result<()> {
        check(unsafe { sys::tg_zigbee_permit_join(seconds) })
    }
    pub fn discover(&mut self, address: u16) -> Result<()> {
        check(unsafe { sys::tg_zigbee_discover(address) })
    }
    pub fn read(&mut self, address: u16, endpoint: u8, cluster: u16, id: u32) -> Result<()> {
        check(unsafe { sys::tg_zigbee_read_attribute(address, endpoint, cluster, id) })
    }
    pub fn onoff(&mut self, address: u16, endpoint: u8, on: bool, id: u32) -> Result<()> {
        check(unsafe { sys::tg_zigbee_send_onoff(address, endpoint, u8::from(on), id) })
    }
    pub fn reporting(&mut self, address: u16, endpoint: u8, cluster: u16, id: u32) -> Result<()> {
        check(unsafe { sys::tg_zigbee_configure_reporting(address, endpoint, cluster, id) })
    }
}
