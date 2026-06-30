use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU8, Ordering};

use crate::drivers;
use crate::krnl::time;
use crate::sync::spinlock::SpinLock;

const ENTRY_COUNT: usize = 256;
pub const MESSAGE_SIZE: usize = 512;

static KLOG: SpinLock<KLog> = SpinLock::new(KLog::new());
static MAX_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

#[derive(Clone, Copy)]
pub struct LogEntry {
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub level: LogLevel,
    pub len: usize,
    pub message: [u8; MESSAGE_SIZE],
}

struct KLog {
    entries: [LogEntry; ENTRY_COUNT],
    next_slot: usize,
    next_sequence: u64,
}

struct EntryWriter {
    entry: LogEntry,
}

impl LogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN ",
            Self::Info => "INFO ",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "error" | "ERROR" => Some(Self::Error),
            "warn" | "warning" | "WARN" | "WARNING" => Some(Self::Warn),
            "info" | "INFO" => Some(Self::Info),
            "debug" | "DEBUG" => Some(Self::Debug),
            "trace" | "TRACE" => Some(Self::Trace),
            _ => None,
        }
    }
}

impl LogEntry {
    const fn empty() -> Self {
        Self {
            sequence: 0,
            timestamp_ns: 0,
            level: LogLevel::Info,
            len: 0,
            message: [0; MESSAGE_SIZE],
        }
    }

    pub fn message(&self) -> &str {
        core::str::from_utf8(&self.message[..self.len]).unwrap_or("<invalid utf8>")
    }
}

impl KLog {
    const fn new() -> Self {
        Self {
            entries: [LogEntry::empty(); ENTRY_COUNT],
            next_slot: 0,
            next_sequence: 1,
        }
    }

    fn push(&mut self, level: LogLevel, args: fmt::Arguments) {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1).max(1);

        let mut writer = EntryWriter {
            entry: LogEntry {
                sequence,
                timestamp_ns: time::uptime_ns(),
                level,
                len: 0,
                message: [0; MESSAGE_SIZE],
            },
        };

        let _ = writer.write_fmt(args);

        self.entries[self.next_slot] = writer.entry;
        self.next_slot = (self.next_slot + 1) % ENTRY_COUNT;
    }

    fn first_after(&self, sequence: u64) -> Option<LogEntry> {
        let mut found = None;

        for entry in self.entries.iter() {
            if entry.len == 0 || entry.sequence <= sequence {
                continue;
            }

            if found.map_or(true, |current: LogEntry| entry.sequence < current.sequence) {
                found = Some(*entry);
            }
        }

        found
    }
}

impl Write for EntryWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let free = MESSAGE_SIZE.saturating_sub(self.entry.len);
        let bytes = text.as_bytes();
        let len = bytes.len().min(free);

        self.entry.message[self.entry.len..self.entry.len + len].copy_from_slice(&bytes[..len]);
        self.entry.len += len;

        Ok(())
    }
}

pub fn write(level: LogLevel, args: fmt::Arguments) {
    if !enabled(level) {
        return;
    }

    KLOG.lock().push(level, args);
    drivers::serial::flush_klog();
    drivers::console::flush_klog();
}

pub fn set_max_level(level: LogLevel) {
    MAX_LEVEL.store(level as u8, Ordering::Relaxed);
}

pub fn enabled(level: LogLevel) -> bool {
    level as u8 <= MAX_LEVEL.load(Ordering::Relaxed)
}

pub fn configure_from_cmdline(cmdline: &str) {
    for arg in cmdline.split_ascii_whitespace() {
        if let Some(level) = arg.strip_prefix("loglevel=") {
            if let Some(level) = LogLevel::parse(level) {
                set_max_level(level);
            }
        }
    }
}

pub fn next_after(sequence: u64) -> Option<LogEntry> {
    KLOG.lock().first_after(sequence)
}

#[macro_export]
macro_rules! klog_error {
    ($($arg:tt)*) => {
        $crate::krnl::klog::write(
            $crate::krnl::klog::LogLevel::Error,
            core::format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! klog_warn {
    ($($arg:tt)*) => {
        $crate::krnl::klog::write(
            $crate::krnl::klog::LogLevel::Warn,
            core::format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! klog_info {
    ($($arg:tt)*) => {
        $crate::krnl::klog::write(
            $crate::krnl::klog::LogLevel::Info,
            core::format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! klog_debug {
    ($($arg:tt)*) => {
        $crate::krnl::klog::write(
            $crate::krnl::klog::LogLevel::Debug,
            core::format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! klog_trace {
    ($($arg:tt)*) => {
        $crate::krnl::klog::write(
            $crate::krnl::klog::LogLevel::Trace,
            core::format_args!($($arg)*),
        )
    };
}
