//! user_init — Primer proceso de usuario (Ring 3) de FastOS / BMO.
//!
//! Demuestra el salto Ring 0 → Ring 3 con `iretq` y permite que el código
//! de usuario invoque `syscall` para volver al kernel.
//!
//! El payload del primer proceso es un blob de bytes nativos x86-64
//! (no requiere loader BEF aún). Hace:
//!
//!   mov rax, 0xF0           ; syscall DebugPrint
//!   lea rdi, [rip + msg]    ; ptr al string UTF-8
//!   mov rsi, msg_len        ; longitud
//!   syscall
//!   mov rax, 0x00           ; ProcessExit
//!   syscall
//! .loop: hlt; jmp .loop
//!
//! El kernel imprime el mensaje vía `serial_write` desde Ring 0,
//! lo que demuestra el round-trip Ring 3 → Ring 0 → Ring 3.

#![allow(dead_code)]

use core::arch::asm;
use crate::arch::gdt::{USER_CS, USER_DS, set_kernel_stack};
use crate::arch::syscall_entry::set_syscall_kernel_stack;
use crate::drivers::serial;

/// Stack de usuario (Ring 3) — 16 KB, alineado.
#[repr(align(16))]
struct UserStack([u8; 16 * 1024]);
static mut USER_STACK: UserStack = UserStack([0; 16 * 1024]);

/// Stack del kernel reservado para las transiciones Ring 3 → Ring 0
/// (entradas por `syscall` o interrupción). 16 KB.
#[repr(align(16))]
struct UserKernStack([u8; 16 * 1024]);
static mut USER_KERN_STACK: UserKernStack = UserKernStack([0; 16 * 1024]);

/// Payload de Ring 3 — código x86-64 nativo.
///
/// Layout final (offsets relativos al inicio del array):
///   0x00..len_code   código ejecutable
///   len_code..end    string "FastOS Ring3 OK\n"
///
/// El payload completo está en la misma página alineada a 4 KB, por lo que
/// se puede mapear identidad y ejecutar directamente desde Ring 3.
///
/// Encoding manual de las instrucciones:
///   48 C7 C0 F0 00 00 00      mov rax, 0xF0
///   48 8D 3D XX XX XX XX      lea rdi, [rip + disp32]
///   48 C7 C6 LL 00 00 00      mov rsi, len
///   0F 05                     syscall
///   48 C7 C0 00 00 00 00      mov rax, 0x00
///   0F 05                     syscall
///   F4                        hlt
///   EB FE                     jmp $-0  (loop)
///
/// Generamos el blob en run-time para resolver `disp32` correctamente
/// según la dirección de carga.
#[repr(align(4096))]
struct UserCode([u8; 4096]);
static mut USER_CODE: UserCode = UserCode([0; 4096]);

const MSG: &[u8] = b"[Ring3] Hola desde el primer proceso de usuario BMO\n";

/// Construye el payload de usuario en `USER_CODE` y devuelve la dirección
/// virtual del entry point.
unsafe fn build_user_payload() -> u64 {
    let base = core::ptr::addr_of_mut!(USER_CODE) as *mut u8;
    let code_buf = core::slice::from_raw_parts_mut(base, 4096);

    // 1) escribir código
    let mut p: usize = 0;

    // mov rax, 0xF0  ;  syscall DebugPrint
    code_buf[p..p+7].copy_from_slice(&[0x48, 0xC7, 0xC0, 0xF0, 0x00, 0x00, 0x00]); p += 7;

    // lea rdi, [rip + disp32]  — placeholder, resolveremos disp32 luego
    code_buf[p..p+3].copy_from_slice(&[0x48, 0x8D, 0x3D]); p += 3;
    let lea_disp_off = p;
    code_buf[p..p+4].copy_from_slice(&[0, 0, 0, 0]); p += 4;

    // mov rsi, MSG.len()
    code_buf[p..p+3].copy_from_slice(&[0x48, 0xC7, 0xC6]); p += 3;
    let len = MSG.len() as u32;
    code_buf[p..p+4].copy_from_slice(&len.to_le_bytes()); p += 4;

    // syscall
    code_buf[p..p+2].copy_from_slice(&[0x0F, 0x05]); p += 2;

    // mov rax, 0x00  ;  ProcessExit
    code_buf[p..p+7].copy_from_slice(&[0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00]); p += 7;

    // syscall
    code_buf[p..p+2].copy_from_slice(&[0x0F, 0x05]); p += 2;

    // .loop: hlt ; jmp .loop  (por si el kernel devuelve aquí)
    code_buf[p..p+1].copy_from_slice(&[0xF4]); p += 1;
    code_buf[p..p+2].copy_from_slice(&[0xEB, 0xFD]); p += 2; // jmp -3 → back to hlt

    // 2) alinear el string a 16 bytes y copiarlo
    while p % 16 != 0 { code_buf[p] = 0x90; p += 1; }
    let msg_off = p;
    code_buf[p..p + MSG.len()].copy_from_slice(MSG);

    // 3) resolver disp32 del lea: RIP-relative al final del `lea` (lea_disp_off + 4)
    let rip_after_lea = lea_disp_off + 4;
    let disp: i32 = msg_off as i32 - rip_after_lea as i32;
    code_buf[lea_disp_off..lea_disp_off + 4].copy_from_slice(&disp.to_le_bytes());

    base as u64
}

/// Lanza el primer proceso Ring 3 — punto sin retorno (hasta que el proceso
/// haga `ProcessExit`).
///
/// Esta función:
///   1. Construye el payload en `USER_CODE`.
///   2. Configura el kernel stack para futuras transiciones desde Ring 3.
///   3. Hace el salto Ring 0 → Ring 3 con `iretq`.
pub fn spawn_first_user_process() {
    serial::serial_write("[user_init] Construyendo payload Ring 3...\n");

    let entry = unsafe { build_user_payload() };
    let user_stack_top = unsafe {
        core::ptr::addr_of!(USER_STACK) as u64 + 16 * 1024 - 16
    };
    let kern_stack_top = unsafe {
        core::ptr::addr_of!(USER_KERN_STACK) as u64 + 16 * 1024
    };

    serial::serial_write("[user_init] entry  = ");
    print_hex(entry);
    serial::serial_write("[user_init] u_rsp  = ");
    print_hex(user_stack_top);
    serial::serial_write("[user_init] k_rsp0 = ");
    print_hex(kern_stack_top);

    // Configurar TSS.RSP0 y la pila para la trampolina de syscall.
    set_kernel_stack(kern_stack_top);
    set_syscall_kernel_stack(kern_stack_top);

    serial::serial_write("[user_init] Saltando a Ring 3 via iretq...\n");

    // iretq necesita el stack en este orden (de tope a fondo):
    //   SS, RSP, RFLAGS, CS, RIP
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
            rflags = in(reg) 0x202u64,    // IF=1 + reserved bit
            cs = in(reg) USER_CS as u64,
            rip = in(reg) entry,
            options(noreturn),
        );
    }
}

fn print_hex(v: u64) {
    let hex = b"0123456789ABCDEF";
    serial::serial_write("0x");
    for i in (0..16).rev() {
        serial::serial_write_byte(hex[((v >> (i * 4)) & 0xF) as usize]);
    }
    serial::serial_write("\n");
}
