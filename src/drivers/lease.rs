//! Shared ownership guard and C error mapping for driver facades.
//!
//! Every C component is single-owner: exactly one Rust wrapper may own it
//! for a firmware run. `Lease` and `check` keep that contract in one place
//! instead of one hand-written copy per driver.
use anyhow::{bail, Result};
use esp_idf_svc::sys;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn check(code: i32, operation: &str) -> Result<()> {
    if code != sys::ESP_OK {
        bail!("{operation}: ESP error 0x{code:X}");
    }
    Ok(())
}

// A failed shutdown deliberately retains ownership to prevent a second owner
// from initializing a partially released C subsystem.
pub(crate) struct Lease {
    owned: &'static AtomicBool,
    pub(crate) active: bool,
    _thread: PhantomData<Rc<()>>,
}

impl Lease {
    pub(crate) fn acquire(owned: &'static AtomicBool) -> Result<Self> {
        if owned
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            bail!("Driver already owned");
        }
        Ok(Self {
            owned,
            active: true,
            _thread: PhantomData,
        })
    }

    pub(crate) fn ensure(&self) -> Result<()> {
        if !self.active {
            bail!("Driver already shut down");
        }
        Ok(())
    }

    pub(crate) fn release(&mut self) {
        self.active = false;
        self.owned.store(false, Ordering::Release);
    }
}
