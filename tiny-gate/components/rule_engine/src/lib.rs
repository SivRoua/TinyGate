#![cfg_attr(not(test), no_std)]

use core::ffi::{c_char, c_void};

#[cfg(not(test))]
use core::panic::PanicInfo;

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    extern "C" {
        fn abort() -> !;
    }
    unsafe {
        abort();
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TgProtocol {
    Ble = 0,
    Zigbee = 1,
    System = 2,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TgValType {
    Int = 0,
    Float = 1,
    Bool = 2,
    String = 3,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union TgValUnion {
    pub i_val: i64,
    pub f_val: f64,
    pub b_val: bool,
    pub s_val: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TgValue {
    pub val_type: TgValType,
    pub val: TgValUnion,
}

#[repr(C)]
pub struct TgEvent {
    pub device_id: *const c_char,
    pub protocol: TgProtocol,
    pub endpoint: u8,
    pub attribute: *const c_char,
    pub value: TgValue,
    pub timestamp_ms: u64,
}

#[repr(C)]
pub struct TgAction {
    pub target_device: *const c_char,
    pub protocol: TgProtocol,
    pub endpoint: u8,
    pub command: *const c_char,
    pub param: TgValue,
}

pub type TgActionCallback = unsafe extern "C" fn(action: *const TgAction, user_data: *mut c_void);

/// Helper: Safely converts a null-terminated C string pointer to a Rust &str
unsafe fn c_str_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
        if len > 256 {
            // Guard against unbounded strings
            return None;
        }
    }
    let bytes = core::slice::from_raw_parts(ptr as *const u8, len);
    core::str::from_utf8(bytes).ok()
}

static VERSION_BANNER: &[u8] = b"TinyGate Rule Engine v0.1.0-alpha (Rust RISC-V)\0";

#[unsafe(no_mangle)]
pub extern "C" fn tg_rule_engine_init() -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn tg_rule_engine_version() -> *const c_char {
    VERSION_BANNER.as_ptr() as *const c_char
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tg_rule_engine_dispatch(
    event: *const TgEvent,
    cb: Option<TgActionCallback>,
    user_data: *mut c_void,
) -> i32 {
    if event.is_null() {
        return -1;
    }

    let ev = &*event;
    let attr = c_str_to_str(ev.attribute).unwrap_or("");
    let dev = c_str_to_str(ev.device_id).unwrap_or("");

    let mut actions_triggered = 0;

    // Example Demo Rule:
    // Trigger: Any device reporting "temperature" > 30.0
    // Action: Send Zigbee command "turn_on" to "cooler_relay"
    if attr == "temperature" && ev.value.val_type == TgValType::Float {
        let temp = ev.value.val.f_val;
        if temp > 30.0 {
            if let Some(callback) = cb {
                static TARGET: &[u8] = b"cooler_relay\0";
                static CMD: &[u8] = b"turn_on\0";

                let action = TgAction {
                    target_device: TARGET.as_ptr() as *const c_char,
                    protocol: TgProtocol::Zigbee,
                    endpoint: 1,
                    command: CMD.as_ptr() as *const c_char,
                    param: TgValue {
                        val_type: TgValType::Bool,
                        val: TgValUnion { b_val: true },
                    },
                };

                callback(&action, user_data);
                actions_triggered += 1;
            }
        }
    }

    // Example Demo Rule 2:
    // Trigger: Device "button_main" with attribute "press" == true
    // Action: Local system command "toggle_relay"
    if dev == "button_main" && attr == "press" && ev.value.val_type == TgValType::Bool {
        if ev.value.val.b_val {
            if let Some(callback) = cb {
                static TARGET: &[u8] = b"local_relay\0";
                static CMD: &[u8] = b"toggle\0";

                let action = TgAction {
                    target_device: TARGET.as_ptr() as *const c_char,
                    protocol: TgProtocol::System,
                    endpoint: 0,
                    command: CMD.as_ptr() as *const c_char,
                    param: TgValue {
                        val_type: TgValType::Int,
                        val: TgValUnion { i_val: 1 },
                    },
                };

                callback(&action, user_data);
                actions_triggered += 1;
            }
        }
    }

    actions_triggered
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::CStr;
    use core::sync::atomic::{AtomicUsize, Ordering};

    static CALLED_COUNT: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn mock_callback(action: *const TgAction, _user_data: *mut c_void) {
        CALLED_COUNT.fetch_add(1, Ordering::SeqCst);
        let act = &*action;
        let cmd = CStr::from_ptr(act.command).to_str().unwrap();
        assert_eq!(cmd, "turn_on");
    }

    #[test]
    fn test_temp_trigger() {
        unsafe {
            CALLED_COUNT.store(0, Ordering::SeqCst);
            let dev = b"sensor_01\0";
            let attr = b"temperature\0";

            let ev = TgEvent {
                device_id: dev.as_ptr() as *const c_char,
                protocol: TgProtocol::Zigbee,
                endpoint: 1,
                attribute: attr.as_ptr() as *const c_char,
                value: TgValue {
                    val_type: TgValType::Float,
                    val: TgValUnion { f_val: 32.5 },
                },
                timestamp_ms: 1000,
            };

            let count = tg_rule_engine_dispatch(&ev, Some(mock_callback), core::ptr::null_mut());
            assert_eq!(count, 1);
            assert_eq!(CALLED_COUNT.load(Ordering::SeqCst), 1);
        }
    }
}
