use core::fmt;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// SSD Logger — writes kernel logs to a file on the SSD.
///
/// Uses exfat-slim for filesystem operations and block-device-driver for I/O.
pub struct SsdLogger {
    buffer: String,
    path: String,
}

impl SsdLogger {
    /// Create a new SSD logger.
    /// `path` is the file path (e.g., "/kernel.log").
    pub fn new(path: &str) -> Self {
        Self {
            buffer: String::with_capacity(4096),
            path: String::from(path),
        }
    }

    /// Append a message to the log buffer.
    pub fn log(&mut self, msg: &str) {
        self.buffer.push_str(msg);
    }

    /// Get the current buffer contents.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Clear the buffer without writing.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Check if there are unflushed entries.
    pub fn has_pending(&self) -> bool {
        !self.buffer.is_empty()
    }

    /// Get the path this logger writes to.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl fmt::Write for SsdLogger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.log(s);
        Ok(())
    }
}

/// Macro for formatted logging.
#[macro_export]
macro_rules! ssd_log {
    ($logger:expr, $($arg:tt)*) => {
        use core::fmt::Write;
        let _ = write!($logger, $($arg)*);
    };
}

/// Simple ring buffer for in-memory logging (before SSD is available).
pub struct RingBuffer {
    entries: Vec<(u64, String)>,
    max_entries: usize,
    counter: u64,
}

impl RingBuffer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries),
            max_entries,
            counter: 0,
        }
    }

    pub fn push(&mut self, msg: &str) {
        self.counter += 1;
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push((self.counter, String::from(msg)));
    }

    pub fn entries(&self) -> &[(u64, String)] {
        &self.entries
    }

    pub fn flush_to_string(&self) -> String {
        let mut out = String::new();
        for (id, msg) in &self.entries {
            out.push_str(&format!("[{}] {}\n", id, msg));
        }
        out
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
