//! Single-owner guard and C error mapping shared by every driver facade.
//!
//! Each C component may have exactly one Rust owner for a firmware run. The
//! atomic CAS in [`Lease::acquire`] enforces that; the owning-task structure
//! enforces the components' single-caller serialization contract. Thread
//! affinity is deliberately not encoded here: embassy services hold drivers
//! across `await` (DESIGN.md, 2026-11-21), and one owner is the real boundary.
use anyhow::{bail, Result};
use esp_idf_svc::sys;
use std::sync::atomic::{AtomicBool, Ordering};

/// Map a C `esp_err_t` to a Rust error with a stable operation label.
pub(crate) fn check(code: sys::esp_err_t, operation: &str) -> Result<()> {
    if code != sys::ESP_OK {
        bail!("{operation}: ESP error 0x{code:X}");
    }
    Ok(())
}

/// Ownership of one C component. A failed shutdown deliberately keeps the
/// lease: without it, a second owner could initialize a partially released
/// C subsystem.
pub(crate) struct Lease {
    owned: &'static AtomicBool,
    active: bool,
}

impl Lease {
    pub(crate) fn acquire(owned: &'static AtomicBool, name: &str) -> Result<Self> {
        if owned
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            bail!("{name} already owned");
        }
        Ok(Self {
            owned,
            active: true,
        })
    }

    pub(crate) fn ensure(&self, name: &str) -> Result<()> {
        if !self.active {
            bail!("{name} already shut down");
        }
        Ok(())
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn release(&mut self) {
        self.active = false;
        self.owned.store(false, Ordering::Release);
    }
}
