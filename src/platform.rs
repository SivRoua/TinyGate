//! Rust-side examples of consuming the public C driver interfaces.
use anyhow::{bail, Result};
use esp_idf_svc::sys;
use std::ffi::CString;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

fn check(code: i32, operation: &str) -> Result<()> {
    if code != sys::ESP_OK {
        bail!("{operation}: ESP error 0x{code:X}");
    }
    Ok(())
}

// A failed shutdown deliberately retains ownership to prevent a second owner
// from initializing a partially released C subsystem.
struct Lease {
    owned: &'static AtomicBool,
    active: bool,
    _thread: PhantomData<Rc<()>>,
}
impl Lease {
    fn acquire(owned: &'static AtomicBool) -> Result<Self> {
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
    fn ensure(&self) -> Result<()> {
        if !self.active {
            bail!("Driver already shut down");
        }
        Ok(())
    }
    fn release(&mut self) {
        self.active = false;
        self.owned.store(false, Ordering::Release);
    }
}

static WIFI_OWNED: AtomicBool = AtomicBool::new(false);
static TEMPERATURE_OWNED: AtomicBool = AtomicBool::new(false);

pub fn system_info() -> Result<sys::tg_system_info_t> {
    let mut info = unsafe { std::mem::zeroed() };
    check(unsafe { sys::tg_system_get_info(&mut info) }, "system info")?;
    Ok(info)
}

pub struct Temperature {
    lease: Lease,
}
impl Temperature {
    pub fn new() -> Result<Self> {
        let mut lease = Lease::acquire(&TEMPERATURE_OWNED)?;
        if let Err(err) = check(
            unsafe { sys::tg_temperature_init(-10, 80) },
            "temperature init",
        ) {
            // init normally rolls back itself; deinit also covers partial enable failure.
            let cleanup = unsafe { sys::tg_temperature_deinit() };
            if cleanup == sys::ESP_OK || cleanup == sys::ESP_ERR_INVALID_STATE {
                lease.release();
            }
            return Err(err);
        }
        Ok(Self { lease })
    }
    pub fn read(&self) -> Result<f32> {
        self.lease.ensure()?;
        let mut value = 0.0;
        check(
            unsafe { sys::tg_temperature_read(&mut value) },
            "temperature read",
        )?;
        Ok(value)
    }
    pub fn shutdown(&mut self) -> Result<()> {
        self.lease.ensure()?;
        check(
            unsafe { sys::tg_temperature_deinit() },
            "temperature deinit",
        )?;
        self.lease.release();
        Ok(())
    }
}
impl Drop for Temperature {
    fn drop(&mut self) {
        if self.lease.active {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}

pub struct Wifi {
    lease: Lease,
}
impl Wifi {
    pub fn new() -> Result<Self> {
        let mut lease = Lease::acquire(&WIFI_OWNED)?;
        if let Err(err) = check(unsafe { sys::tg_wifi_init() }, "Wi-Fi init") {
            let cleanup = unsafe { sys::tg_wifi_deinit() };
            if cleanup == sys::ESP_OK || cleanup == sys::ESP_ERR_INVALID_STATE {
                lease.release();
            }
            return Err(err);
        }
        Ok(Self { lease })
    }
    pub fn scan(&mut self) -> Result<()> {
        self.lease.ensure()?;
        check(unsafe { sys::tg_wifi_scan_start() }, "Wi-Fi scan start")
    }
    pub fn status(&self) -> Result<sys::tg_wifi_status_t> {
        self.lease.ensure()?;
        let mut status = unsafe { std::mem::zeroed() };
        check(
            unsafe { sys::tg_wifi_get_status(&mut status) },
            "Wi-Fi status",
        )?;
        Ok(status)
    }
    pub fn ap(&self, index: u16) -> Result<sys::tg_wifi_ap_t> {
        self.lease.ensure()?;
        let mut ap = unsafe { std::mem::zeroed() };
        check(
            unsafe { sys::tg_wifi_scan_get_ap(index, &mut ap) },
            "Wi-Fi scan result",
        )?;
        Ok(ap)
    }
    #[allow(dead_code)]
    pub fn connect(&mut self, ssid: &str, password: &str) -> Result<()> {
        self.lease.ensure()?;
        let ssid = CString::new(ssid)?;
        let password = CString::new(password)?;
        // C copies credentials synchronously before these strings are dropped.
        check(
            unsafe { sys::tg_wifi_connect(ssid.as_ptr(), password.as_ptr()) },
            "Wi-Fi connect",
        )
    }
    #[allow(dead_code)]
    pub fn disconnect(&mut self) -> Result<()> {
        self.lease.ensure()?;
        check(unsafe { sys::tg_wifi_disconnect() }, "Wi-Fi disconnect")
    }
    pub fn shutdown(&mut self) -> Result<()> {
        self.lease.ensure()?;
        check(unsafe { sys::tg_wifi_deinit() }, "Wi-Fi deinit")?;
        self.lease.release();
        Ok(())
    }
}
impl Drop for Wifi {
    fn drop(&mut self) {
        if self.lease.active {
            if let Err(err) = self.shutdown() {
                log::error!("{err:#}");
            }
        }
    }
}
