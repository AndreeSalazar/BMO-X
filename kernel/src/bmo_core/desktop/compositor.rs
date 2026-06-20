//! Compositor Ring 3 — coordinador trivial.
//!
//! Tras Sesión 20 todo el render del escritorio vive en Ring 0
//! (`desktop/render.rs`). El proceso Ring 3 es minúsculo (~50 bytes)
//! y sólo orquesta el frame loop:
//!
//! ```bmo (pseudo)
//! beep 660 80                         ; bienvenida
//! mientras 1 {
//!     syscall DesktopFrame             ; 0x65 (Ring 0 dibuja todo)
//!     syscall NanoSleep 16_000_000     ; 0x51
//!     syscall KeyPoll                  ; 0x70 → rax
//!     si rax == 0x01: salir            ; ESC
//! }
//! ```
//!
//! El payload se ensambla con `barex::bmoasm::Emitter`.

extern crate alloc;

use crate::bmo_core::lang::bmoasm::emit::{Emitter, Reg64};

const SC_ESC: u8 = 0x01;

const SYS_EXIT:         u64 = 0x00;
const SYS_NSLEEP:       u64 = 0x51;
const SYS_DESKTOP_FRAME:u64 = 0x65;
const SYS_KEYPOLL:      u64 = 0x70;
const SYS_BEEP:         u64 = 0x80;

// ── Helpers de encoding x86-64 (faltantes en bmoasm::Emitter S15) ───

fn cmp_rax_imm32(e: &mut Emitter, imm: i32) {
    e.emit_raw(&[0x48, 0x3D]);
    e.emit_raw(&imm.to_le_bytes());
}

fn jmp_rel32(e: &mut Emitter, rel: i32) {
    e.emit_raw(&[0xE9]);
    e.emit_raw(&rel.to_le_bytes());
}

// ── Macro-helpers ───────────────────────────────────────────────────

fn sys0(e: &mut Emitter, nr: u64) {
    e.mov_reg_imm64(Reg64::Rax, nr);
    e.syscall();
}

fn sys1(e: &mut Emitter, nr: u64, a0: u64) {
    e.mov_reg_imm64(Reg64::Rax, nr);
    e.mov_reg_imm64(Reg64::Rdi, a0);
    e.syscall();
}

fn sys2(e: &mut Emitter, nr: u64, a0: u64, a1: u64) {
    e.mov_reg_imm64(Reg64::Rax, nr);
    e.mov_reg_imm64(Reg64::Rdi, a0);
    e.mov_reg_imm64(Reg64::Rsi, a1);
    e.syscall();
}

/// Construye el payload del compositor en `code_buf`. Devuelve
/// `(entry_offset, total_size)`.
pub fn build_compositor(code_buf: &mut [u8], _base_addr: u64) -> (usize, usize) {
    let mut e = Emitter::new();

    // Beep de bienvenida (660 Hz, 80 ms).
    sys2(&mut e, SYS_BEEP, 660, 80);

    // ── ETIQUETA: .frame ─────────────────────────────────────────────
    let frame_start = e.here();

    // 1. Render Ring 0 hace todo (wallpaper, status bar, ventanas, dock, cursor).
    sys0(&mut e, SYS_DESKTOP_FRAME);

    // 2. Dormir ~16 ms (≈60 FPS).
    sys1(&mut e, SYS_NSLEEP, 16_000_000);

    // 3. Pollear teclado.
    sys0(&mut e, SYS_KEYPOLL);
    cmp_rax_imm32(&mut e, SC_ESC as i32);

    // 4. Si NO es ESC → jne rel8 a `loop_back`.
    let jne_off = e.here();
    e.emit_raw(&[0x75, 0]); // placeholder

    // 5. Si ES ESC → ProcessExit (sale).
    sys0(&mut e, SYS_EXIT);

    // loop_back: vuelve al inicio del frame
    let loop_back = e.here();
    let rel8 = (loop_back as isize) - (jne_off as isize + 2);
    e.bytes[jne_off + 1] = (rel8 as i8) as u8;

    // jmp rel32 → frame_start
    let here_after_jmp = e.here() + 5;
    let frame_rel = (frame_start as isize) - (here_after_jmp as isize);
    jmp_rel32(&mut e, frame_rel as i32);

    let total = e.bytes.len();
    code_buf[..total].copy_from_slice(&e.bytes);
    (0, total)
}
