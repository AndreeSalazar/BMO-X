#![allow(dead_code, unused_unsafe)]

//! GDT + TSS for x86-64 Long Mode — Ring 0 / Ring 3 support.
//!
//! Layout:
//!   0x00: Null descriptor
//!   0x08: Kernel Code (Ring 0, 64-bit, CS)
//!   0x10: Kernel Data (Ring 0, SS/DS/ES)
//!   0x18: User Data   (Ring 3, SS/DS/ES)  — MUST come before User Code for sysret
//!   0x20: User Code   (Ring 3, 64-bit, CS)
//!   0x28: TSS low  (16 bytes total)
//!   0x30: TSS high

use core::arch::asm;
use core::mem::size_of;

/// Kernel code segment selector.
pub const KERNEL_CS: u16 = 0x08;
/// Kernel data segment selector.
pub const KERNEL_DS: u16 = 0x10;
/// User data segment selector (RPL=3).
pub const USER_DS: u16 = 0x18 | 3;
/// User code segment selector (RPL=3).
pub const USER_CS: u16 = 0x20 | 3;
/// TSS segment selector.
pub const TSS_SEL: u16 = 0x28;

/// Task State Segment — x86-64 Long Mode (104 bytes minimum).
#[repr(C, packed)]
pub struct Tss {
    _reserved0: u32,
    /// Ring 0 stack pointers (RSP0-RSP2). RSP0 is used on Ring 3 → Ring 0 transitions.
    pub rsp: [u64; 3],
    _reserved1: u64,
    /// Interrupt Stack Table (IST1-IST7).
    pub ist: [u64; 7],
    _reserved2: u64,
    _reserved3: u16,
    /// I/O Map Base Address (offset to I/O permission bitmap).
    pub iomap_base: u16,
}

/// IST1 stack for #PF/#GP/#DF exceptions (8 KB, 16-byte aligned).
///
/// AMD Zen 3 may push up to 6 exception frames onto this stack before
/// invoking the handler (e.g., a #DF that occurs while handling a
/// #GP). 4 KB is the minimum, but 8 KB gives margin to avoid a
/// triple-fault if the chain is long.
#[repr(align(16))]
struct Ist1Stack([u8; 8192]);
static mut IST1_STACK: Ist1Stack = Ist1Stack([0; 8192]);

impl Tss {
    pub const fn new() -> Self {
        Self {
            _reserved0: 0,
            rsp: [0; 3],
            _reserved1: 0,
            ist: [0; 7],
            _reserved2: 0,
            _reserved3: 0,
            iomap_base: size_of::<Tss>() as u16,
        }
    }
}

/// GDT with 7 entries (null + 4 segments + TSS 16-byte).
#[repr(C, align(16))]
struct Gdt {
    entries: [u64; 7],
}

/// GDTR for lgdt instruction.
#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base: u64,
}

// Static globals
static mut TSS: Tss = Tss::new();
static mut GDT: Gdt = Gdt { entries: [0; 7] };

/// Kernel stack for Ring 3 → Ring 0 transitions (16 KB, 16-byte aligned).
#[repr(align(16))]
struct KernelStack([u8; 16384]);
static mut KERNEL_STACK: KernelStack = KernelStack([0; 16384]);

/// Build a 64-bit code/data segment descriptor.
const fn make_segment(dpl: u8, code: bool) -> u64 {
    let mut d: u64 = 0;
    // Limit 0xFFFFF (bits 0-15 and 48-51)
    d |= 0xFFFF; // limit low
    d |= 0x0F << 48; // limit high
    // Access byte (bit 40-47):
    //   bit 47: Present = 1
    //   bit 45-46: DPL
    //   bit 44: Descriptor type = 1 (code/data)
    //   bit 43: Executable
    //   bit 41: Read/Write = 1
    //   bit 40: Accessed = 0
    let mut access: u8 = 0x92; // Present + Read/Write + Descriptor type
    access |= dpl << 5;
    if code {
        access |= 0x08; // Executable
    }
    d |= (access as u64) << 40;
    // Flags (bit 52-55):
    //   bit 55: Granularity = 1 (4KB pages)
    //   bit 53: Long mode = 1 (for code segments)
    //   bit 54: Size = 0 (must be 0 for long mode 64-bit)
    let mut flags: u8 = 0x0A; // Granularity + Long mode
    if !code {
        flags = 0x0C; // Granularity + Size (32-bit for data)
    }
    d |= (flags as u64) << 52;
    d
}

/// Build TSS descriptor (16 bytes = 2 u64 entries).
fn make_tss_descriptor(tss_addr: u64, tss_size: u16) -> (u64, u64) {
    let mut low: u64 = 0;
    // Limit (bits 0-15 and 48-51)
    low |= (tss_size as u64) & 0xFFFF;
    low |= (((tss_size as u64) >> 16) & 0x0F) << 48;
    // Base address
    low |= ((tss_addr & 0xFFFF) as u64) << 16;           // base 15:0
    low |= (((tss_addr >> 16) & 0xFF) as u64) << 32;     // base 23:16
    low |= (((tss_addr >> 24) & 0xFF) as u64) << 56;     // base 31:24
    // Access: Present + 64-bit TSS Available (type = 0x9)
    low |= 0x89u64 << 40; // Present + Type 9 (64-bit TSS available)
    // High 8 bytes: base 63:32
    let high: u64 = (tss_addr >> 32) & 0xFFFFFFFF;
    (low, high)
}

/// Update TSS.rsp0 — called on every context switch to set the kernel stack
/// for the next Ring 3 → Ring 0 transition.
pub fn set_kernel_stack(rsp0: u64) {
    unsafe { TSS.rsp[0] = rsp0; }
}

/// Return the top of the global kernel stack (16-byte aligned).
///
/// Used as a fallback stack for the syscall entry if no per-thread kernel
/// stack has been configured yet (i.e., before the first Ring 3 process
/// is spawned).
#[inline(always)]
pub fn kernel_stack_top() -> u64 {
    unsafe { core::ptr::addr_of!(KERNEL_STACK) as u64 + 16384 }
}

/// Initialize GDT with Ring 0/3 segments and TSS, then load via LGDT.
pub fn init_gdt() {
    unsafe {
        // Set RSP0 to top of kernel stack
        let stack_top = core::ptr::addr_of!(KERNEL_STACK) as u64 + 16384;
        TSS.rsp[0] = stack_top;

        // Set IST1 for #PF/#GP/#DF exception handling (dedicated stack).
        // Size matches Ist1Stack::[u8; 8192] above.
        let ist1_top = core::ptr::addr_of!(IST1_STACK) as u64 + 8192;
        TSS.ist[0] = ist1_top;

        // Build GDT entries
        GDT.entries[0] = 0;                              // 0x00: Null
        GDT.entries[1] = make_segment(0, true);           // 0x08: Kernel Code
        GDT.entries[2] = make_segment(0, false);          // 0x10: Kernel Data
        GDT.entries[3] = make_segment(3, false);          // 0x18: User Data
        GDT.entries[4] = make_segment(3, true);           // 0x20: User Code

        // TSS descriptor (16 bytes)
        let tss_addr = core::ptr::addr_of!(TSS) as u64;
        let (tss_lo, tss_hi) = make_tss_descriptor(tss_addr, (size_of::<Tss>() - 1) as u16);
        GDT.entries[5] = tss_lo;                          // 0x28: TSS low
        GDT.entries[6] = tss_hi;                          // 0x30: TSS high

        // Load GDTR
        let gdtr = Gdtr {
            limit: (size_of::<Gdt>() - 1) as u16,
            base: core::ptr::addr_of!(GDT) as u64,
        };

        asm!(
            "lgdt [{}]",
            in(reg) &gdtr,
            options(nostack)
        );

        // Reload CS via far return
        asm!(
            "push {kernel_cs}",
            "lea {tmp}, [rip + 2f]",
            "push {tmp}",
            "retfq",
            "2:",
            kernel_cs = in(reg) KERNEL_CS as u64,
            tmp = out(reg) _,
            options(nostack),
        );

        // Reload data segment registers
        asm!(
            "mov ds, {0:x}",
            "mov es, {0:x}",
            "mov ss, {0:x}",
            "mov fs, {0:x}",
            "mov gs, {0:x}",
            in(reg) KERNEL_DS as u64,
            options(nostack),
        );

        // Load TSS
        asm!(
            "ltr {0:x}",
            in(reg) TSS_SEL as u64,
            options(nostack),
        );
    }
}
