//! Device vocabulary shared by every access protocol and by the uplink.
//!
//! Shapes align with `contracts/schemas` v1: snake_case property names,
//! string endpoints, bool/number values. Wire-format concerns (`event_id`,
//! `seq`, JSON encoding) belong to the event and uplink milestones.

/// Access protocol of a device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Zigbee,
    Ble,
}

/// Known observable properties (contracts: `temperature` degC, `power` bool).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Property {
    Temperature,
    Power,
}

impl Property {
    /// Contract field name (`snake_case`, stable).
    pub fn name(&self) -> &'static str {
        match self {
            Property::Temperature => "temperature",
            Property::Power => "power",
        }
    }
}

/// A property value; contracts allow `boolean|number|string`, never null.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Number(f32),
    Text(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_names_match_the_contract() {
        assert_eq!(Property::Temperature.name(), "temperature");
        assert_eq!(Property::Power.name(), "power");
    }
}
