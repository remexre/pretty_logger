//! A pretty logger.
//!
//! A logger similar to `pretty_env_logger`, but configured from the `init`
//! function instead of from an environment variable.
//!
//! It also supports falling back to a non-colored logger.

#![deny(missing_docs)]

extern crate ansi_term;
extern crate isatty;
extern crate log;
extern crate unicode_segmentation;

use std::cmp::max;
use std::io::{stderr, stdout, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use ansi_term::{ANSIGenericString, Colour, Style};
use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use unicode_segmentation::UnicodeSegmentation;

/// Where to log errors to.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Destination {
    /// Standard output
    Stdout,

    /// Standard error
    Stderr,
}

impl Destination {
    /// Returns whether the given destination is a TTY.
    pub fn isatty(&self) -> bool {
        match *self {
            Destination::Stdout => isatty::stdout_isatty(),
            Destination::Stderr => isatty::stderr_isatty(),
        }
    }
}

impl Destination {
    /// Returns a `Write` corresponding to the `Destination`.
    fn write(&self) -> Box<Write> {
        match *self {
            Destination::Stdout => Box::new(stdout()),
            Destination::Stderr => Box::new(stderr()),
        }
    }
}

impl Default for Destination {
    fn default() -> Destination {
        Destination::Stderr
    }
}

/// The logger.
///
/// The defaults are:
///
///  - Log to `stderr`
///  - Log at the info level and higher.
///  - Use the default theme (see the [`Theme`](struct.Theme.html) type for details).
///  - Use color iff `stderr` is a TTY
pub struct Logger {
    destination: Destination,
    level: LevelFilter,
    max_module_width: AtomicUsize,
    max_target_width: AtomicUsize,
    theme: Theme,
}

impl Logger {
    /// Creates a new instance of Logger.
    pub fn new(
        destination: Destination,
        level: LevelFilter,
        theme: Theme,
    ) -> Logger {
        Logger {
            destination,
            level,
            max_module_width: AtomicUsize::new(0),
            max_target_width: AtomicUsize::new(0),
            theme,
        }
    }

    /// Sets this logger as the global logger.
    pub fn set_logger(self) -> Result<(), SetLoggerError> {
        log::set_boxed_logger(Box::new(self))
    }

    fn update_module_width(&self, width: usize) -> usize {
        loop {
            let old = self.max_module_width.load(Ordering::SeqCst);
            let new = max(old, width);
            if self.max_module_width.compare_and_swap(
                old,
                new,
                Ordering::SeqCst,
            ) == old
            {
                return new;
            }
        }
    }

    fn update_target_width(&self, width: usize) -> usize {
        loop {
            let old = self.max_target_width.load(Ordering::SeqCst);
            let new = max(old, width);
            if self.max_target_width.compare_and_swap(
                old,
                new,
                Ordering::SeqCst,
            ) == old
            {
                return new;
            }
        }
    }
}

impl Default for Logger {
    fn default() -> Logger {
        let destination = Destination::default();
        let theme = if destination.isatty() {
            Theme::default()
        } else {
            Theme::empty()
        };
        Logger::new(destination, LevelFilter::Info, theme)
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.level
            .to_level()
            .map(|level| metadata.level() <= level)
            .unwrap_or(false)
    }

    fn flush(&self) {}

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let module = record.module_path().unwrap_or("<unknown>");
        let target = record.target();
        let module_length =
            self.update_module_width(module.graphemes(true).count());

        let _ = if module == target {
            writeln!(
                self.destination.write(),
                "{}|{:.*}|{}",
                self.theme.paint_log_level(record.level()),
                module_length,
                module,
                record.args()
            )
        } else {
            let target_length =
                self.update_target_width(target.graphemes(true).count());
            writeln!(
                self.destination.write(),
                "{}|{:.*}|{:.*}|{}",
                self.theme.paint_log_level(record.level()),
                module_length,
                module,
                target_length,
                target,
                record.args()
            )
        };
    }
}

/// The color scheme to use.
///
/// The default theme has:
///
///  - `ERROR` printed in bold red.
///  - `WARN ` printed in yellow.
///  - `INFO ` printed in cyan.
///  - `DEBUG` printed in gray.
///  - `TRACE` printed in dimmed gray.
///  - The module name is not styled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// The style to give the "ERROR" string.
    pub error: Style,

    /// The style to give the "WARN" string.
    pub warn: Style,

    /// The style to give the "INFO" string.
    pub info: Style,

    /// The style to give the "DEBUG" string.
    pub debug: Style,

    /// The style to give the "TRACE" string.
    pub trace: Style,

    /// The style to give the module name.
    pub module: Style,
}

impl Theme {
    /// Returns a theme that does not highlight anything.
    pub fn empty() -> Theme {
        Theme {
            error: Style::new(),
            warn: Style::new(),
            info: Style::new(),
            debug: Style::new(),
            trace: Style::new(),
            module: Style::new(),
        }
    }

    /// Paints a log level with a theme.
    pub fn paint_log_level(
        &self,
        level: Level,
    ) -> ANSIGenericString<'static, str> {
        let (style, name) = match level {
            Level::Error => (self.error, "ERROR"),
            Level::Warn => (self.warn, "WARN "),
            Level::Info => (self.info, "INFO "),
            Level::Debug => (self.debug, "DEBUG"),
            Level::Trace => (self.trace, "TRACE"),
        };
        style.paint(name)
    }
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            error: Colour::Red.bold(),
            warn: Colour::Yellow.bold(),
            info: Colour::Cyan.normal(),
            debug: Colour::White.normal(),
            trace: Colour::White.dimmed(),
            module: Style::new(),
        }
    }
}

/// Initializes the global logger.
pub fn init(
    destination: Destination,
    level: LevelFilter,
    theme: Theme,
) -> Result<(), SetLoggerError> {
    platform_init();
    Logger::new(destination, level, theme).set_logger()
}

/// Initializes the global logger to log at the given level, using the defaults
/// for other fields.
pub fn init_level(level: LevelFilter) -> Result<(), SetLoggerError> {
    platform_init();
    let mut logger = Logger::default();
    logger.level = level;
    logger.set_logger()
}

/// Initializes the global logger with the defaults.
pub fn init_to_defaults() -> Result<(), SetLoggerError> {
    platform_init();
    Logger::default().set_logger()
}

#[cfg(windows)]
fn platform_init() {
    use ansi_term::enable_ansi_support;
    let _ = enable_ansi_support();
}

#[cfg(not(windows))]
fn platform_init() {}
