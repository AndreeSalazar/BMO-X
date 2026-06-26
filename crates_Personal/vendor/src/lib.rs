#![no_std]

//! FastOS Hardware Abstraction Layer
//!
//! - PCI config space access (IO ports + ECAM)
//! - AHCI/SATA disk driver (via simple-ahci)
//! - Minimal exFAT write (for kernel logging)
//! - Kernel logging to SSD

extern crate alloc;

pub mod pci;

/// AHCI/SATA driver — wraps simple-ahci with our HAL.
pub mod ahci {
    pub use simple_ahci::AhciDriver;
    pub use simple_ahci::Hal;

    /// Our HAL: identity mapping (virt == phys during boot).
    pub struct FastOsHal;

    impl Hal for FastOsHal {
        fn virt_to_phys(virt: usize) -> usize {
            virt
        }

        fn flush_dcache() {
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        }

        fn current_ms() -> u64 {
            // Use TSC for timing — rough approximation
            unsafe {
                let lo: u32;
                let hi: u32;
                core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi);
                ((hi as u64) << 32 | lo as u64) / 3_500_000 // ~3.5 MHz TSC
            }
        }
    }

    pub type AhciDisk = AhciDriver<FastOsHal>;
}

/// Minimal exFAT support for kernel logging.
pub mod fs {
    use alloc::vec::Vec;

    /// Minimal exFAT writer — appends data to a file.
    /// Full implementation uses exfat-slim when async runtime is available.
    pub struct ExFatWriter {
        pub data: Vec<u8>,
    }

    impl ExFatWriter {
        pub fn new() -> Self {
            Self { data: Vec::new() }
        }

        pub fn append(&mut self, data: &[u8]) {
            self.data.extend_from_slice(data);
        }

        pub fn as_bytes(&self) -> &[u8] {
            &self.data
        }

        pub fn clear(&mut self) {
            self.data.clear();
        }
    }

    /// exFAT boot sector constants.
    pub const EXFAT_SIGNATURE: &[u8; 8] = b"EXFAT   ";
    pub const BYTES_PER_SECTOR_SHIFT: usize = 108;
    pub const SECTORS_PER_CLUSTER_SHIFT: usize = 110;
    pub const FAT_OFFSET: usize = 80;
    pub const CLUSTER_HEAP_OFFSET: usize = 88;
    pub const ROOT_CLUSTER: usize = 96;
}

/// Kernel logging to SSD.
pub mod log {
    use core::fmt;
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;

    /// SSD Logger — buffers log entries for batch writing to SSD.
    pub struct SsdLogger {
        buffer: String,
        path: String,
    }

    impl SsdLogger {
        pub fn new(path: &str) -> Self {
            Self {
                buffer: String::with_capacity(4096),
                path: String::from(path),
            }
        }

        pub fn log(&mut self, msg: &str) {
            self.buffer.push_str(msg);
        }

        pub fn buffer(&self) -> &str {
            &self.buffer
        }

        pub fn clear(&mut self) {
            self.buffer.clear();
        }

        pub fn has_pending(&self) -> bool {
            !self.buffer.is_empty()
        }

        pub fn path(&self) -> &str {
            &self.path
        }

        /// Consume the buffer and return it as bytes for writing to SSD.
        pub fn drain(&mut self) -> Vec<u8> {
            let bytes = self.buffer.as_bytes().to_vec();
            self.buffer.clear();
            bytes
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

    /// Ring buffer for in-memory logging (before SSD is available).
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
}
