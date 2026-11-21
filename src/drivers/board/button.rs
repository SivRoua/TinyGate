//! GPIO9-to-GND button. Driver events carry no application actions.
use crate::drivers::lease::{Lease, check};
use anyhow::Result;
use esp_idf_svc::sys;
use std::sync::atomic::AtomicBool;

static OWNED: AtomicBool = AtomicBool::new(false);

pub struct Button {
    lease: Lease,
}

pub struct Event {
    pub kind: &'static str,
    pub timestamp_ms: u64,
    pub held_ms: u32,
}

impl Button {
    pub fn new() -> Result<Self> {
        let lease = Lease::acquire(&OWNED, "Button")?;
        if let Err(err) = check(unsafe { sys::tg_button_init() }, "Button init") {
            // init rolls back itself; release the lease so a later probe can
            // retry without rebooting.
            let mut lease = lease;
            lease.release();
            return Err(err);
        }
        Ok(Self { lease })
    }

    pub fn poll(&mut self) -> Result<Option<Event>> {
        self.lease.ensure("Button")?;
        let mut raw: sys::tg_button_event_t = unsafe { std::mem::zeroed() };
        let code = unsafe { sys::tg_button_poll_event(&mut raw) };
        if code == sys::ESP_ERR_NOT_FOUND {
            return Ok(None);
        }
        check(code, "Button poll")?;
        let kind = match u32::from(raw.kind) {
            sys::TG_BUTTON_PRESS => "PRESS",
            sys::TG_BUTTON_RELEASE => "RELEASE",
            sys::TG_BUTTON_SHORT => "SHORT",
            sys::TG_BUTTON_LONG => "LONG",
            other => anyhow::bail!("Unknown button event: {other}"),
        };
        Ok(Some(Event {
            kind,
            timestamp_ms: raw.timestamp_ms,
            held_ms: raw.held_ms,
        }))
    }

    pub fn status(&self) -> Result<sys::tg_button_status_t> {
        self.lease.ensure("Button")?;
        let mut status = unsafe { std::mem::zeroed() };
        check(
            unsafe { sys::tg_button_get_status(&mut status) },
            "Button status",
        )?;
        Ok(status)
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.lease.ensure("Button")?;
        check(unsafe { sys::tg_button_deinit() }, "Button deinit")?;
        self.lease.release();
        Ok(())
    }
}

impl Drop for Button {
    fn drop(&mut self) {
        if self.lease.is_active() {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}
