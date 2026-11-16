//! GPIO9-to-GND button. Driver events carry no application actions.
use anyhow::{bail, Result};
use esp_idf_svc::sys;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{marker::PhantomData, rc::Rc};
static OWNED: AtomicBool = AtomicBool::new(false);
pub struct Button {
    active: bool,
    _thread: PhantomData<Rc<()>>,
}
pub struct Event {
    pub kind: &'static str,
    pub timestamp_ms: u64,
    pub held_ms: u32,
}
fn check(code: i32) -> Result<()> {
    if code != sys::ESP_OK {
        bail!("Button ESP error 0x{code:X}");
    }
    Ok(())
}
impl Button {
    pub fn new() -> Result<Self> {
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            bail!("Button already owned");
        }
        if let Err(err) = check(unsafe { sys::tg_button_init() }) {
            OWNED.store(false, Ordering::Release);
            return Err(err);
        }
        Ok(Self {
            active: true,
            _thread: PhantomData,
        })
    }
    fn ensure(&self) -> Result<()> {
        if !self.active {
            bail!("Button already stopped");
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Result<Option<Event>> {
        self.ensure()?;
        let mut raw: sys::tg_button_event_t = unsafe { std::mem::zeroed() };
        let code = unsafe { sys::tg_button_poll_event(&mut raw) };
        if code == sys::ESP_ERR_NOT_FOUND {
            return Ok(None);
        }
        check(code)?;
        let kind = match u32::from(raw.kind) {
            sys::TG_BUTTON_PRESS => "PRESS",
            sys::TG_BUTTON_RELEASE => "RELEASE",
            sys::TG_BUTTON_SHORT => "SHORT",
            sys::TG_BUTTON_LONG => "LONG",
            other => bail!("Unknown button event: {other}"),
        };
        Ok(Some(Event {
            kind,
            timestamp_ms: raw.timestamp_ms,
            held_ms: raw.held_ms,
        }))
    }
    pub fn status(&self) -> Result<sys::tg_button_status_t> {
        self.ensure()?;
        let mut status = unsafe { std::mem::zeroed() };
        check(unsafe { sys::tg_button_get_status(&mut status) })?;
        Ok(status)
    }
    pub fn shutdown(&mut self) -> Result<()> {
        self.ensure()?;
        check(unsafe { sys::tg_button_deinit() })?;
        self.active = false;
        OWNED.store(false, Ordering::Release);
        Ok(())
    }
}
impl Drop for Button {
    fn drop(&mut self) {
        if self.active {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}
