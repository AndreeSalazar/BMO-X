#![allow(dead_code)]

//! SMP (Symmetric Multi-Processing) for FastOS.
//!
//! Brings up Application Processors (APs) on AMD Ryzen 5 5600X (6C/12T).
//! Uses INIT-SIPI-SIPI IPI sequence through the Local APIC ICR.

use super::apic;
use crate::cpu;
use crate::dev::console;
use crate::mem::phys;
use core::ptr::{read_volatile, write_volatile};

// ── Constants ─────────────────────────────────────────────────────────
const MAX_AP: usize = 12;
const AP_STACK_SIZE: usize = 16 * 1024;
const TRAMPOLINE_PHYS: u64 = 0x8000;
const GDT_PHYS: u64 = 0x8500;
const AP_READY_MAGIC: u32 = 0x41_50_52_44;
const AP_TIMEOUT_CYCLES: u64 = 3_700_000_000;

// Per-CPU data layout offsets relative to TRAMPOLINE_PHYS + 0x400
const PD_READY: u64 = 0x00;
const PD_LAPIC_ID: u64 = 0x04;
const PD_STACK_TOP: u64 = 0x08;
const PD_AP_ENTRY: u64 = 0x30;

// ── Global state ──────────────────────────────────────────────────────
static mut AP_COUNT: usize = 0;
static mut AP_STACKS: [u64; MAX_AP] = [0; MAX_AP];

pub fn online_cpus() -> usize {
    unsafe { AP_COUNT + 1 }
}

#[no_mangle]
pub extern "C" fn ap_main(lapic_id: u32) -> ! {
    // Signal BSP that this AP is alive
    unsafe {
        let ptr = (TRAMPOLINE_PHYS + 0x400 + PD_READY) as *mut u32;
        write_volatile(ptr, AP_READY_MAGIC);
    }

    console::serial_write("[smp] AP alive, LAPIC ID=");
    serial_write_hex(lapic_id as u64);
    console::serial_write("\n");

    loop {
        crate::cpu::halt();
    }
}

fn serial_write_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    let mut buf = [0u8; 16];
    for i in 0..16 {
        buf[15 - i] = hex[((val >> (i * 4)) & 0xF) as usize];
    }
    console::serial_write(core::str::from_utf8(&buf).unwrap_or("0000000000000000"));
}

fn serial_write_num(val: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if val == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        let mut v = val;
        while v > 0 {
            i -= 1;
            buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
    }
    console::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}

pub unsafe fn smp_init() {
    console::serial_write("[smp] Starting AP bring-up (INIT-SIPI-SIPI)...\n");

    let max_cores = detect_core_count();
    let cores_to_start = core::cmp::min(max_cores - 1, MAX_AP);
    if cores_to_start == 0 {
        console::serial_write("[smp] Only 1 core detected; no APs to start.\n");
        return;
    }

    console::serial_write("[smp] Detected ");
    serial_write_num(cores_to_start as u64);
    console::serial_write(" AP(s) to start.\n");

    setup_ap_gdt();
    setup_trampoline();

    for i in 0..cores_to_start {
        let stack = phys::alloc_pages_contiguous(AP_STACK_SIZE / 4096)
            .expect("[smp] Failed to allocate AP stack");
        AP_STACKS[i] = stack + AP_STACK_SIZE as u64;
    }

    let pml4_phys = crate::mem::virt::read_cr3();
    let bsp_id = apic::read_lapic_id();
    let mut started = 0;

    for target_ap in 0..cores_to_start {
        let target_lapic_id = get_ap_lapic_id(target_ap);
        if target_lapic_id == bsp_id {
            continue;
        }

        let base = TRAMPOLINE_PHYS + 0x400;
        write_volatile((base + PD_READY) as *mut u32, 0);
        write_volatile((base + PD_LAPIC_ID) as *mut u32, target_lapic_id);
        write_volatile((base + PD_STACK_TOP) as *mut u64, AP_STACKS[started]);
        write_volatile((base + PD_AP_ENTRY) as *mut u64, ap_main as *const () as u64);

        // Write PML4 at trampoline + 0x3F0 (read by stub)
        write_volatile((TRAMPOLINE_PHYS + 0x3F0) as *mut u64, pml4_phys);

        send_init_ipi(target_lapic_id);
        crate::cpu::busy_wait_ms(10);

        send_sipi(target_lapic_id, 0x8);
        crate::cpu::busy_wait_ms(10);

        let mut ready = false;
        let start = crate::cpu::rdtsc();
        while read_volatile((base + PD_READY) as *const u32) != AP_READY_MAGIC {
            if crate::cpu::rdtsc().wrapping_sub(start) > AP_TIMEOUT_CYCLES {
                console::serial_write("[smp] AP timeout, LAPIC ID=");
                serial_write_hex(target_lapic_id as u64);
                console::serial_write("\n");
                break;
            }
        }
        if read_volatile((base + PD_READY) as *const u32) == AP_READY_MAGIC {
            ready = true;
        }

        // SIPI #2 if needed
        if !ready {
            send_sipi(target_lapic_id, 0x8);
            crate::cpu::busy_wait_ms(10);

            let start = crate::cpu::rdtsc();
            while read_volatile((base + PD_READY) as *const u32) != AP_READY_MAGIC {
                if crate::cpu::rdtsc().wrapping_sub(start) > AP_TIMEOUT_CYCLES {
                    break;
                }
            }
            if read_volatile((base + PD_READY) as *const u32) == AP_READY_MAGIC {
                ready = true;
            }
        }

        if ready {
            started += 1;
        }
    }

    AP_COUNT = started;
    console::serial_write("[smp] AP bring-up complete: ");
    serial_write_num(online_cpus() as u64);
    console::serial_write(" CPU(s) online.\n");
}

unsafe fn detect_core_count() -> usize {
    let (_, ebx, _, _) = crate::cpu::cpuid(0x0B, 0);
    let cores = (ebx & 0xFFFF) as usize;
    if cores == 0 { 1 } else { cores }
}

unsafe fn get_ap_lapic_id(ap_idx: usize) -> u32 {
    let bsp_id = apic::read_lapic_id();
    let mut found = 0;

    for level in 0..32 {
        let (_, ebx, ecx, _) = crate::cpu::cpuid_x2(0x0B, level);
        let shift = ebx & 0xFF;
        if shift == 0 || ecx == 0 {
            break;
        }
        let mask = (1u32 << shift) - 1;
        let id = (ecx >> shift) & mask;

        if id != bsp_id {
            if found == ap_idx {
                return id;
            }
            found += 1;
        }
    }
    0
}

// ── IPI helpers ───────────────────────────────────────────────────────

unsafe fn send_init_ipi(target_lapic_id: u32) {
    apic::apic_write(apic::APIC_ICR_HI, target_lapic_id << 24);
    apic::apic_write(apic::APIC_ICR_LO, 0x000C4500);
}

unsafe fn send_sipi(target_lapic_id: u32, vector: u32) {
    apic::apic_write(apic::APIC_ICR_HI, target_lapic_id << 24);
    apic::apic_write(apic::APIC_ICR_LO, 0x000C4600 | vector);
}

// ── GDT for APs ───────────────────────────────────────────────────────

unsafe fn setup_ap_gdt() {
    let gdt = GDT_PHYS as *mut u8;
    core::ptr::write_bytes(gdt, 0, 256);

    // Null descriptor (selector 0x00)
    // Code32 (selector 0x08)
    write_volatile(gdt.add(13), 0x9A);
    write_volatile(gdt.add(14), 0xCF);

    // Data32 (selector 0x10)
    write_volatile(gdt.add(21), 0x92);
    write_volatile(gdt.add(22), 0xCF);

    // Code64 (selector 0x18)
    write_volatile(gdt.add(29), 0x9A);
    write_volatile(gdt.add(30), 0x2F);

    // Data64 (selector 0x20)
    write_volatile(gdt.add(37), 0x92);
    write_volatile(gdt.add(38), 0xCF);

    // LGDT descriptor at trampoline + 0x200
    let gdtr = (TRAMPOLINE_PHYS + 0x200) as *mut u8;
    write_volatile(gdtr, 0xFF);
    write_volatile(gdtr.add(1), 0x00);
    write_volatile(gdtr.add(2), (GDT_PHYS & 0xFF) as u8);
    write_volatile(gdtr.add(3), ((GDT_PHYS >> 8) & 0xFF) as u8);
    write_volatile(gdtr.add(4), ((GDT_PHYS >> 16) & 0xFF) as u8);
    write_volatile(gdtr.add(5), ((GDT_PHYS >> 24) & 0xFF) as u8);
}

// ── Trampoline stub ───────────────────────────────────────────────────

unsafe fn setup_trampoline() {
    let page = TRAMPOLINE_PHYS as *mut u8;
    core::ptr::write_bytes(page, 0, 4096);

    let stub = build_trampoline_stub();
    core::ptr::copy_nonoverlapping(stub.as_ptr(), page, stub.len());
}

fn build_trampoline_stub() -> [u8; 256] {
    let mut c = [0u8; 256];
    let mut i: usize = 0;

    // ── Offset 0x00: 16-bit Real Mode Entry ───────────────────────────
    c[i] = 0xFA; i += 1;                     // CLI
    c[i] = 0x31; c[i+1] = 0xC0; i += 2;     // XOR AX, AX
    c[i] = 0x8E; c[i+1] = 0xD8; i += 2;     // MOV DS, AX
    c[i] = 0x8E; c[i+1] = 0xC0; i += 2;     // MOV ES, AX
    c[i] = 0x8E; c[i+1] = 0xD0; i += 2;     // MOV SS, AX
    c[i] = 0xBC; c[i+1] = 0x00; c[i+2] = 0x7C; i += 3; // MOV SP, 0x7C00

    // LGDT [0x8200] (16-bit: 0x0F 0x01 /2)
    c[i] = 0x0F; c[i+1] = 0x01; c[i+2] = 0x16; i += 3;
    c[i] = 0x00; c[i+1] = 0x82; i += 2;

    // MOV EAX, CR0; OR EAX, 1; MOV CR0, EAX
    c[i] = 0x0F; c[i+1] = 0x20; c[i+2] = 0xC0; i += 3;
    c[i] = 0x66; c[i+1] = 0x83; c[i+2] = 0xC8; c[i+3] = 0x01; i += 4;
    c[i] = 0x0F; c[i+1] = 0x22; c[i+2] = 0xC0; i += 3;

    // JMP 0x0008:pm32 (far jump to 32-bit code segment)
    let pm32 = 0x8000 + 48;
    c[i] = 0x66; c[i+1] = 0xEA; i += 2;
    c[i] = (pm32 & 0xFF) as u8; c[i+1] = ((pm32 >> 8) & 0xFF) as u8; i += 2;
    c[i] = 0x00; c[i+1] = 0x00; i += 2;
    c[i] = 0x08; c[i+1] = 0x00; i += 2;

    // Pad to offset 48
    while i < 48 { c[i] = 0x90; i += 1; }

    // ── Offset 48: 32-bit Protected Mode ──────────────────────────────
    c[i] = 0x66; c[i+1] = 0xB8; i += 2;     // MOV AX, 0x10
    c[i] = 0x10; c[i+1] = 0x00; i += 2;
    c[i] = 0x8E; c[i+1] = 0xD8; i += 2;     // MOV DS, AX
    c[i] = 0x8E; c[i+1] = 0xC0; i += 2;     // MOV ES, AX
    c[i] = 0x8E; c[i+1] = 0xD0; i += 2;     // MOV SS, AX

    // MOV ESP, 0x7C00
    c[i] = 0x66; c[i+1] = 0xBC; i += 2;
    c[i] = 0x00; c[i+1] = 0x7C; c[i+2] = 0x00; c[i+3] = 0x00; i += 4;

    // Enable PAE + PGE: MOV EAX, CR4; OR 0xA0; MOV CR4, EAX
    c[i] = 0x0F; c[i+1] = 0x20; c[i+2] = 0xE0; i += 3;
    c[i] = 0x66; c[i+1] = 0x0D; i += 2;
    c[i] = 0xA0; c[i+1] = 0x00; c[i+2] = 0x00; c[i+3] = 0x00; i += 4;
    c[i] = 0x0F; c[i+1] = 0x22; c[i+2] = 0xE0; i += 3;

    // Load CR3 from [0x83F0]
    c[i] = 0x67; c[i+1] = 0xA1; i += 2;
    c[i] = 0xF0; c[i+1] = 0x83; c[i+2] = 0x00; c[i+3] = 0x00; i += 4;
    c[i] = 0x0F; c[i+1] = 0x22; c[i+2] = 0xD8; i += 3;

    // Enable long mode: RDMSR 0xC0000080, OR LME, WRMSR
    c[i] = 0xB9; i += 1;
    c[i] = 0x80; c[i+1] = 0x00; c[i+2] = 0x00; c[i+3] = 0xC0; i += 4;
    c[i] = 0x0F; c[i+1] = 0x32; i += 2;
    c[i] = 0x0D; i += 1;
    c[i] = 0x00; c[i+1] = 0x01; c[i+2] = 0x00; c[i+3] = 0x00; i += 4;
    c[i] = 0x0F; c[i+1] = 0x30; i += 2;

    // Enable paging: MOV EAX, CR0; OR PG; MOV CR0, EAX
    c[i] = 0x0F; c[i+1] = 0x20; c[i+2] = 0xC0; i += 3;
    c[i] = 0x66; c[i+1] = 0x0D; i += 2;
    c[i] = 0x00; c[i+1] = 0x00; c[i+2] = 0x00; c[i+3] = 0x80; i += 4;
    c[i] = 0x0F; c[i+1] = 0x22; c[i+2] = 0xC0; i += 3;

    // JMP 0x0018:lm64 (far jump to 64-bit code segment)
    let lm64 = 0x8000 + 160;
    c[i] = 0x66; c[i+1] = 0xEA; i += 2;
    c[i] = (lm64 & 0xFF) as u8; c[i+1] = ((lm64 >> 8) & 0xFF) as u8; i += 2;
    c[i] = 0x00; c[i+1] = 0x00; i += 2;
    c[i] = 0x18; c[i+1] = 0x00; i += 2;

    // Pad to offset 160
    while i < 160 { c[i] = 0x90; i += 1; }

    // ── Offset 160: 64-bit Long Mode ──────────────────────────────────
    // MOV AX, 0x20 (data64 selector)
    c[i] = 0x66; c[i+1] = 0xB8; i += 2;
    c[i] = 0x20; c[i+1] = 0x00; i += 2;
    c[i] = 0x8E; c[i+1] = 0xD8; i += 2;     // MOV DS, AX
    c[i] = 0x8E; c[i+1] = 0xC0; i += 2;     // MOV ES, AX
    c[i] = 0x8E; c[i+1] = 0xD0; i += 2;     // MOV SS, AX
    c[i] = 0x8E; c[i+1] = 0xE0; i += 2;     // MOV FS, AX
    c[i] = 0x8E; c[i+1] = 0xE8; i += 2;     // MOV GS, AX

    // MOV RAX, [0x8408] (stack_top)
    c[i] = 0x48; c[i+1] = 0xA1; i += 2;
    c[i] = 0x08; c[i+1] = 0x84; c[i+2] = 0x00; c[i+3] = 0x00; i += 4;
    // MOV RSP, RAX
    c[i] = 0x48; c[i+1] = 0x89; c[i+2] = 0xC4; i += 3;

    // MOV RAX, [0x8430] (ap_entry)
    c[i] = 0x48; c[i+1] = 0xA1; i += 2;
    c[i] = 0x30; c[i+1] = 0x84; c[i+2] = 0x00; c[i+3] = 0x00; i += 4;

    // MOV EDI, [0x8404] (lapic_id — first arg to ap_main)
    c[i] = 0x67; c[i+1] = 0x8B; c[i+2] = 0x3D; i += 3;
    c[i] = 0x04; c[i+1] = 0x84; c[i+2] = 0x00; c[i+3] = 0x00; i += 4;

    // MFENCE
    c[i] = 0x0F; c[i+1] = 0xAE; c[i+2] = 0xF0; i += 3;

    // JMP RAX
    c[i] = 0xFF; c[i+1] = 0xE0;

    c
}
