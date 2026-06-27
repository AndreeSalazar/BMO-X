//! AP Core Startup — INIT/SIPI sequence + trampoline page.
//!
//! Universal x86-64 AP startup. Works on Intel and AMD.
//!
//! The trampoline is a small piece of code + data copied to a
//! low-memory page (< 1 MB) that AP cores execute after SIPI.
//! It sets up 64-bit long mode, loads the GDT, and jumps to
//! the Rust AP entry point.
//!
//! Sequence:
//!   1. BSP sends INIT IPI (edge-triggered, deassert)
//!   2. Wait 10 ms
//!   3. BSP sends SIPI with vector = page >> 12
//!   4. Wait 200 µs
//!   5. If AP didn't start, send second SIPI
//!   6. Wait 200 µs
//!   7. AP executes trampoline → enters Rust → registers per-CPU data

use core::arch::asm;
use super::ipi;
use super::percpu;

/// The AP trampoline page. Must be < 1 MB for real-mode SIPI.
/// We allocate it at a fixed physical address. This page contains:
///   - Real-mode entry point (first 4 KB)
///   - GDT + data (second 4 KB)
///
/// This is copied from the static TRAMPOLINE_CODE at init time.
#[repr(C, align(4096))]
pub struct TrampolinePage {
    /// 4 KB: real-mode code + stack
    pub code: [u8; 4096],
    /// 4 KB: GDT + data
    pub data: [u8; 4096],
}

/// Fixed physical address for the trampoline. Must be < 1 MB.
/// We use 0x8000 (32 KB) — safe, well below the 640 KB barrier.
pub const TRAMPOLINE_PHYS: u64 = 0x8000;

/// Offset of the 64-bit entry point within the trampoline code page.
const ENTRY_OFFSET: u16 = 0x200;

/// AP boot status flags. Written by APs, read by BSP.
pub const AP_BOOT_STATUS_NOT_STARTED: u32 = 0;
pub const AP_BOOT_STATUS_STARTED: u32 = 1;
pub const AP_BOOT_STATUS_64BIT: u32 = 2;
pub const AP_BOOT_STATUS_ERROR: u32 = 0xDEAD;

/// Address of the status word (at offset 0x100 in the trampoline data page).
const STATUS_OFFSET: u16 = 0x100;

/// Stack size for each AP core (64 KB).
const AP_STACK_SIZE: usize = 64 * 1024;

/// Stacks for AP cores. Static allocation (max 128 CPUs).
static mut AP_STACKS: [[u8; AP_STACK_SIZE]; percpu::MAX_CPUS] = [[0u8; AP_STACK_SIZE]; percpu::MAX_CPUS];

/// Address of the AP trampoline code (physical).
static mut TRAMPOLINE_VIRT: u64 = 0;

/// Write a 16-bit value to the trampoline page at a given offset.
unsafe fn trampoline_write16(offset: u16, val: u16) {
    let ptr = (TRAMPOLINE_PHYS + offset as u64) as *mut u16;
    core::ptr::write_volatile(ptr, val);
}

/// Read a 32-bit value from the trampoline data page.
unsafe fn trampoline_read32(offset: u16) -> u32 {
    let ptr = (TRAMPOLINE_PHYS + 4096 + offset as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

/// Write a 32-bit value to the trampoline data page.
unsafe fn trampoline_write32(offset: u16, val: u32) {
    let ptr = (TRAMPOLINE_PHYS + 4096 + offset as u64) as *mut u32;
    core::ptr::write_volatile(ptr, val);
}

/// Copy the trampoline code to low memory. Must be called once at boot,
/// before any AP startup attempts.
///
/// # Safety
/// Writes to physical memory below 1 MB.
pub unsafe fn init_trampoline() {
    // Copy the trampoline code template to the physical page.
    // The trampoline is assembled at compile time as a static byte array.
    let code_src = TRAMPOLINE_CODE.as_ptr();
    let code_dst = TRAMPOLINE_PHYS as *mut u8;
    core::ptr::copy_nonoverlapping(code_src, code_dst, TRAMPOLINE_CODE.len());

    // Copy the GDT template
    let gdt_src = AP_GDT_TEMPLATE.as_ptr();
    let gdt_dst = (TRAMPOLINE_PHYS + 4096) as *mut u8;
    core::ptr::copy_nonoverlapping(gdt_src, gdt_dst, AP_GDT_TEMPLATE.len());

    // Initialize status word to NOT_STARTED
    trampoline_write32(STATUS_OFFSET, AP_BOOT_STATUS_NOT_STARTED);

    // Store the virtual address of the trampoline (identity-mapped)
    TRAMPOLINE_VIRT = TRAMPOLINE_PHYS;

    crate::dev::console::serial_write("[smp] trampoline at 0x");
    crate::serial::hex(TRAMPOLINE_PHYS);
    crate::dev::console::serial_write("\n");
}

/// Start a single AP core using the INIT/SIPI sequence.
///
/// `apic_id`: APIC ID of the target AP.
/// `core_id`: logical core ID to assign.
///
/// Returns `Ok(core_id)` on success, `Err(())` on timeout.
pub unsafe fn start_ap(apic_id: u32, core_id: u32) -> Result<u32, ()> {
    crate::dev::console::serial_write("[smp] starting AP APIC=");
    crate::dev::console::serial_write_u64(apic_id as u64, 10);
    crate::dev::console::serial_write(" core=");
    crate::dev::console::serial_write_u64(core_id as u64, 10);
    crate::dev::console::serial_write("\n");

    // Clear the status word
    trampoline_write32(STATUS_OFFSET, AP_BOOT_STATUS_NOT_STARTED);

    // Set up the stack for this AP
    let stack_top = AP_STACKS[core_id as usize].as_ptr() as u64 + AP_STACK_SIZE as u64;
    // Store stack pointer in trampoline data so AP can find it
    trampoline_write32(0x120, stack_top as u32);
    trampoline_write32(0x124, (stack_top >> 32) as u32);
    // Store core_id
    trampoline_write32(0x130, core_id);
    // Store APIC ID
    trampoline_write32(0x134, apic_id);

    // 1. Send INIT IPI (level-triggered, assert)
    ipi::send_init_ipi(apic_id);

    // 2. Wait 10 ms
    delay_ms(10);

    // 3. Deassert INIT (INIT IPI with delivery mode = INIT, level = deassert)
    ipi::send_init_deinit_apic_ipi();

    // 4. Wait 1 ms
    delay_ms(1);

    // 5. Send SIPI (Startup IPI) with vector = trampoline page >> 12
    let vector = (TRAMPOLINE_PHYS >> 12) as u8;
    ipi::send_sipi(apic_id, vector);

    // 6. Wait 200 µs
    delay_us(200);

    // 7. Check if AP started
    let status = trampoline_read32(STATUS_OFFSET);
    if status >= AP_BOOT_STATUS_STARTED {
        // AP started, wait for it to reach 64-bit mode
        let mut attempts = 0;
        while trampoline_read32(STATUS_OFFSET) < AP_BOOT_STATUS_64BIT && attempts < 1000 {
            delay_us(10);
            attempts += 1;
        }
        if trampoline_read32(STATUS_OFFSET) >= AP_BOOT_STATUS_64BIT {
            crate::dev::console::serial_write("[smp] AP APIC=");
            crate::dev::console::serial_write_u64(apic_id as u64, 10);
            crate::dev::console::serial_write(" online\n");
            return Ok(core_id);
        }
    }

    // 8. Second SIPI attempt
    crate::dev::console::serial_write("[smp] first SIPI failed, retrying\n");
    ipi::send_sipi(apic_id, vector);
    delay_us(200);

    let status = trampoline_read32(STATUS_OFFSET);
    if status >= AP_BOOT_STATUS_64BIT {
        crate::dev::console::serial_write("[smp] AP APIC=");
        crate::dev::console::serial_write_u64(apic_id as u64, 10);
        crate::dev::console::serial_write(" online (2nd SIPI)\n");
        return Ok(core_id);
    }

    crate::dev::console::serial_write("[smp] AP APIC=");
    crate::dev::console::serial_write_u64(apic_id as u64, 10);
    crate::dev::console::serial_write(" FAILED to start (status=");
    crate::dev::console::serial_write_u64(status as u64, 16);
    crate::dev::console::serial_write(")\n");
    Err(())
}

/// AP entry point. Called from the trampoline after entering 64-bit mode.
/// This is a naked function — the trampoline jumps here directly.
#[no_mangle]
pub unsafe extern "sysv64" fn ap_entry() {
    // We're now in 64-bit long mode on the AP core.
    // The trampoline has already set up CR3, GDT, and enabled paging.

    // Read core_id and APIC ID from trampoline data
    let core_id = trampoline_read32(0x130);
    let apic_id = trampoline_read32(0x134);
    let stack_top_lo = trampoline_read32(0x120);
    let stack_top_hi = trampoline_read32(0x124);
    let kernel_stack_top = ((stack_top_hi as u64) << 32) | (stack_top_lo as u64);

    // Set TSC_AUX to APIC ID (for RDTSCP)
    super::percpu::set_kernel_gs_base(0); // Will be set below

    // Set up per-CPU data for this AP
    let core_id_actual = percpu::register_ap(apic_id, kernel_stack_top);

    // Set GS-base to point to this core's PerCpu struct
    let percpu_ptr = percpu::get(core_id_actual).unwrap() as *const percpu::PerCpu as u64;
    percpu::set_gs_base(percpu_ptr);

    // Set IA32_TSC_AUX to APIC ID
    let tsc_aux = (apic_id as u64) | ((core_id_actual as u64) << 32);
    asm!("wrmsr", in("ecx") 0xC0000101u32,
         in("eax") (tsc_aux as u32), in("edx") ((tsc_aux >> 32) as u32),
         options(nostack));

    // Signal that we're in 64-bit mode
    trampoline_write32(STATUS_OFFSET, AP_BOOT_STATUS_64BIT);

    // Set up IDT for this AP
    crate::arch::idt::init_idt();

    // Enable LAPIC on this AP
    let spurious = super::super::apic::apic_read(super::super::apic::APIC_SPURIOUS);
    super::super::apic::apic_write(super::super::apic::APIC_SPURIOUS, spurious | 0x1FF);

    // Enter idle loop — scheduler will wake us up via IPI
    loop {
        let pc = percpu::current_mut();
        pc.idle = true;
        asm!("sti; hlt", options(nostack));
        pc.idle = false;
    }
}

// ── Delay helpers ──────────────────────────────────────────────────────

/// Busy-wait delay in milliseconds.
unsafe fn delay_ms(ms: u32) {
    // Use PIT-based delay or rdtsc-based delay
    let start = crate::cpu::rdtsc();
    let freq = crate::vendor::amd::cpu::zen3::fastos_cpu::tsc_freq_hz();
    if freq == 0 {
        // Fallback: rough loop
        for _ in 0..ms * 10000 {
            asm!("pause", options(nostack));
        }
        return;
    }
    let ticks = freq * ms as u64 / 1000;
    while crate::cpu::rdtsc().wrapping_sub(start) < ticks {
        asm!("pause", options(nostack));
    }
}

/// Busy-wait delay in microseconds.
unsafe fn delay_us(us: u32) {
    let start = crate::cpu::rdtsc();
    let freq = crate::vendor::amd::cpu::zen3::fastos_cpu::tsc_freq_hz();
    if freq == 0 {
        for _ in 0..us * 10 {
            asm!("pause", options(nostack));
        }
        return;
    }
    let ticks = freq * us as u64 / 1_000_000;
    while crate::cpu::rdtsc().wrapping_sub(start) < ticks {
        asm!("pause", options(nostack));
    }
}

// ── Trampoline binary ─────────────────────────────────────────────────
// This is the actual machine code that the trampoline page executes.
// It starts in 16-bit real mode, transitions to 32-bit protected mode,
// then to 64-bit long mode, and jumps to ap_entry().

/// 16-bit real-mode entry. This code is at offset 0 of the trampoline page.
/// It sets up a minimal GDT, enables protected mode, then long mode.
static TRAMPOLINE_CODE: [u8; 256] = {
    let mut code = [0u8; 256];

    // Offset 0x00: 16-bit real-mode entry (set up segments, enable A20)
    // cli
    code[0x00] = 0xFA;
    // xor ax, ax
    code[0x01] = 0x31; code[0x02] = 0xC0;
    // mov ds, ax
    code[0x03] = 0x8E; code[0x04] = 0xD8;
    // mov es, ax
    code[0x05] = 0x8E; code[0x06] = 0xC0;
    // mov ss, ax
    code[0x07] = 0x8E; code[0x08] = 0xD0;
    // mov sp, 0x7C00 (stack below trampoline)
    code[0x09] = 0xBC; code[0x0A] = 0x00; code[0x0B] = 0x7C;

    // Enable A20 line via keyboard controller
    // in al, 0x64
    code[0x0C] = 0xE4; code[0x0D] = 0x64;
    // test al, 0x02
    code[0x0E] = 0xA8; code[0x0F] = 0x02;
    // jnz 0x0C (spin)
    code[0x10] = 0x75; code[0x11] = 0xFA;
    // mov al, 0xD1
    code[0x12] = 0xB0; code[0x13] = 0xD1;
    // out 0x64, al
    code[0x14] = 0xE6; code[0x15] = 0x64;
    // in al, 0x64
    code[0x16] = 0xE4; code[0x17] = 0x64;
    // test al, 0x02
    code[0x18] = 0xA8; code[0x19] = 0x02;
    // jnz 0x16
    code[0x1A] = 0x75; code[0x1B] = 0xFA;
    // mov al, 0xDF
    code[0x1C] = 0xB0; code[0x1D] = 0xDF;
    // out 0x64, al
    code[0x1E] = 0xE6; code[0x1F] = 0x64;

    // Load GDT (physical address = TRAMPOLINE_PHYS + 0x1000)
    // lgdt [gdt_ptr]
    code[0x20] = 0x0F; code[0x21] = 0x01;
    code[0x22] = 0x1E; // mod=00, r/m=110 → [disp16]
    // GDT pointer: limit=0x1F (32 bytes), base=TRAMPOLINE_PHYS+0x1000
    // We'll patch this at runtime. For now, use a fixed offset.
    code[0x23] = 0x20; // limit low
    code[0x24] = 0x00; // limit high (byte)
    code[0x25] = 0x00; code[0x26] = 0x80; // base low = 0x8000
    code[0x27] = 0x00; code[0x28] = 0x00; // base high = 0

    // mov eax, cr0
    code[0x29] = 0x0F; code[0x2A] = 0x20; code[0x2B] = 0xC0;
    // or eax, 1 (PE bit)
    code[0x2C] = 0x83; code[0x2D] = 0xC8; code[0x2E] = 0x01;
    // mov cr0, eax
    code[0x2F] = 0x0F; code[0x30] = 0x22; code[0x31] = 0xC0;

    // Far jump to 32-bit protected mode code (offset 0x40)
    // jmp 0x08:0x40 (kernel code segment)
    code[0x32] = 0xEA;
    code[0x33] = 0x40; code[0x34] = 0x00; // offset = 0x0040
    code[0x35] = 0x00; code[0x36] = 0x00; // segment = 0x0008

    // Pad to offset 0x40 (32-bit protected mode entry)
    // At 0x40: set up 32-bit segments, enable PAE + paging, enter long mode

    // 32-bit entry at offset 0x40:
    // mov ax, 0x10  (data segment)
    code[0x40] = 0x66; code[0x41] = 0xB8;
    code[0x42] = 0x10; code[0x43] = 0x00;
    // mov ds, ax
    code[0x44] = 0x8E; code[0x45] = 0xD8;
    // mov es, ax
    code[0x46] = 0x8E; code[0x47] = 0xC0;
    // mov ss, ax
    code[0x48] = 0x8E; code[0x49] = 0xD0;
    // mov esp, 0x7C00
    code[0x4A] = 0x66; code[0x4B] = 0xBC;
    code[0x4C] = 0x00; code[0x4D] = 0x7C;
    code[0x4E] = 0x00; code[0x4F] = 0x00;

    // Enable PAE (CR4.PAE = bit 5)
    // mov eax, cr4
    code[0x50] = 0x0F; code[0x51] = 0x20; code[0x52] = 0xE0;
    // or eax, 0x20
    code[0x53] = 0x83; code[0x54] = 0xC8; code[0x55] = 0x20;
    // mov cr4, eax
    code[0x56] = 0x0F; code[0x57] = 0x22; code[0x58] = 0xE0;

    // Load CR3 with the PML4 table address (passed from BSP via trampoline data)
    // For simplicity, we'll use the same CR3 as the BSP (identity-mapped).
    // The BSP stores its CR3 at trampoline data offset 0x140.
    // mov eax, [0x8000 + 0x1000 + 0x140]  → physical 0x8140
    code[0x59] = 0xA1; // mov eax, [disp32]
    code[0x5A] = 0x40; code[0x5B] = 0x81; code[0x5C] = 0x00; code[0x5D] = 0x00;
    // mov cr3, eax
    code[0x5E] = 0x0F; code[0x5F] = 0x22; code[0x60] = 0xD8;

    // Enable paging (CR0.PG = bit 31)
    // mov eax, cr0
    code[0x61] = 0x0F; code[0x62] = 0x20; code[0x63] = 0xC0;
    // or eax, 0x80000000
    code[0x64] = 0x0D; code[0x65] = 0x00; code[0x66] = 0x00;
    code[0x67] = 0x00; code[0x68] = 0x80;
    // mov cr0, eax
    code[0x69] = 0x0F; code[0x6A] = 0x22; code[0x6B] = 0xC0;

    // Enable Long Mode (EFER.LME = MSR 0xC0000080, bit 8)
    // mov ecx, 0xC0000080
    code[0x6C] = 0xB9;
    code[0x6D] = 0x80; code[0x6E] = 0x00; code[0x6F] = 0x00;
    code[0x70] = 0xC0;
    // rdmsr
    code[0x71] = 0x0F; code[0x72] = 0x32;
    // or eax, 0x100 (LME)
    code[0x73] = 0x0D; code[0x74] = 0x00; code[0x75] = 0x01;
    code[0x76] = 0x00; code[0x77] = 0x00;
    // wrmsr
    code[0x78] = 0x0F; code[0x79] = 0x30;

    // Far jump to 64-bit long mode code (offset 0x80)
    // jmp 0x08:0x80
    code[0x7A] = 0xEA;
    code[0x7B] = 0x80; code[0x7C] = 0x00; // offset = 0x0080
    code[0x7D] = 0x00; code[0x7E] = 0x00; // segment = 0x0008
    code[0x7F] = 0x00; // padding

    // 64-bit long mode entry at offset 0x80:
    // We are now in 64-bit mode with the same GDT.
    // Load 64-bit data segments and set up a proper stack.

    // 64-bit mov rax, imm32 won't work with 0x80 prefix, use 32-bit mov + zero extend
    // Actually in 64-bit mode we need different encoding. Let's use:
    // mov ax, 0x10
    // Wait — in 64-bit mode, segment loading works differently.
    // Let's just load a known-good stack and jump to Rust.

    // Bits 0x80-0xBF: 64-bit setup
    // Use 32-bit immediate moves (zero-extended to 64-bit in long mode)

    // mov eax, 0x10  (data segment selector)
    // In 64-bit mode: B8 10 00 00 00
    code[0x80] = 0xB8; code[0x81] = 0x10;
    code[0x82] = 0x00; code[0x83] = 0x00; code[0x84] = 0x00;
    // mov ds, ax
    code[0x85] = 0x8E; code[0x86] = 0xD8;
    // mov es, ax
    code[0x87] = 0x8E; code[0x88] = 0xC0;
    // mov ss, ax
    code[0x89] = 0x8E; code[0x8A] = 0xD0;

    // Load RSP from trampoline data (offset 0x120 in data page)
    // mov esp, [0x8120]
    code[0x8B] = 0x8B; code[0x8C] = 0x25;
    code[0x8D] = 0x20; code[0x8E] = 0x81;
    code[0x8F] = 0x00; code[0x90] = 0x00;

    // Set RBP = 0
    // xor ebp, ebp
    code[0x91] = 0x31; code[0x92] = 0xED;

    // Signal status = AP_BOOT_STATUS_64BIT (already set from Rust side)
    // Actually the AP sets this itself. We'll just call ap_entry.

    // Load the address of ap_entry into rax
    // We need to store this at a known offset. Let's use the GDT data page.
    // At data page offset 0x160: address of ap_entry (8 bytes)
    // mov rax, [0x8160]
    // Actually we can't use 64-bit addressing in 32-bit mode, but we're in 64-bit now.
    // mov rax, qword ptr [0x8160]
    code[0x93] = 0x48; // REX.W prefix
    code[0x94] = 0xA1; // mov rax, moffs64
    code[0x95] = 0x60; code[0x96] = 0x81; code[0x97] = 0x00; code[0x98] = 0x00;
    code[0x99] = 0x00; code[0x9A] = 0x00; // offset in 8-byte immediate

    // jmp rax
    code[0x9B] = 0xFF; code[0x9C] = 0xE0;

    code
};

/// GDT template for AP cores. Loaded by the trampoline.
/// Layout:
///   0x00: Null descriptor
///   0x08: Kernel code (64-bit, DPL=0)
///   0x10: Kernel data (64-bit, DPL=0)
static AP_GDT_TEMPLATE: [u8; 32] = {
    let mut gdt = [0u8; 32];

    // Null descriptor (0x00)
    // Already zero

    // Kernel code segment (0x08): L=1, D=0, P=1, S=1, type=0xA (execute/read)
    gdt[0x08] = 0x00; // limit 0
    gdt[0x09] = 0x00;
    gdt[0x0A] = 0x00; // base 0
    gdt[0x0B] = 0x00;
    gdt[0x0C] = 0xFF; // limit 0xFFFF (bits 0-15)
    gdt[0x0D] = 0xFF; // limit 0xFFFF (bits 16-19) + AVL=0
    gdt[0x0E] = 0x00; // base 0
    gdt[0x0F] = 0x9A; // access: P=1, DPL=00, S=1, type=1010 (exec/read)

    // Kernel data segment (0x10): P=1, DPL=0, S=1, type=0x2 (read/write)
    gdt[0x10] = 0x00;
    gdt[0x11] = 0x00;
    gdt[0x12] = 0x00;
    gdt[0x13] = 0x00;
    gdt[0x14] = 0xFF;
    gdt[0x15] = 0xFF;
    gdt[0x16] = 0x00;
    gdt[0x17] = 0x92; // access: P=1, DPL=00, S=1, type=0010 (read/write)

    gdt
};

/// Patch the trampoline with the BSP's CR3 and ap_entry address.
pub unsafe fn patch_trampoline(cr3: u64, ap_entry_addr: u64) {
    // Store CR3 at data page offset 0x140
    let cr3_ptr = (TRAMPOLINE_PHYS + 4096 + 0x140) as *mut u64;
    core::ptr::write_volatile(cr3_ptr, cr3);

    // Store ap_entry address at data page offset 0x160
    let entry_ptr = (TRAMPOLINE_PHYS + 4096 + 0x160) as *mut u64;
    core::ptr::write_volatile(entry_ptr, ap_entry_addr);

    // Patch the LGDT base address in the trampoline code (offset 0x25-0x28)
    let gdt_base = TRAMPOLINE_PHYS + 0x1000;
    let code_base = TRAMPOLINE_PHYS as *mut u8;
    core::ptr::write_volatile(code_base.add(0x25), (gdt_base & 0xFF) as u8);
    core::ptr::write_volatile(code_base.add(0x26), ((gdt_base >> 8) & 0xFF) as u8);
    core::ptr::write_volatile(code_base.add(0x27), ((gdt_base >> 16) & 0xFF) as u8);
    core::ptr::write_volatile(code_base.add(0x28), ((gdt_base >> 24) & 0xFF) as u8);

    crate::dev::console::serial_write("[smp] trampoline patched: CR3=0x");
    crate::serial::hex(cr3);
    crate::dev::console::serial_write(" entry=0x");
    crate::serial::hex(ap_entry_addr);
    crate::dev::console::serial_write("\n");
}
