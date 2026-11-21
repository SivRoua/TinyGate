//! Status-page console over a row-addressable display backend.
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
//!
//! The console is backend-agnostic; `ui::mod` attaches the real OLED.
use anyhow::Result;
use std::fmt;

/// Text rows of the OLED canvas; one row maps 1:1 to one SSD1306 page.
const ROWS: u8 = 8;

/// Hardware sink for the console, so the row/paging logic stays testable.
pub trait Backend {
    fn write_line(&mut self, row: u8, text: &str) -> Result<()>;
    fn flush_page(&mut self, row: u8) -> Result<()>;
    fn clear(&mut self) -> Result<()>;
}

pub struct Console<B: Backend> {
    backend: B,
    cursor: u8,
    pending: String,
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
}
