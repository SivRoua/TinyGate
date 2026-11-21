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

impl Value {
    /// Short rendering for the status page (5x7 font: ASCII, no unit glyph).
    pub fn display(&self) -> String {
        match self {
            Value::Bool(on) => if *on { "ON" } else { "OFF" }.to_string(),
            Value::Number(value) => format!("{value:.2}"),
            Value::Text(text) => (*text).to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_names_match_the_contract() {
        assert_eq!(Property::Temperature.name(), "temperature");
        assert_eq!(Property::Power.name(), "power");
    }

    #[test]
    fn values_render_for_the_5x7_font() {
        assert_eq!(Value::Bool(true).display(), "ON");
        assert_eq!(Value::Number(25.5).display(), "25.50");
        assert_eq!(Value::Text("BOOT").display(), "BOOT");
    }
}
