//! user_init — Lanza el primer proceso de usuario (Ring 3) de FastOS / BMO.
//!
//! Modos:
//!   - `spawn_hello()`       → payload mínimo: imprime por serial vía syscall
//!                             0xF0 y termina con 0x00. Demuestra la trampolina.
//!   - `spawn_desktop()`     → payload del compositor Hyprland/Win11 generado
//!                             con `barex::bmoasm::Emitter`. Loop ~60 FPS,
//!                             ESC sale.

#![allow(dead_code)]

use core::arch::asm;
use crate::arch::gdt::{USER_CS, USER_DS, set_kernel_stack};
use crate::arch::syscall_entry::set_syscall_kernel_stack;
use crate::drivers::serial;
use crate::desktop::compositor;

// ─── Pilas y buffer de código ───────────────────────────────────────

#[repr(align(16))]
struct UserStack([u8; 32 * 1024]);
static mut USER_STACK: UserStack = UserStack([0; 32 * 1024]);

#[repr(align(16))]
struct UserKernStack([u8; 32 * 1024]);
static mut USER_KERN_STACK: UserKernStack = UserKernStack([0; 32 * 1024]);

#[repr(align(4096))]
struct UserCode([u8; 16 * 1024]);
static mut USER_CODE: UserCode = UserCode([0; 16 * 1024]);

const MSG: &[u8] = b"[Ring3] Hola desde el primer proceso de usuario BMO\n";

// ─── Construcción del payload "hello" (mini, sin bmoasm) ────────────

unsafe fn build_hello_payload() -> u64 {
    let base = core::ptr::addr_of_mut!(USER_CODE) as *mut u8;
    let buf = core::slice::from_raw_parts_mut(base, 16 * 1024);
    let mut p: usize = 0;

    // mov rax, 0xF0 (DebugPrint)
    buf[p..p+7].copy_from_slice(&[0x48, 0xC7, 0xC0, 0xF0, 0x00, 0x00, 0x00]); p += 7;
    // lea rdi, [rip + disp32]
    buf[p..p+3].copy_from_slice(&[0x48, 0x8D, 0x3D]); p += 3;
    let lea_disp_off = p;
    buf[p..p+4].copy_from_slice(&[0; 4]); p += 4;
    // mov rsi, len
    buf[p..p+3].copy_from_slice(&[0x48, 0xC7, 0xC6]); p += 3;
    buf[p..p+4].copy_from_slice(&(MSG.len() as u32).to_le_bytes()); p += 4;
    // syscall
    buf[p..p+2].copy_from_slice(&[0x0F, 0x05]); p += 2;
    // mov rax, 0 ; syscall (ProcessExit)
    buf[p..p+7].copy_from_slice(&[0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00]); p += 7;
    buf[p..p+2].copy_from_slice(&[0x0F, 0x05]); p += 2;
    // hlt; jmp $-3
    buf[p] = 0xF4; p += 1;
    buf[p..p+2].copy_from_slice(&[0xEB, 0xFD]); p += 2;
    while p % 16 != 0 { buf[p] = 0x90; p += 1; }
    let msg_off = p;
    buf[p..p + MSG.len()].copy_from_slice(MSG);
    let disp = msg_off as i32 - (lea_disp_off as i32 + 4);
    buf[lea_disp_off..lea_disp_off + 4].copy_from_slice(&disp.to_le_bytes());

    base as u64
}

/// Construye el payload del compositor con `bmoasm::Emitter` y devuelve
/// su entry point.
unsafe fn build_desktop_payload() -> u64 {
    let base = core::ptr::addr_of_mut!(USER_CODE) as *mut u8;
    let buf = core::slice::from_raw_parts_mut(base, 16 * 1024);
    let base_addr = base as u64;
    let (entry_off, total) = compositor::build_compositor(buf, base_addr);
    serial::serial_write("[user_init] compositor size = ");
    print_dec(total as u64);
    serial::serial_write(" bytes\n");
    base_addr + entry_off as u64
}

// ─── Salto Ring 0 → Ring 3 ──────────────────────────────────────────

fn enter_ring3(entry: u64) -> ! {
    let user_stack_top = unsafe {
        core::ptr::addr_of!(USER_STACK) as u64 + 32 * 1024 - 16
    };
    let kern_stack_top = unsafe {
        core::ptr::addr_of!(USER_KERN_STACK) as u64 + 32 * 1024
    };

    set_kernel_stack(kern_stack_top);
    set_syscall_kernel_stack(kern_stack_top);

    serial::serial_write("[user_init] entry  = "); print_hex(entry);
    serial::serial_write("[user_init] u_rsp  = "); print_hex(user_stack_top);
    serial::serial_write("[user_init] k_rsp0 = "); print_hex(kern_stack_top);
    serial::serial_write("[user_init] Saltando a Ring 3 (iretq)...\n");

    unsafe {
        asm!(
            "mov ds, {ds:x}",
            "mov es, {ds:x}",
            "mov fs, {ds:x}",
            "mov gs, {ds:x}",
            "push {ss}",
            "push {rsp}",
            "push {rflags}",
            "push {cs}",
            "push {rip}",
            "iretq",
            ds = in(reg) USER_DS as u64,
            ss = in(reg) USER_DS as u64,
            rsp = in(reg) user_stack_top,
            rflags = in(reg) 0x002u64,    // IF=0 (no IRQs aún), bit 1 reservado
            cs = in(reg) USER_CS as u64,
            rip = in(reg) entry,
            options(noreturn),
        );
    }
}

// ─── API pública ───────────────────────────────────────────────────

pub fn spawn_hello() -> ! {
    serial::serial_write("[user_init] Lanzando 'hello' Ring 3...\n");
    let entry = unsafe { build_hello_payload() };
    enter_ring3(entry);
}

pub fn spawn_desktop() -> ! {
    serial::serial_write("[user_init] Construyendo compositor con bmoasm::Emitter...\n");
    let entry = unsafe { build_desktop_payload() };
    enter_ring3(entry);
}

// ─── Helpers ───────────────────────────────────────────────────────

fn print_hex(v: u64) {
    let hex = b"0123456789ABCDEF";
    serial::serial_write("0x");
    for i in (0..16).rev() {
        serial::serial_write_byte(hex[((v >> (i * 4)) & 0xF) as usize]);
    }
    serial::serial_write("\n");
}

fn print_dec(mut v: u64) {
    if v == 0 { serial::serial_write_byte(b'0'); return; }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while v > 0 { buf[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
    while i > 0 { i -= 1; serial::serial_write_byte(buf[i]); }
}
