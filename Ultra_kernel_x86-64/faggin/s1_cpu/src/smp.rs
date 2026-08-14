//! **BRINGING UP THE OTHER CORES** -- the AP trampoline, written by hand.
//!
//! === Why this is a file of its own, and it is the least like its neighbours ===
//!
//! Because an application processor **starts in 16-bit real mode** while the
//! machine around it is already in 64-bit long mode. The trampoline has to walk
//! it through 16 -> 32 -> 64 with hand-assembled far jumps, its own descriptor
//! tables and the kernel's `CR3`, and none of that can be expressed in Rust.
//!
//! ** The previous version of this failed for four reasons at once: the
//! trampoline was assembled as 64-bit code for a CPU that starts in 16, the page
//! tables overlapped, the counter lived inside the PML4, and the GDT had no
//! 32-bit data segment. It was also placed before `ExitBootServices`.
//!
//! It came up `12 of 12` first try on the Ryzen once rewritten -- and the check
//! that made that possible was reading the assembled bytes back out of the ELF
//! and confirming they were **16-bit code with zero REX.W prefixes**.

#[allow(unused_imports)]
use crate::*;

// ===================================================================
//  AMD SMP STARTUP (Zen 3: 6C/12T on Ryzen 5 5600X)
// ===================================================================
//
// Everything SMP-related lives here, written as inline ASM via
// `#[naked]` + `naked_asm!` -- no global_asm, no separate sections,
// no linker relocation issues.
//
// AP startup flow:
//   1. BSP builds a minimal PML4 (identity-mapped 0..4GB) at 0x7000
//   2. BSP copies the SMP trampoline to physical 0x8000
//   3. BSP writes IDT pointer to 0x8138 (shared with APs)
//   4. BSP initializes the LAPIC (MSR 0x1B, SIVR, TPR)
//   5. BSP sends INIT IPI + 2x SIPI to each AP (APIC IDs 0..15)
//   6. APs wake at 0x8000, transition 16->32->64-bit, jump to ap_entry
//   7. APs signal online via atomic counter, halt
//   8. BSP waits for all APs to come online
//
// Memory layout (all identity-mapped by PML4):
//   0x7000-0x7FFF: PML4 (4KB) + online counter at 0x7FF8
//   0x8000-0x80FF: Trampoline (256 bytes)
//   0x8100-0x81FF: Shared BSP<->AP data
//   0x8200-...:    Per-AP stacks (4KB each)

// -- Shared GDT (BSP + APs use the same one) ---------------------

#[repr(C, align(16))]
pub struct SmpGdt { entries: [u64; 4] } // null + 16-bit code + 32-bit code + 64-bit code
pub static mut SMP_GDT: SmpGdt = SmpGdt { entries: [
    0,                                  // null
    0x0000_9B00_0000_FFFFu64,           // 16-bit code, DPL=0, base=0, limit=64K
    0x00CF_9A00_0000_FFFFu64,           // 32-bit code, DPL=0, base=0, limit=4G
    0x0020_9B00_0000_0000u64,           // 64-bit code, DPL=0 (32-bit entry, L=1)
] };

#[repr(C, packed)]
pub struct SmpGdtr { limit: u16, base: u64 }
pub static mut SMP_GDTR: SmpGdtr = SmpGdtr { limit: 31, base: 0 };

// -- AP entry: naked function with 16->32->64 transition -----------

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.ap_entry")]
#[unsafe(naked)]
pub unsafe extern "C" fn ap_entry() {
    // The AP starts here in 16-bit real mode after receiving SIPI.
    // We transition through 32-bit protected mode to 64-bit long mode,
    // then jump to the 64-bit AP entry point (passed via shared data).
    //
    // Memory layout (physical addresses):
    //   0x7000: PML4 (BSP-built, identity-mapped 0..4GB)
    //   0x8058: GDT pointer (10 bytes: limit + base)
    //   0x8060: GDT (32 bytes: 4 entries)
    //   0x8100: PML4 address (u64, set by BSP)
    //   0x8108: Stack top (u64, per-AP)
    //   0x8110: 64-bit AP entry point (u64, set by BSP)
    //   0x8138: IDT pointer (10 bytes: limit + base, set by BSP)
    //   0x7FF8: Online counter (u32, atomic)
    naked_asm!(
        // === 16-bit real mode -> 32-bit protected mode ===
        // Load GDT pointer from 0x8058 (BSP wrote it there)
        // In 64-bit mode, lgdt needs an 80-bit memory operand (10 bytes)
        // We use a register to avoid the 16-bit mode encoding issue.
        "mov rax, 0x8058",
        "lgdt [rax]",

        // Far jump to 32-bit code segment (selector 0x10)
        // 0x68 = push imm32, 0x6A = push imm8
        "push 0x10",                     // 32-bit code selector
        "push offset pmode32",           // entry point in 32-bit mode
        "retfq",

        // === 32-bit protected mode ===
        "pmode32:",
        "mov ax, 0x18",                  // 32-bit data selector
        "mov ds, ax",
        "mov es, ax",
        "mov ss, ax",
        "mov esp, 0x8F00",              // temporary stack

        // Enable PAE (required for long mode)
        "mov rax, cr4",
        "or eax, 0x20",                  // CR4.PAE = bit 5
        "mov cr4, rax",

        // Load PML4 from BSP-built page tables
        "mov rax, [0x8100]",            // PML4 address
        "mov cr3, rax",

        // Enable long mode in EFER (MSR 0xC0000080)
        "mov rcx, 0xC0000080",
        "rdmsr",
        "or eax, 0x901",                 // LME (bit 8) + NXE (bit 11)
        "wrmsr",

        // Enable paging (activates long mode)
        "mov rax, cr0",
        "or eax, 0x80000000",            // CR0.PG = bit 31
        "mov cr0, rax",

        // Reload GDT (64-bit descriptors)
        "mov rax, 0x8058",
        "lgdt [rax]",

        // Far jump to 64-bit code segment (selector 0x20)
        "push 0x20",                     // 64-bit code selector
        "push offset pmode64",           // entry point in 64-bit mode
        "retfq",

        // === 64-bit long mode ===
        "pmode64:",
        // Clear segment registers (not used in 64-bit mode)
        "xor ax, ax",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",

        // Load IDT from BSP (BSP wrote the IDT pointer to 0x8138)
        "mov rax, 0x8138",
        "lidt [rax]",

        // Set up per-AP stack (BSP wrote the stack top to 0x8108)
        "mov rsp, [0x8108]",

        // Jump to the 64-bit AP entry point (BSP wrote it to 0x8110)
        "mov rax, [0x8110]",
        "jmp rax",
    );
}

// -- 64-bit AP entry: signals online, then halts ------------------

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.ap_entry64")]
pub unsafe extern "C" fn ap_entry64() {
    // Signal online: atomic increment of counter at 0x7FF8
    unsafe {
        let counter = 0x7FF8 as *mut u32;
        core::arch::asm!(
            "lock inc dword ptr [{}]",
            in(reg) counter,
            options(nostack, preserves_flags),
        );
    }
    // Halt and wait for the kernel to wake us
    loop {
        unsafe { asm!("hlt"); }
    }
}

// -- LAPIC (Local APIC) -------------------------------------------

pub unsafe fn lapic_base() -> u64 {
    let lo: u32; let hi: u32;
    asm!("rdmsr", in("ecx") 0x1Bu32, out("eax") lo, out("edx") hi);
    ((hi as u64) << 32) | (lo as u64) & 0xFFFFF000
}

pub unsafe fn lapic_write(reg: u32, val: u32) {
    let base = lapic_base() as *mut u32;
    core::ptr::write_volatile(base.add(reg as usize), val);
}

pub unsafe fn lapic_read(reg: u32) -> u32 {
    let base = lapic_base() as *const u32;
    core::ptr::read_volatile(base.add(reg as usize))
}

pub unsafe fn lapic_id() -> u32 {
    (lapic_read(0x020) >> 24) & 0xFF
}

pub unsafe fn lapic_init() {
    let lo: u32; let hi: u32;
    asm!("rdmsr", in("ecx") 0x1Bu32, out("eax") lo, out("edx") hi);
    asm!("wrmsr", in("ecx") 0x1Bu32, in("eax") lo | (1 << 11), in("edx") hi);
    lapic_write(0x0F0, 0x100 | 0xFF);  // SIVR: enable + spurious vector 0xFF
    lapic_write(0x080, 0);            // TPR: accept all interrupts
}

pub unsafe fn send_init_ipi(apic_id: u32) {
    let icr = 0x000C4500u32 | ((apic_id & 0xFF) << 24);
    lapic_write(0x310, (apic_id >> 8) & 0xFF);
    lapic_write(0x300, icr);
    while lapic_read(0x300) & (1 << 12) != 0 {}
}

pub unsafe fn send_sipi(apic_id: u32, vector: u8) {
    let icr = 0x000C4600u32 | ((apic_id & 0xFF) << 24) | (vector as u32);
    lapic_write(0x310, (apic_id >> 8) & 0xFF);
    lapic_write(0x300, icr);
    while lapic_read(0x300) & (1 << 12) != 0 {}
}

// -- PML4 setup (minimal identity-mapped 0..4GB) ------------------

pub unsafe fn setup_smp_pml4() {
    let pml4 = 0x7000 as *mut u64;
    for i in 0..512 { core::ptr::write_volatile(pml4.add(i), 0); }
    let pdpt = 0x7100 as *mut u64;
    for i in 0..512 { core::ptr::write_volatile(pdpt.add(i), 0); }
    let pd = 0x7200 as *mut u64;
    for i in 0..512 { core::ptr::write_volatile(pd.add(i), 0); }
    core::ptr::write_volatile(pml4, 0x7103);  // PML4[0] = PDPT
    core::ptr::write_volatile(pdpt, 0x7203);  // PDPT[0] = PD
    for i in 0..2048usize {
        let entry = (i as u64 * 0x200000) | 0x83;  // HUGE | PRESENT | WRITABLE
        core::ptr::write_volatile(pd.add(i), entry);
    }
}

// -- Trampoline copy: copies the ap_entry naked function to 0x8000 -

pub unsafe fn copy_trampoline() {
    let src = ap_entry as *const u8;
    let dst = 0x8000 as *mut u8;
    // The trampoline (ap_entry naked function) is about 150 bytes.
    // We copy 256 bytes to be safe and to include any padding.
    const TRAMPOLINE_SIZE: usize = 256;
    for i in 0..TRAMPOLINE_SIZE {
        core::ptr::write_volatile(dst.add(i), core::ptr::read_volatile(src.add(i)));
    }
}

// -- TSC-based delay ----------------------------------------------

pub fn rdtsc() -> u64 {
    let lo: u32; let hi: u32;
    unsafe { asm!("rdtsc", out("eax") lo, out("edx") hi); }
    ((hi as u64) << 32) | (lo as u64)
}

pub fn delay_ms(ms: u32) {
    let freq = unsafe { CPU.tsc_freq };
    let start = rdtsc();
    let target = start + (freq * ms as u64) / 1000;
    while rdtsc() < target {
        core::hint::spin_loop();
    }
}

// -- SMP startup (BSP wakes all APs via INIT+SIPI) ----------------

pub unsafe fn smp_startup() {
    ser_print!("\n[s1_cpu] === AMD SMP STARTUP (Zen 3) ===\n");

    // 1. Build minimal PML4 (identity-mapped 0..4GB)
    setup_smp_pml4();
    ser_print!("[s1_cpu] PML4 at 0x7000\n");

    // 2. Copy trampoline (the ap_entry naked function) to physical 0x8000
    copy_trampoline();
    ser_print!("[s1_cpu] trampoline at 0x8000\n");

    // 3. Write GDT pointer to 0x8058 (APs load it with lgdt [0x8058])
    let gdt_base = core::ptr::addr_of!(SMP_GDT) as u64;
    core::ptr::write_volatile(0x8058 as *mut u16, 31u16);  // limit = 31 (4 entries x 8 - 1)
    core::ptr::write_volatile(0x805A as *mut u64, gdt_base);  // base

    // 4. Write IDT pointer to 0x8138 (APs load it with lidt [0x8138])
    let idt_limit = (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16;
    let idt_base = core::ptr::addr_of!(IDT) as u64;
    core::ptr::write_volatile(0x8138 as *mut u16, idt_limit);
    core::ptr::write_volatile(0x813A as *mut u64, idt_base);

    // 5. Initialize online counter at 0x7FF8
    core::ptr::write_volatile(0x7FF8 as *mut u32, 0);

    // 6. Initialize LAPIC
    lapic_init();
    ser_print!("[s1_cpu] LAPIC enabled\n");

    // 7. Get BSP APIC ID
    let bsp_id = lapic_id();
    ser_print!("[s1_cpu] BSP APIC ID: ");
    ser_dec!(bsp_id as usize);
    ser_print!("\n");

    // 8. For each possible APIC ID (0..15), send INIT+SIPI
    let cpu = unsafe { &CPU };
    let num_threads = (cpu.threads_per_core as u32) * (cpu.cores_per_ccx as u32);
    ser_print!("[s1_cpu] Expected threads: ");
    ser_dec!(num_threads as usize);
    ser_print!("\n");

    for apic_id in 0..16u32 {
        if apic_id == bsp_id { continue; }

        // Write per-AP data to shared memory at 0x8100
        core::ptr::write_volatile(0x8100 as *mut u64, 0x7000u64);  // PML4
        let stack_top = 0x8200u64 + (apic_id as u64) * 0x1000 + 0x1000;
        core::ptr::write_volatile(0x8108 as *mut u64, stack_top);
        core::ptr::write_volatile(0x8110 as *mut u64, ap_entry64 as *const () as u64);

        ser_print!("[s1_cpu] waking AP ");
        ser_dec!(apic_id as usize);
        ser_print!("...");

        // INIT IPI -> 10ms wait -> SIPI #1 -> 1ms wait -> SIPI #2 -> 1ms wait
        send_init_ipi(apic_id);
        delay_ms(10);
        send_sipi(apic_id, 8);  // vector=8 -> address 0x8000
        delay_ms(1);
        send_sipi(apic_id, 8);
        delay_ms(1);
    }

    // 9. Wait for all APs to come online
    let expected_aps = num_threads - 1;
    ser_print!("[s1_cpu] waiting for ");
    ser_dec!(expected_aps as usize);
    ser_print!(" APs...\n");
    let mut online: u32 = 0;
    let mut timeout: u32 = 1000;
    while online < expected_aps && timeout > 0 {
        online = core::ptr::read_volatile(0x7FF8 as *const u32);
        if online < expected_aps {
            delay_ms(1);
            timeout -= 1;
        }
    }
    let total = online + 1;  // +1 for BSP
    ser_print!("[s1_cpu] SMP: ");
    ser_dec!(total as usize);
    ser_print!(" / ");
    ser_dec!(num_threads as usize);
    ser_print!(" threads online");
    if total < num_threads {
        ser_print!(" (timeout)");
    }
    ser_print!("\n");
}

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
