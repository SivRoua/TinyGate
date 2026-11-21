//! Status-page formatting macros. Exported at the crate root because Rust
//! macros cannot be method-scoped; every one of them needs a `Console`-like
//! value that implements `write_row`/`set_row` (see `ui::console`).

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
