//! vDSO — Virtual Dynamic Shared Object page for Ring 3.
//!
//! A read-only page mapped into every Ring 3 process containing
//! frequently-queried kernel data. Avoids syscall overhead for
//! time queries, TSC info, framebuffer data, and system caps.
//!
//! Timer interrupt updates `monotonic_tsc` periodically.
//! All other fields are static (set once at boot).

use core::sync::atomic::{AtomicU64, Ordering};

/// Layout of the vDSO page. Must be `#[repr(C)]` for stable ABI.
/// Ring 3 reads this at a well-known virtual address.
#[repr(C)]
pub struct VdsoPage {
    /// TSC frequency in Hz (static, set at boot).
    pub tsc_freq_hz: u64,
    /// TSC-based monotonic timestamp, updated by timer IRQ ~1ms.
    pub monotonic_tsc: AtomicU64,
    /// Realtime nanoseconds since boot, updated by timer IRQ.
    pub realtime_ns: AtomicU64,
    /// Framebuffer physical address.
    pub fb_addr: u64,
    /// Framebuffer width in pixels.
    pub fb_width: u32,
    /// Framebuffer height in pixels.
    pub fb_height: u32,
    /// Framebuffer stride in pixels.
    pub fb_stride: u32,
    /// Memory page size (always 4096).
    pub page_size: u32,
    /// Total usable RAM in MB.
    pub total_ram_mb: u64,
    /// Kernel version as packed bytes.
    pub kernel_version: [u8; 8],
    /// Feature flags bitmap (syscall groups available).
    pub feature_flags: u64,
}

/// Feature flags exposed to Ring 3 via vDSO.
pub const FEATURE_NET: u64    = 1 << 0;
pub const FEATURE_GPU: u64    = 1 << 1;
pub const FEATURE_IPC: u64    = 1 << 2;
pub const FEATURE_AUDIO: u64  = 1 << 3;

/// Physical address of the vDSO page.
static mut VDSO_PHYS: u64 = 0;

/// Virtual (kernel-side) pointer to the mapped vDSO page.
static mut VDSO_PTR: *mut VdsoPage = core::ptr::null_mut();

/// Initialize the vDSO page: allocate physical page, map in kernel space,
/// fill static fields. Called during Phase 1 (memory init).
pub fn init() {
    let page = match unsafe { crate::mm::frame_alloc::alloc_pages_contiguous(1) } {
        Some(phys) => phys,
        None => {
            crate::dev::console::serial_write("[vdso] FAIL: no physical page\n");
            return;
        }
    };

    unsafe {
        // Zero the page
        let virt = crate::mm::vmm::phys_to_virt(page) as *mut u8;
        core::ptr::write_bytes(virt, 0, 4096);

        // Initialize static fields
        let vdso = &mut *(virt as *mut VdsoPage);
        vdso.tsc_freq_hz = crate::cpu::tsc_per_sec();
        vdso.monotonic_tsc = AtomicU64::new(crate::cpu::rdtsc());
        vdso.realtime_ns = AtomicU64::new(0);
        vdso.fb_addr = crate::info::FB_ADDR;
        vdso.fb_width = crate::info::FB_WIDTH;
        vdso.fb_height = crate::info::FB_HEIGHT;
        vdso.fb_stride = crate::info::FB_STRIDE;
        vdso.page_size = 4096;
        vdso.total_ram_mb = crate::mm::frame_alloc::total_ram() / (1024 * 1024);
        vdso.kernel_version = *b"2.0.0   ";
        vdso.feature_flags = 0
            | if cfg!(feature = "syscalls-net") { FEATURE_NET } else { 0 }
            | if cfg!(feature = "syscalls-gpu") { FEATURE_GPU } else { 0 }
            | if cfg!(feature = "syscalls-ipc") { FEATURE_IPC } else { 0 }
            | if cfg!(feature = "syscalls-audio") { FEATURE_AUDIO } else { 0 };

        VDSO_PHYS = page;
        VDSO_PTR = virt as *mut VdsoPage;
    }

    crate::dev::console::serial_write("[vdso] page initialized at phys=0x");
    crate::dev::console::serial_write_u64(unsafe { VDSO_PHYS }, 16);
    crate::dev::console::serial_write("\n");
}

/// Update the vDSO time fields. Called from timer interrupt.
pub fn tick() {
    unsafe {
        if let Some(vdso) = VDSO_PTR.as_mut() {
            vdso.monotonic_tsc.store(crate::cpu::rdtsc(), Ordering::Relaxed);
            vdso.realtime_ns.store(crate::dev::timer::now_ns(), Ordering::Relaxed);
        }
    }
}

/// Get the physical address of the vDSO page (for user page table mapping).
pub fn phys() -> u64 {
    unsafe { VDSO_PHYS }
}
