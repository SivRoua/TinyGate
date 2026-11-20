//! Chip-level system information (wraps tg_system).
use anyhow::Result;
use esp_idf_svc::sys;

use super::check;

pub fn system_info() -> Result<sys::tg_system_info_t> {
    let mut info = unsafe { std::mem::zeroed() };
    check(unsafe { sys::tg_system_get_info(&mut info) }, "system info")?;
    Ok(info)
}
