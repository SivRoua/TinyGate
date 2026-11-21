//! Zigbee peer registry: which endpoint carries which product property.
//!
//! The C driver speaks clusters and raw attribute values; this module is the
//! one place that turns them into the gateway's device vocabulary.
use crate::gateway::device::{Property, Value};
use esp_idf_svc::sys;

/// Home Automation profile; only HA endpoints get configured.
pub(super) const PROFILE_HA: u16 = 0x0104;
pub(super) const CLUSTER_TEMPERATURE: u16 = 0x0402;
pub(super) const CLUSTER_ONOFF: u16 = 0x0006;

#[derive(Default)]
pub(super) struct Peer {
    temperature_endpoint: Option<u8>,
    onoff_endpoint: Option<u8>,
}

impl Peer {
    /// Whether this cluster still needs endpoint discovery. Configure only
    /// once per cluster; a failed read/report leaves it unclaimed for retry.
    pub(super) fn wants(&self, cluster: u16) -> bool {
        match cluster {
            CLUSTER_TEMPERATURE => self.temperature_endpoint.is_none(),
            CLUSTER_ONOFF => self.onoff_endpoint.is_none(),
            _ => false,
        }
    }

    /// Mark a cluster endpoint as successfully configured.
    pub(super) fn record(&mut self, cluster: u16, endpoint: u8) {
        match cluster {
            CLUSTER_TEMPERATURE => self.temperature_endpoint = Some(endpoint),
            CLUSTER_ONOFF => self.onoff_endpoint = Some(endpoint),
            _ => {}
        }
    }

    /// Attribute value to typed observation, per the bring-up protocol units
    /// (temperature attribute carries 0.01 degC; OnOff carries 0/1).
    pub(super) fn observation(&self, event: &sys::tg_zigbee_event_t) -> Option<(Property, Value)> {
        if self.temperature_endpoint == Some(event.endpoint) && event.cluster == CLUSTER_TEMPERATURE
        {
            Some((
                Property::Temperature,
                Value::Number(event.value as f32 / 100.0),
            ))
        } else if self.onoff_endpoint == Some(event.endpoint) && event.cluster == CLUSTER_ONOFF {
            Some((Property::Power, Value::Bool(event.value != 0)))
        } else {
            None
        }
    }
}

/// Short addresses in the valid peer range (0 reserved, 0xfff8+ reserved).
pub(super) fn is_peer_address(address: u16) -> bool {
    address > 0 && address < 0xfff8
}
