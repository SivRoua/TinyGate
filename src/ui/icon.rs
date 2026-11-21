//! Icons rendered as two paired character cells on the OLED canvas.
//!
//! `Display::write_line` maps U+E080..U+E0FF onto the C canvas icon code
//! space, so an icon flows through any `print!`/`println!`/`write!` text
//! and takes two of the 21 columns. Halves can also stand alone as 5x7
//! mini icons via [`Icon::half_left`]/[`Icon::half_right`].
use std::fmt;

/// An 11x7 screen icon rendered from two paired character cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Icon {
    left: u8,
    right: u8,
}

impl Icon {
    /// Wi-Fi waves (0x80/0x81).
    pub const WIFI: Icon = Icon {
        left: 0x80,
        right: 0x81,
    };
    /// BLE crystal core (0x82/0x83).
    pub const BLE: Icon = Icon {
        left: 0x82,
        right: 0x83,
    };
    /// Zigbee "Z" (0x84/0x85).
    pub const ZB: Icon = Icon {
        left: 0x84,
        right: 0x85,
    };
    /// Battery with charge bar (0x86/0x87).
    pub const BAT: Icon = Icon {
        left: 0x86,
        right: 0x87,
    };
    /// Chain link pair (0x88/0x89).
    pub const LINK: Icon = Icon {
        left: 0x88,
        right: 0x89,
    };
    /// Warning triangle with exclamation (0x8A/0x8B).
    pub const WARN: Icon = Icon {
        left: 0x8A,
        right: 0x8B,
    };
    /// Heartbeat (0x8C/0x8D).
    pub const HEART: Icon = Icon {
        left: 0x8C,
        right: 0x8D,
    };
    /// Signal strength bars (0x8E/0x8F).
    pub const SIG: Icon = Icon {
        left: 0x8E,
        right: 0x8F,
    };

    fn half_byte(&self, side: u8) -> char {
        char::from_u32(0xE000 + u32::from(side)).expect("valid private-use char")
    }

    /// The left half alone, for inline 5x7 mini icons.
    pub fn half_left(&self) -> char {
        self.half_byte(self.left)
    }

    /// The right half alone, for inline 5x7 mini icons.
    pub fn half_right(&self) -> char {
        self.half_byte(self.right)
    }
}

impl fmt::Display for Icon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Write::write_char(f, self.half_left())?;
        fmt::Write::write_char(f, self.half_right())
    }
}

#[cfg(test)]
mod tests {
    use super::Icon;

    #[test]
    fn icon_formats_as_two_private_use_chars() {
        let text = format!("{} UP", Icon::WIFI);
        let chars: Vec<u32> = text.chars().map(|c| c as u32).collect();
        assert_eq!(
            chars,
            vec![0xE080, 0xE081, b' ' as u32, 'U' as u32, 'P' as u32]
        );
        // A single half flows inline too.
        assert_eq!(Icon::SIG.half_right() as u32, 0xE08F);
    }
}
