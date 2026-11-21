//! Connection identity per the MQTT contract: every boot gets a fresh
//! `boot_id`, `boot_counter` persists across reboots, and `seq` climbs
//! within this boot (contracts: online/heartbeat + event ordering).
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs};
use log::warn;
use std::sync::atomic::{AtomicU32, Ordering};

/// NVS namespace for gateway-own state (Wi-Fi credentials live in `tg_wifi`).
const NAMESPACE: &str = "tg_ident";
const BOOT_COUNTER_KEY: &str = "boot_counter";

pub struct Identity {
    pub boot_id: String,
    pub boot_counter: u32,
    seq: AtomicU32,
}

impl Identity {
    /// Load the persistent `boot_counter`, increment it for this boot, and
    /// mint a `boot_id`. A failing NVS degrades to a boot-scoped identity
    /// (counter 1) instead of blocking the firmware.
    pub fn load() -> Identity {
        let counter = match bump_counter() {
            Ok(next) => next,
            Err(err) => {
                warn!("identity NVS unavailable: {err:#}");
                1
            }
        };
        Identity {
            boot_id: format!("boot_{counter}"),
            boot_counter: counter,
            seq: AtomicU32::new(0),
        }
    }

    /// Monotonic event sequence within this boot (contracts: `(boot_counter,
    /// seq)` orders observations).
    pub fn next_seq(&self) -> u32 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }

    /// R4: assigned at deployment and persistent. Placeholder until the
    /// provisioning milestone; the contract charset is `[A-Za-z0-9_-]`.
    pub fn gateway_id(&self) -> &'static str {
        "tinygate-dev"
    }
}

/// Read-modify-write the persistent boot counter. The default NVS partition
/// is taken once here (esp-idf-svc's Rust-side guard is single-shot).
fn bump_counter() -> anyhow::Result<u32> {
    let partition = EspDefaultNvsPartition::take()?;
    let nvs = EspNvs::new(partition, NAMESPACE, true)?;
    let next = nvs
        .get_u32(BOOT_COUNTER_KEY)?
        .unwrap_or(0)
        .saturating_add(1);
    nvs.set_u32(BOOT_COUNTER_KEY, next)?;
    Ok(next)
}
