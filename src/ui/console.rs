//! Status-page console over the OLED.
//!
//! The screen is a **status page, not a terminal**: `println!` fills rows
//! top-down; once row 8 is reached the cursor wraps to row 0 and the next
//! `println!` starts overwriting a fresh page. Callers redraw the complete
//! page each cycle (typically `clear!` or `home` first).
//!
//! `print!` appends to the current row's buffer; the row is rendered and its
//! single page flushed only on the next newline (`println!`), or via
//! `flush_line`. One row costs ~1..2 I2C transactions instead of the
//! whole-frame 100ms flush.
use crate::drivers::display::Display;
use anyhow::Result;
use std::fmt;

/// Text rows of the OLED canvas; one row maps 1:1 to one SSD1306 page.
const ROWS: u8 = 8;

/// An 11x7 screen icon rendered from two paired character cells.
///
/// `Display::write_line` maps U+E080..U+E0FF onto the C canvas icon code
/// space, so an icon flows through any `print!`/`println!`/`write!` text
/// and takes two of the 21 columns. Halves can also stand alone as 5x7
/// mini icons via [`Icon::half_left`]/[`Icon::half_right`].
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

/// Hardware sink for the console, so the row/paging logic stays testable.
pub trait Backend {
    fn write_line(&mut self, row: u8, text: &str) -> Result<()>;
    fn flush_page(&mut self, row: u8) -> Result<()>;
    fn clear(&mut self) -> Result<()>;
}

impl Backend for Display {
    fn write_line(&mut self, row: u8, text: &str) -> Result<()> {
        Display::write_line(self, row, text)
    }
    fn flush_page(&mut self, row: u8) -> Result<()> {
        Display::flush_page(self, row)
    }
    fn clear(&mut self) -> Result<()> {
        Display::clear(self)
    }
}

pub struct Console<B: Backend = Display> {
    backend: B,
    cursor: u8,
    pending: String,
}

impl Console<Display> {
    /// Take exclusive ownership of the OLED.
    pub fn new(display: Display) -> Self {
        Self::with_backend(display)
    }
}

impl<B: Backend> Console<B> {
    /// Wrap a backend (tests inject a recorder here).
    pub fn with_backend(backend: B) -> Self {
        Self {
            backend,
            cursor: 0,
            pending: String::new(),
        }
    }

    /// Next output writes row 0 without clearing old content.
    pub fn home(&mut self) {
        self.cursor = 0;
    }

    /// Blank the screen and reset the cursor; start of a new page.
    pub fn clear(&mut self) -> Result<()> {
        self.pending.clear();
        self.cursor = 0;
        self.backend.clear()
    }

    /// Row the next newline will render into (0..8).
    pub fn row(&self) -> u8 {
        self.cursor
    }

    /// Point the cursor at a row (wrapped into 0..8); pending text is kept.
    pub fn set_row(&mut self, row: u8) {
        self.cursor = row % ROWS;
    }

    /// Unwrap the backend, releasing the console's exclusive hold on it.
    pub fn into_backend(self) -> B {
        self.backend
    }

    /// Render text into an explicit row and flush only that page.
    /// The cursor and the pending row buffer stay untouched.
    pub fn write_row(&mut self, row: u8, text: &str) -> Result<()> {
        self.backend.write_line(row, text)?;
        self.backend.flush_page(row)
    }

    /// Render the buffered text at the cursor, flush its page, then advance.
    fn emit_pending(&mut self) -> Result<()> {
        let text = std::mem::take(&mut self.pending);
        let row = self.cursor;
        self.backend.write_line(row, &text)?;
        self.backend.flush_page(row)?;
        self.cursor = (self.cursor + 1) % ROWS;
        Ok(())
    }

    /// Render the buffered row now without advancing or wrapping.
    pub fn flush_line(&mut self) -> Result<()> {
        let text = std::mem::take(&mut self.pending);
        self.backend.write_line(self.cursor, &text)?;
        self.backend.flush_page(self.cursor)
    }
}

impl<B: Backend> fmt::Write for Console<B> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let mut segments = text.split('\n');
        if let Some(first) = segments.next() {
            self.pending.push_str(first);
        }
        for segment in segments {
            if let Err(err) = self.emit_pending() {
                log::warn!("OLED console row {}: {err:#}", self.cursor);
                return Err(fmt::Error);
            }
            self.pending.push_str(segment);
        }
        Ok(())
    }
}

/// `print!(scr, fmt...)`: buffered write into the current row. Nothing is
/// rendered until a newline, `flush_line`, or the next `println!`.
/// The caller must have `fmt::Write` in scope (same contract as `std::write!`).
#[macro_export]
macro_rules! print {
    ($scr:expr, $($arg:tt)*) => {
        ($scr).write_fmt(format_args!($($arg)*))
    };
}

/// `println!(scr, fmt...)`: render the buffered row, flush only its page,
/// and advance; wrapping past row 7 back to row 0 starts a new status page.
#[macro_export]
macro_rules! println {
    ($scr:expr) => {
        $crate::print!($scr, "\n")
    };
    ($scr:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {{
        // Two calls: writing $fmt directly keeps inline captures like "R{row}"
        // working; a separate newline then emits the row.
        let status = $crate::print!($scr, $fmt $(, $arg)*);
        if status.is_ok() {
            $crate::print!($scr, "\n")
        } else {
            status
        }
    }};
}

/// `print_at!(scr, row, fmt...)`: render straight into `row` and flush only
/// that page. The cursor and the pending row buffer stay untouched, so this
/// is the fixed-layout primitive for status pages.
#[macro_export]
macro_rules! print_at {
    ($scr:expr, $row:expr, $($arg:tt)*) => {
        ($scr).write_row($row, &format!($($arg)*))
    };
}

/// `println_at!(scr, row, fmt...)`: write `row` in place, then point the
/// cursor at `row + 1` so subsequent stream output continues below it.
/// Wrapping past row 7 back to row 0 keeps the status-page semantics.
#[macro_export]
macro_rules! println_at {
    ($scr:expr, $row:expr, $($arg:tt)*) => {{
        let status = ($scr).write_row($row, &format!($($arg)*));
        if status.is_ok() {
            ($scr).set_row($row + 1);
        }
        status
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    #[derive(Default)]
    struct Recorder {
        lines: Vec<(u8, String)>,
        flushed: Vec<u8>,
    }
    impl Backend for Recorder {
        fn write_line(&mut self, row: u8, text: &str) -> Result<()> {
            self.lines.push((row, text.to_string()));
            Ok(())
        }
        fn flush_page(&mut self, row: u8) -> Result<()> {
            self.flushed.push(row);
            Ok(())
        }
        fn clear(&mut self) -> Result<()> {
            self.lines.clear();
            self.flushed.clear();
            Ok(())
        }
    }

    #[test]
    fn print_buffers_until_newline() {
        let mut scr = Console::with_backend(Recorder::default());
        write!(scr, "UP ").unwrap();
        crate::print!(scr, "{} S", 42).unwrap();
        // Nothing rendered yet: both fragments are still in the row buffer.
        assert_eq!(scr.row(), 0);
        assert!(scr.backend.lines.is_empty());
        crate::println!(scr).unwrap(); // bare newline emits the buffered row
        assert_eq!(scr.backend.lines, vec![(0, "UP 42 S".to_string())]);
        assert_eq!(scr.backend.flushed, vec![0]);
        assert_eq!(scr.row(), 1);
    }

    #[test]
    fn println_advances_and_wraps_pages() {
        let mut scr = Console::with_backend(Recorder::default());
        for row in 0..8 {
            crate::println!(scr, "R{row}").unwrap();
            assert_eq!(scr.row(), row + 1);
        }
        // Full page written; the next println starts overwriting row 0.
        crate::println!(scr, "NEW PAGE").unwrap();
        assert_eq!(scr.backend.lines.first().unwrap().1, "NEW PAGE");
        assert_eq!(scr.backend.lines.last().unwrap().0, 0);
        assert_eq!(scr.row(), 1);
    }

    #[test]
    fn embedded_newlines_render_in_order() {
        let mut scr = Console::with_backend(Recorder::default());
        write!(scr, "A\nB\n\nC").unwrap();
        let rows: Vec<(u8, String)> = scr
            .backend
            .lines
            .iter()
            .map(|(r, t)| (*r, t.clone()))
            .collect();
        assert_eq!(
            rows,
            vec![
                (0, "A".to_string()),
                (1, "B".to_string()),
                (2, String::new()),
            ]
        );
        assert!(crate::print!(scr, "").is_ok());
        crate::println!(scr).unwrap(); // emits "C"
        assert_eq!(scr.backend.lines.last().unwrap(), &(3, "C".to_string()));
    }

    #[test]
    fn print_at_writes_row_without_disturbing_stream_state() {
        let mut scr = Console::with_backend(Recorder::default());
        write!(scr, "PENDING").unwrap(); // buffered, not rendered yet
        crate::print_at!(scr, 5, "FIXED {} B", 64).unwrap();
        assert_eq!(scr.backend.lines, vec![(5, "FIXED 64 B".to_string())]);
        assert_eq!(scr.backend.flushed, vec![5]);
        // Cursor and pending buffer untouched: the stream row still emits to row 0.
        assert_eq!(scr.row(), 0);
        crate::println!(scr).unwrap();
        assert_eq!(
            scr.backend.lines.last().unwrap(),
            &(0, "PENDING".to_string())
        );
    }

    #[test]
    fn println_at_seeks_cursor_below_the_written_row() {
        let mut scr = Console::with_backend(Recorder::default());
        crate::println_at!(scr, 6, "TITLE").unwrap();
        assert_eq!(scr.backend.lines, vec![(6, "TITLE".to_string())]);
        assert_eq!(scr.row(), 7);
        crate::println!(scr, "BODY").unwrap(); // stream output continues below
        assert_eq!(scr.backend.lines.last().unwrap(), &(7, "BODY".to_string()));
        // row + 1 wraps into the next page.
        crate::println_at!(scr, 7, "END").unwrap();
        assert_eq!(scr.row(), 0);
    }

    #[test]
    fn icon_formats_as_two_private_use_chars() {
        let text = format!("{} UP", super::Icon::WIFI);
        let chars: Vec<u32> = text.chars().map(|c| c as u32).collect();
        assert_eq!(
            chars,
            vec![0xE080, 0xE081, b' ' as u32, 'U' as u32, 'P' as u32]
        );
        // A single half flows inline too.
        assert_eq!(super::Icon::SIG.half_right() as u32, 0xE08F);
    }
}
