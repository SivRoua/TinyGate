//! Rust wrappers for the board platform drivers.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum WifiPowerSave {
    None = 0,
    Minimum = 1,
    Maximum = 2,
}
#[allow(dead_code)]
impl Wifi {
    pub fn set_tx_power(&mut self, quarter_dbm: u8) -> Result<()> {
        self.lease.ensure()?;
        check(
            unsafe { sys::tg_wifi_set_tx_power(quarter_dbm) },
            "set Wi-Fi TX power",
        )
    }
    pub fn tx_power(&self) -> Result<u8> {
        self.lease.ensure()?;
        let mut power = 0;
        check(
            unsafe { sys::tg_wifi_get_tx_power(&mut power) },
            "get Wi-Fi TX power",
        )?;
        Ok(power)
    }
    pub fn set_power_save(&mut self, mode: WifiPowerSave) -> Result<()> {
        self.lease.ensure()?;
        check(
            unsafe { sys::tg_wifi_set_power_save(mode as u8) },
            "set Wi-Fi power save",
        )
    }
    pub fn power_save(&self) -> Result<WifiPowerSave> {
        self.lease.ensure()?;
        let mut mode = 0;
        check(
            unsafe { sys::tg_wifi_get_power_save(&mut mode) },
            "get Wi-Fi power save",
        )?;
        match mode {
            0 => Ok(WifiPowerSave::None),
            1 => Ok(WifiPowerSave::Minimum),
            2 => Ok(WifiPowerSave::Maximum),
            _ => bail!("Unknown Wi-Fi power save mode {mode}"),
        }
    }
    pub fn save_credentials(&self, ssid: &str, password: &str) -> Result<()> {
        self.lease.ensure()?;
        let ssid = CString::new(ssid)?;
        let password = CString::new(password)?;
        check(
            unsafe { sys::tg_wifi_save_credentials(ssid.as_ptr(), password.as_ptr()) },
            "save Wi-Fi credentials",
        )
    }
    pub fn load_credentials(&self) -> Result<(String, String)> {
        self.lease.ensure()?;
        let mut value: sys::tg_wifi_credentials_t = unsafe { std::mem::zeroed() };
        check(
            unsafe { sys::tg_wifi_load_credentials(&mut value) },
            "load Wi-Fi credentials",
        )?;
        let text = |data: &[core::ffi::c_char]| -> Result<String> {
            let bytes: Vec<u8> = data
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as u8)
                .collect();
            Ok(String::from_utf8(bytes)?)
        };
        Ok((text(&value.ssid)?, text(&value.password)?))
    }
    pub fn forget_credentials(&self) -> Result<()> {
        self.lease.ensure()?;
        check(
            unsafe { sys::tg_wifi_forget_credentials() },
            "forget Wi-Fi credentials",
        )
    }
    pub fn start_ap(&mut self, ssid: &str, password: &str, channel: u8) -> Result<()> {
        self.lease.ensure()?;
        let ssid = CString::new(ssid)?;
        let password = CString::new(password)?;
        check(
            unsafe { sys::tg_wifi_ap_start(ssid.as_ptr(), password.as_ptr(), channel) },
            "Wi-Fi AP start",
        )
    }
    pub fn stop_ap(&mut self) -> Result<()> {
        self.lease.ensure()?;
        check(unsafe { sys::tg_wifi_ap_stop() }, "Wi-Fi AP stop")
    }
    /// Explicit bounded policy, invoked by the core, never automatically at boot.
    pub fn connect_with_retry(&mut self, ssid: &str, password: &str, attempts: u8) -> Result<()> {
        use std::time::{Duration, Instant};
        if attempts == 0 || attempts > 5 {
            bail!("Wi-Fi attempts must be 1..5");
        }
        for attempt in 0..attempts {
            self.connect(ssid, password)?;
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                let state = self.status()?;
                if state.has_ip {
                    return Ok(());
                }
                if (!state.connecting && !state.associated && state.disconnect_reason != 0)
                    || Instant::now() >= deadline
                {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            self.disconnect()?;
            let deadline = Instant::now() + Duration::from_secs(2);
            while self.status()?.disconnecting {
                if Instant::now() >= deadline {
                    bail!("Wi-Fi disconnect timed out");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            if attempt + 1 < attempts {
                std::thread::sleep(Duration::from_millis(500u64 << attempt));
            }
        }
        bail!("Wi-Fi connection/DHCP failed after {attempts} attempts")
    }
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
