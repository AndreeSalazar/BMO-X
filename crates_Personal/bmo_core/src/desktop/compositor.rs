//! Compositor Ring 3 — coordinador trivial.
//!
//! Tras Sesión 20 todo el render del escritorio vive en Ring 0
//! (`desktop/render.rs`). El proceso Ring 3 es minúsculo (~50 bytes)
//! y sólo orquesta el frame loop.
//!
//! El payload se ensambla directamente con bytes x86-64.

#![allow(dead_code)]

extern crate alloc;

use alloc::vec::Vec;

const SC_ESC: u8 = 0x01;

const SYS_EXIT:          u64 = 0x00;
const SYS_NSLEEP:        u64 = 0x51;
const SYS_DESKTOP_FRAME: u64 = 0x65;
const SYS_KEYPOLL:       u64 = 0x70;
const SYS_BEEP:          u64 = 0x80;

fn emit_mov_rax_imm64(buf: &mut Vec<u8>, imm: u64) {
    buf.extend_from_slice(&[0x48, 0xB8]);
    buf.extend_from_slice(&imm.to_le_bytes());
}

fn emit_mov_rdi_imm64(buf: &mut Vec<u8>, imm: u64) {
    buf.extend_from_slice(&[0x48, 0xBF]);
    buf.extend_from_slice(&imm.to_le_bytes());
}

fn emit_mov_rsi_imm64(buf: &mut Vec<u8>, imm: u64) {
    buf.extend_from_slice(&[0x48, 0xBE]);
    buf.extend_from_slice(&imm.to_le_bytes());
}

fn emit_syscall(buf: &mut Vec<u8>) {
    buf.extend_from_slice(&[0x0F, 0x05]);
}

fn emit_cmp_rax_imm32(buf: &mut Vec<u8>, imm: i32) {
    buf.extend_from_slice(&[0x48, 0x3D]);
    buf.extend_from_slice(&imm.to_le_bytes());
}

fn emit_jne_rel8(buf: &mut Vec<u8>, _placeholder: u8) {
    buf.extend_from_slice(&[0x75, _placeholder]);
}

fn emit_jmp_rel32(buf: &mut Vec<u8>, rel: i32) {
    buf.extend_from_slice(&[0xE9]);
    buf.extend_from_slice(&rel.to_le_bytes());
}

fn emit_sys0(buf: &mut Vec<u8>, nr: u64) {
    emit_mov_rax_imm64(buf, nr);
    emit_syscall(buf);
}

fn emit_sys1(buf: &mut Vec<u8>, nr: u64, a0: u64) {
    emit_mov_rax_imm64(buf, nr);
    emit_mov_rdi_imm64(buf, a0);
    emit_syscall(buf);
}

fn emit_sys2(buf: &mut Vec<u8>, nr: u64, a0: u64, a1: u64) {
    emit_mov_rax_imm64(buf, nr);
    emit_mov_rdi_imm64(buf, a0);
    emit_mov_rsi_imm64(buf, a1);
    emit_syscall(buf);
}

/// Construye el payload del compositor en `code_buf`. Devuelve
/// `(entry_offset, total_size)`.
pub fn build_compositor(code_buf: &mut [u8], _base: u64) -> (usize, usize) {
    let mut code = Vec::new();

    emit_sys2(&mut code, SYS_BEEP, 660, 80);

    let frame_start = code.len();

    emit_sys0(&mut code, SYS_DESKTOP_FRAME);
    emit_sys1(&mut code, SYS_NSLEEP, 16_000_000);
    emit_sys0(&mut code, SYS_KEYPOLL);
    emit_cmp_rax_imm32(&mut code, SC_ESC as i32);

    let jne_off = code.len();
    emit_jne_rel8(&mut code, 0);

    emit_sys0(&mut code, SYS_EXIT);

    let loop_back = code.len();
    let rel8 = (loop_back as isize) - (jne_off as isize + 2);
    code[jne_off + 1] = (rel8 as i8) as u8;

    let here_after_jmp = code.len() + 5;
    let frame_rel = (frame_start as isize) - (here_after_jmp as isize);
    emit_jmp_rel32(&mut code, frame_rel as i32);

    let total = code.len();
    code_buf[..total].copy_from_slice(&code);
    (0, total)
}
