//! # x86-64 implementation of the `Arch` trait
//!
//! This is the **single** place in `bmo-platform` that uses x86-64
//! inline assembly. Every other module in the crate calls into
//! here through the `Arch` trait surface, which is CPU-agnostic.
//!
//! The `X86_64` struct is filled in once at boot time from the
//! kernel's `BootContext` (TSC frequency, vendor string, brand
//! string, logical core count, the `syscall` MSR address) and then
//! frozen — it is a read-only snapshot of the platform's identity.

use core::arch::asm;

use super::Arch;

/// x86-64 platform impl. One static instance, populated at boot.
pub struct X86_64 {
    /// TSC frequency in Hz (read from the kernel's calibration).
    tsc_freq_hz: u64,
    /// CPUID vendor string, 12 bytes (e.g. "AuthenticAMD").
    vendor: [u8; 12],
    /// CPUID brand string, up to 48 bytes (NUL-terminated).
    brand: [u8; 48],
    /// Logical core count from CPUID.1:EBX[23:16].
    logical_cores: u32,
    /// Address of the kernel's `syscall` entry point. The kernel
    /// writes this into IA32_LSTAR during s8_syscall, and we read
    /// it from `BootContext.syscall_entry`. We then jump to it
    /// on every `syscall()` call from Ring 3.
    syscall_entry: u64,
}

impl X86_64 {
    /// Construct the `X86_64` instance from the kernel's handoff
    /// values. Called once from `runtime::boot`.
    pub(crate) fn from_handoff(
        tsc_freq_hz: u64,
        vendor: [u8; 12],
        brand: [u8; 48],
        logical_cores: u32,
        syscall_entry: u64,
    ) -> Self {
        Self { tsc_freq_hz, vendor, brand, logical_cores, syscall_entry }
    }
}

impl Arch for X86_64 {
    fn name(&self) -> &'static str { "x86_64" }

    fn monotonic_ns(&self, _tsc_freq_hz_unused: u64) -> u64 {
        // We use the stored tsc_freq_hz from the handoff (the
        // argument is for symmetry with future arches that may
        // need it; on x86_64 the freq is captured at boot).
        let tsc: u64;
        unsafe {
            // rdtsc loads EDX:EAX. We use lfence before+after to
            // serialize. The combined EDX:EAX result lands in rax.
            asm!(
                "lfence",
                "rdtsc",
                "lfence",
                out("edx") _,
                lateout("rax") tsc,
                options(nostack, preserves_flags),
            );
        }
        // TSC ticks → nanoseconds using the calibrated frequency.
        let secs = tsc / self.tsc_freq_hz;
        let frac = tsc % self.tsc_freq_hz;
        secs.wrapping_mul(1_000_000_000)
            .wrapping_add((frac.wrapping_mul(1_000_000_000)) / self.tsc_freq_hz)
    }

    fn spin_hint(&self) {
        unsafe { asm!("pause", options(nostack, preserves_flags)); }
    }

    fn idle(&self) -> ! {
        // The platform-agnostic contract: "park until the next
        // interrupt". On x86_64 that is `sti; hlt`. `sti` first
        // so we don't sleep through the interrupt that would
        // wake us. The interrupt handler will iret back here.
        loop {
            unsafe { asm!("sti; hlt", options(nostack)); }
        }
    }

    fn full_fence(&self) {
        unsafe { asm!("mfence", options(nostack, preserves_flags)); }
    }

    fn syscall(&self, nr: u64, args: &[u64; 6]) -> u64 {
        // System V AMD64 syscall convention:
        //   rax = syscall number
        //   rdi, rsi, rdx, r10, r8, r9 = args[0..6]
        //   r11 = saved rflags (kernel clobbers)
        //   rcx = saved rip (kernel clobbers)
        //   return value in rax
        //
        // We use `syscall` directly rather than `int 0x80` because
        // s8_syscall programmed the IA32_LSTAR MSR with a 64-bit
        // `syscall` entry stub. `int 0x80` is 32-bit and would
        // truncate args above 32 bits.
        let rax: u64;
        unsafe {
            asm!(
                "syscall",
                inout("rax") nr => rax,
                in("rdi")  args[0],
                in("rsi")  args[1],
                in("rdx")  args[2],
                in("r10")  args[3],
                in("r8")   args[4],
                in("r9")   args[5],
                lateout("rcx") _,
                lateout("r11") _,
                options(nostack, preserves_flags),
            );
        }
        rax
    }

    fn logical_cores(&self) -> u32 { self.logical_cores }

    fn vendor(&self) -> &str {
        // Find first NUL, or use full 12.
        let len = self.vendor.iter().position(|&b| b == 0).unwrap_or(12);
        // SAFETY: the slice came from a `[u8; 12]` with valid UTF-8
        // ASCII (CPUID only emits A-Z). The slice borrows from
        // `self`, which is in the platform's static storage and
        // thus lives for the program's entire lifetime.
        unsafe {
            core::str::from_utf8_unchecked(&self.vendor[..len])
        }
    }

    fn brand(&self) -> &str {
        let len = self.brand.iter().position(|&b| b == 0).unwrap_or(48);
        // SAFETY: ASCII from CPUID. See vendor() comment.
        unsafe { core::str::from_utf8_unchecked(&self.brand[..len]) }
    }
}
