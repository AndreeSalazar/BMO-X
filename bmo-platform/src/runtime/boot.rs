//! # The Ring 3 boot path
//!
//! This module is called once per userland process, exactly once,
//! at process spawn time. It:
//!
//! 1. Takes the kernel's `BootContext` pointer (in `rdi` per the
//!    System V AMD64 convention).
//! 2. Builds a [`BootContextV1`] — a typed, CPU-agnostic view
//!    over the kernel's handoff struct.
//! 3. Builds a [`PlatformInfo`] — the platform's identity (CPU
//!    vendor, brand, TSC freq, logical core count).
//! 4. Initializes the active [`crate::arch::Arch`] impl and stashes
//!    it in the platform's `static` storage.
//! 5. Maps the four standard estuary pages into the userland
//!    address space and returns typed [`Estuary`] handles to
//!    the caller.
//!
//! After `boot()` returns, userland code is free to use the
//! `bmo_platform` API without any further setup.

use boot_context::BootContext as KernelBootContext;
use bmo_channel::Channel;

use crate::arch::{self, Arch};
use crate::channel::{
    Estuary, FramebufferEstuary, InputEstuary, LogEstuary, SyscallEstuary,
};

/// The CPU-agnostic view of the kernel's handoff struct.
///
/// This is what the userland sees. The kernel's full
/// `BootContext` has x86-64-specific fields (`ioapic_base`,
/// `hpet_base`, `tsc_freq` from the TSC, `cr3`, etc.). The
/// `BootContextV1` strips those out and adds the channel-page
/// addresses that `bmo-platform` requires.
#[derive(Debug, Clone, Copy)]
pub struct BootContextV1 {
    /// Monotonic clock frequency in Hz (TSC on x86-64, CNTVCT on aarch64).
    pub clock_freq_hz: u64,
    /// Number of logical cores visible to this process.
    pub logical_cores: u32,
    /// Physical address of the first estuary page (Input). The
    /// kernel maps this into the userland at `USER_ESTUARY_BASE`.
    pub estuary_input: u64,
    /// Physical address of the Framebuffer estuary.
    pub estuary_framebuffer: u64,
    /// Physical address of the Syscall estuary.
    pub estuary_syscall: u64,
    /// Physical address of the Log estuary.
    pub estuary_log: u64,
    /// Framebuffer base address (legacy field, also in `info.rs`).
    pub fb_addr: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    pub fb_stride: u32,
    /// Address of the kernel's `syscall` entry stub. The
    /// `Arch::syscall` method on x86_64 jumps to this address
    /// on every synchronous syscall. (aarch64 would use `svc #0`
    /// which doesn't need a preconfigured address.)
    pub syscall_entry: u64,
}

/// Information returned by [`boot`] about the platform itself.
#[derive(Debug, Clone, Copy)]
pub struct PlatformInfo {
    /// Architecture name (e.g. `"x86_64"`).
    pub arch: &'static str,
    /// CPU vendor (e.g. `"AuthenticAMD"`).
    /// Borrowed from the platform's static storage; valid for the
    /// program's lifetime.
    pub vendor: &'static str,
    /// CPU brand (e.g. `"AMD Ryzen 5 5600X 6-Core Processor"`).
    /// Borrowed from the platform's static storage; valid for the
    /// program's lifetime.
    pub brand: &'static str,
    /// Number of logical cores.
    pub logical_cores: u32,
    /// Clock frequency in Hz.
    pub clock_freq_hz: u64,
}

/// The four standard estuaries, ready to use after [`boot`].
pub struct Estuaries {
    /// Input events (keyboard, mouse) flowing Ring 0 → Ring 3.
    pub input: InputEstuary<'static>,
    /// Framebuffer draw commands flowing Ring 3 → Ring 0.
    pub framebuffer: FramebufferEstuary<'static>,
    /// Async syscalls flowing Ring 3 → Ring 0.
    pub syscall: SyscallEstuary<'static>,
    /// Kernel log lines flowing Ring 0 → Ring 3.
    pub log: LogEstuary<'static>,
}

/// Boot the platform layer. Called once at process spawn.
///
/// # Arguments
/// `ctx_ptr` — a non-null pointer to the kernel's `BootContext`,
/// passed in `rdi` by the kernel's process-spawn code.
///
/// # Returns
/// A `(PlatformInfo, Estuaries)` tuple. The `PlatformInfo` is a
/// read-only snapshot of the platform's identity. The `Estuaries`
/// are the four standard typed channels, ready to `send`/`poll`.
///
/// # Safety
/// The caller (typically `_start` from `bmo-rt`) must ensure that
/// `ctx_ptr` is a valid pointer to a fully initialized kernel
/// `BootContext` with the correct magic value. The channel pages
/// pointed to by the `BootContext` must be mapped read-write at
/// the addresses the kernel placed them.
pub unsafe fn boot(
    ctx_ptr: *const KernelBootContext,
) -> (PlatformInfo, Estuaries) {
    // 1. Validate the kernel handoff.
    let ctx = &*ctx_ptr;
    if !ctx.is_valid() {
        panic!("bmo-platform: kernel BootContext magic mismatch");
    }

    // 2. Read CPUID from the kernel's fields. The kernel's
    // `BootContext` doesn't yet have a `cpu_vendor` field; for
    // v0.1 we read it from the context. The cleanest production
    // path is to add `cpu_vendor: [u8; 12]` and `cpu_brand: [u8; 48]`
    // to `BootContext` in a future revision.
    let (vendor_bytes, brand_bytes) = read_cpuid_strings();

    // 3. Build the platform's identity.
    let platform = crate::arch::x86_64::X86_64::from_handoff(
        ctx.tsc_freq,
        vendor_bytes,
        brand_bytes,
        // CPUID.1:EBX[23:16] gives logical core count. We read it
        // directly here; in a future revision this comes from ctx.
        read_logical_cores(),
        ctx.syscall_entry,
    );

    // 4. Install the Arch impl in the platform's static storage
    //    BEFORE we read the identity fields, so we can borrow
    //    `vendor` and `brand` from the static.
    arch::install(platform);

    // 5. Read the identity fields through the now-static reference.
    //    `arch::current()` returns `&'static dyn Arch`; the strings
    //    it returns point into the static storage and so are
    //    `&'static str`.
    let arch_ref = arch::current();
    let info = PlatformInfo {
        arch: arch_ref.name(),
        vendor: arch_ref.vendor(),
        brand: arch_ref.brand(),
        logical_cores: arch_ref.logical_cores(),
        clock_freq_hz: ctx.tsc_freq,
    };

    // 5. Map the four standard estuary pages.
    //
    //    In a production kernel, the userland's PML4 already
    //    has these pages mapped at fixed addresses. The kernel
    //    publishes the physical addresses in
    //    `BootContext.channel_pages[]`:
    //      [0] = Input
    //      [1] = Framebuffer
    //      [2] = Syscall
    //      [3] = Log
    //      [4..15] = available for custom estuaries
    let pages = &ctx.channel_pages;
    let estuary_input_phys      = pages[0];
    let estuary_framebuffer_phys = pages[1];
    let estuary_syscall_phys    = pages[2];
    let estuary_log_phys        = pages[3];

    // Convert the physical addresses to virtual. On x86-64 the
    // kernel maps everything to the higher half at
    // 0xFFFF_8000_0000_0000 + phys, AND it identity-maps the
    // first 4 MB. The estuary pages are in the post-kernel
    // region (>0x400000), so the userland sees them at
    // 0xFFFF_8000_0000_0000 + phys.
    let higher_half_base: u64 = 0xFFFF_8000_0000_0000;
    let input_ch      = &*((higher_half_base + estuary_input_phys)      as *const Channel);
    let framebuf_ch   = &*((higher_half_base + estuary_framebuffer_phys) as *const Channel);
    let syscall_ch    = &*((higher_half_base + estuary_syscall_phys)    as *const Channel);
    let log_ch        = &*((higher_half_base + estuary_log_phys)        as *const Channel);

    // 6. Validate each channel's magic. A wrong page here usually
    //    means the kernel's channel-page publication is broken.
    let input_e   = Estuary::from_raw(input_ch);
    let framebuf_e = Estuary::from_raw(framebuf_ch);
    let syscall_e  = Estuary::from_raw(syscall_ch);
    let log_e      = Estuary::from_raw(log_ch);
    if !input_e.is_valid() {
        panic!("bmo-platform: Input estuary page has wrong magic");
    }
    if !framebuf_e.is_valid() {
        panic!("bmo-platform: Framebuffer estuary page has wrong magic");
    }
    if !syscall_e.is_valid() {
        panic!("bmo-platform: Syscall estuary page has wrong magic");
    }
    if !log_e.is_valid() {
        panic!("bmo-platform: Log estuary page has wrong magic");
    }

    let estuaries = Estuaries {
        input: input_e,
        framebuffer: framebuf_e,
        syscall: syscall_e,
        log: log_e,
    };

    (info, estuaries)
}

// ── CPUID helpers (called once at boot) ──────────────────────────

/// Run CPUID leaves 0 + 0x80000002..0x80000004 to get the vendor
/// and brand strings. Returns `(vendor[12], brand[48])`.
fn read_cpuid_strings() -> ([u8; 12], [u8; 48]) {
    use core::arch::asm;
    let mut vendor = [0u8; 12];
    let mut brand = [0u8; 48];

    // Leaf 0: max_leaf, ebx, ecx, edx (vendor).
    let (_max, ebx, ecx, edx): (u32, u32, u32, u32);
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") 0u32 => _,
            inout("ecx") 0u32 => ecx,
            ebx_out = out(reg) ebx,
            out("edx") edx,
        );
    }
    vendor[0..4].copy_from_slice(&ebx.to_ne_bytes());
    vendor[4..8].copy_from_slice(&edx.to_ne_bytes());
    vendor[8..12].copy_from_slice(&ecx.to_ne_bytes());

    // Leaves 0x80000002..0x80000004: brand string (48 bytes).
    let mut idx = 0;
    for &leaf in &[0x80000002u32, 0x80000003, 0x80000004] {
        let (a, b, c, d): (u32, u32, u32, u32);
        unsafe {
            asm!(
                "push rbx",
                "cpuid",
                "mov {ebx_out:e}, ebx",
                "pop rbx",
                inout("eax") leaf => a,
                inout("ecx") 0u32 => c,
                ebx_out = out(reg) b,
                out("edx") d,
            );
        }
        for v in [a, b, c, d] {
            if idx < 48 { brand[idx] = v as u8; idx += 1; }
            if v > 0xFF && idx < 48 { brand[idx] = (v >> 8) as u8; idx += 1; }
            if v > 0xFFFF && idx < 48 { brand[idx] = (v >> 16) as u8; idx += 1; }
            if v > 0xFFFFFF && idx < 48 { brand[idx] = (v >> 24) as u8; idx += 1; }
        }
    }
    (vendor, brand)
}

/// Read CPUID.1:EBX[23:16] for the logical core count.
fn read_logical_cores() -> u32 {
    use core::arch::asm;
    let ebx: u32;
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") 1u32 => _,
            inout("ecx") 0u32 => _,
            ebx_out = out(reg) ebx,
            out("edx") _,
        );
    }
    (ebx >> 16) & 0xFF
}
