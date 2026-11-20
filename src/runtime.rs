//! Runtime integration: glue the esp-idf crates to the Rust ecosystem's
//! cross-crate contracts.
//!
//! `critical-section` 1.x requires exactly one implementation crate to
//! export the `_critical_section_1_0_*` symbols, and none of the esp-idf
//! crates register one. This module does, backed by FreeRTOS's nesting
//! critical sections (`vPortEnterCritical`/`vPortExitCritical`), which are
//! interrupt-disabling and therefore safe as a global lock on this
//! single-core chip. Everything that needs short non-preemptible
//! synchronization — the embassy-sync bus today — goes through this.

// `restore-state-none` is feature-unified by esp-idf-hal, so the state is ().
#[unsafe(no_mangle)]
extern "Rust" fn _critical_section_1_0_acquire() -> () {
    unsafe {
        esp_idf_svc::sys::vPortEnterCritical();
    }
}

#[unsafe(no_mangle)]
extern "Rust" fn _critical_section_1_0_release(_: ()) {
    unsafe {
        esp_idf_svc::sys::vPortExitCritical();
    }
}
