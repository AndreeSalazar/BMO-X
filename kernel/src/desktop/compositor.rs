//! Compositor Ring 3 — genera el payload x86-64 nativo de un escritorio
//! tipo Hyprland / Windows 11 usando `barex::bmoasm::Emitter`.
//!
//! Layout pantalla (1920×1080):
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │  ░ Status bar  (Hyprland-style, 32 px) — workspaces · clock  │
//! ├───────────────────────────┬──────────────────────────────────┤
//! │                           │                                  │
//! │   Tile  ▌ BMO Shell       │   Tile  ▌ Datos.md viewer        │
//! │   (left half)             │   (right half)                   │
//! │                           │                                  │
//! ├──────────────────────────────────────────────────────────────┤
//! │  ▌ Win11 Taskbar (40 px) — Start · running apps · tray       │
//! └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! Loop principal (~60 FPS):
//!
//! ```bmo  (pseudo-BMO Simple)
//! mientras 1 {
//!     llamar fb_fill(0,0,1920,1080, AZUL_WIN11)     ; wallpaper
//!     llamar fb_fill(0,0,1920,32,    HYPR_BAR)      ; status bar
//!     llamar fb_fill(8,40,948,930,   PANEL)         ; tile izquierdo
//!     llamar fb_fill(8,40,948,28,    AZUL_WIN11)    ; titlebar L
//!     llamar fb_fill(964,40,948,930, PANEL)         ; tile derecho
//!     llamar fb_fill(964,40,948,28,  VERDE_BMO)     ; titlebar R
//!     llamar fb_fill(0,1040,1920,40, TASKBAR)       ; taskbar
//!     llamar fb_fill(8,1044,80,32,   VERDE_BMO)     ; Start button
//!     llamar fb_text(...)                            ; etiquetas
//!     pausa
//!     llamar nano_sleep(16_000_000)                 ; ~60 FPS
//!     si llamar key_poll() = ESC: salir
//! }
//! ```

extern crate alloc;
use alloc::vec::Vec;

use crate::barex::bmoasm::emit::{Emitter, Reg64};
use crate::barex::bmoasm::builtin::{IntrinsicId, bytes_for};

// ── Constantes del look Win11 + Hyprland ────────────────────────────
pub const C_WIN11_BLUE:  u32 = 0xFF0078D4;
pub const C_HYPR_BAR:    u32 = 0xFF1A1B26;  // tokyonight bg
pub const C_PANEL:       u32 = 0xFF21262D;
pub const C_PANEL_HI:    u32 = 0xFF30363D;
pub const C_BMO_GREEN:   u32 = 0xFF76B900;
pub const C_TASKBAR:     u32 = 0xFF161B22;
pub const C_TEXT:        u32 = 0xFFE6EDF3;
pub const C_TEXT_DIM:    u32 = 0xFF8B949E;
pub const C_ACCENT_CYAN: u32 = 0xFF56D4DD;

const SC_ESC: u8 = 0x01;

// ── BMO syscall numbers (must match arch/syscall_entry.rs) ─────────
const SYS_EXIT:      u64 = 0x00;
const SYS_NSLEEP:    u64 = 0x51;
const SYS_FBFILL:    u64 = 0x61;
const SYS_FBTEXT:    u64 = 0x62;
const SYS_FBPRES:    u64 = 0x63;
const SYS_FBBLIT:    u64 = 0x64;
const SYS_KEYPOLL:   u64 = 0x70;
const SYS_MOUSEPOLL: u64 = 0x71;
const SYS_BEEP:      u64 = 0x80;

// ────────────────────────────────────────────────────────────────────
// Helpers de codificación x86-64 que el `Emitter` aún no tiene
// (BMO Simple S15 sólo expone mov_reg_imm64 + ret + syscall + nop).
// Usamos `emit_raw` para añadir lo justo:
//   - mov rax, imm32     (siempre con REX.W para set claro)
//   - cmp rax, imm32
//   - jne rel8 / je rel8
//   - jmp rel32          (loop largo del frame)
//   - movabs ya lo da mov_reg_imm64
// ────────────────────────────────────────────────────────────────────

/// `cmp rax, imm32` → 48 3D ii ii ii ii
fn cmp_rax_imm32(e: &mut Emitter, imm: i32) {
    e.emit_raw(&[0x48, 0x3D]);
    e.emit_raw(&imm.to_le_bytes());
}

/// `jne rel8` → 75 rr   (toma el offset relativo desde el byte siguiente)
fn jne_rel8(e: &mut Emitter, rel: i8) {
    e.emit_raw(&[0x75, rel as u8]);
}

/// `jmp rel32` → E9 dd dd dd dd
fn jmp_rel32(e: &mut Emitter, rel: i32) {
    e.emit_raw(&[0xE9]);
    e.emit_raw(&rel.to_le_bytes());
}

// ────────────────────────────────────────────────────────────────────
// Macro-helpers que emiten una llamada syscall BMO con args
// BMO ABI:  RAX=nr · RDI=a0 · RSI=a1 · RDX=a2 · R10=a3 · R8=a4
// ────────────────────────────────────────────────────────────────────

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

fn sys5(e: &mut Emitter, nr: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64) {
    e.mov_reg_imm64(Reg64::Rax, nr);
    e.mov_reg_imm64(Reg64::Rdi, a0);
    e.mov_reg_imm64(Reg64::Rsi, a1);
    e.mov_reg_imm64(Reg64::Rdx, a2);
    e.mov_reg_imm64(Reg64::R10, a3);
    e.mov_reg_imm64(Reg64::R8,  a4);
    e.syscall();
}

/// FbFill(x,y,w,h,color)
fn fbfill(e: &mut Emitter, x: u32, y: u32, w: u32, h: u32, color: u32) {
    sys5(e, SYS_FBFILL, x as u64, y as u64, w as u64, h as u64, color as u64);
}

/// FbText(x, y, ptr, len, color) — `ptr` se pasa como dirección absoluta
fn fbtext(e: &mut Emitter, x: u32, y: u32, ptr: u64, len: u32, color: u32) {
    sys5(e, SYS_FBTEXT, x as u64, y as u64, ptr, len as u64, color as u64);
}

// ────────────────────────────────────────────────────────────────────
// Generación del payload completo del compositor
// ────────────────────────────────────────────────────────────────────

/// Strings que viven al final del payload; sus offsets se resuelven en el
/// segundo pase (cuando ya conocemos `base_addr`).
struct StringTable<'a> {
    items: &'a [&'a [u8]],
    /// Offsets desde el inicio del buffer al inicio de cada string.
    offsets: Vec<usize>,
}

const LABELS: &[&[u8]] = &[
    b" BMO  1  2  3  4  5     [Hyprland] -- 1920x1080",         // 0 status bar
    b" BMO Shell  -- Ring 3",                                    // 1 left titlebar
    b" Datos.md  -- FastOS Snapshot",                            // 2 right titlebar
    b"$ bmo > _",                                                // 3 shell prompt
    b"FastOS / BMO  v0.9.0",                                     // 4 right content L1
    b"Ring 0 + Ring 3 OK",                                       // 5 right content L2
    b"Compositor: 60 FPS estable",                               // 6 right content L3
    b" START ",                                                  // 7 Start button
    b" 12:34 ",                                                  // 8 clock
    b"ESC: salir del escritorio",                                // 9 footer hint
];

/// Construye el payload completo del compositor en `code_buf`.
///
/// Devuelve `(entry_offset, total_size)` — `entry_offset` siempre es 0
/// (las strings van al final).
pub fn build_compositor(code_buf: &mut [u8], base_addr: u64) -> (usize, usize) {
    let mut e = Emitter::new();

    // ── Beep de bienvenida (440 Hz, 60 ms) ──────────────────────────
    sys2(&mut e, SYS_BEEP, 440, 60);

    // ── Cabecera de programa: limpia rbx (frame counter) ────────────
    // xor ebx, ebx   31 DB
    e.emit_raw(&[0x31, 0xDB]);

    // ── ETIQUETA: .frame ────────────────────────────────────────────
    let frame_start = e.here();

    // 1) Wallpaper Win11
    fbfill(&mut e, 0, 0, 1920, 1080, C_WIN11_BLUE);

    // 2) Status bar Hyprland
    fbfill(&mut e, 0, 0, 1920, 32, C_HYPR_BAR);

    // 3) Tile izquierdo (panel + titlebar + borde)
    fbfill(&mut e, 8,   40, 948, 996, C_PANEL);
    fbfill(&mut e, 8,   40, 948, 28,  C_WIN11_BLUE);

    // 4) Tile derecho (panel + titlebar verde BMO)
    fbfill(&mut e, 964, 40, 948, 996, C_PANEL);
    fbfill(&mut e, 964, 40, 948, 28,  C_BMO_GREEN);

    // 5) Taskbar Win11
    fbfill(&mut e, 0, 1040, 1920, 40, C_TASKBAR);
    fbfill(&mut e, 8, 1044,   80, 32, C_BMO_GREEN); // Start button
    fbfill(&mut e, 1820, 1044, 92, 32, C_PANEL_HI); // tray

    // ── Reservar el segundo pase de strings: la dirección absoluta de
    //    cada string es base_addr + (data_offset + string_offset_dentro_de_data).
    //    Como aún no la conocemos, lo hacemos en build_with_strings tras
    //    layout. Para mantener el payload de un sólo pase, usamos una
    //    sub-rutina que devuelve la posición y luego parchamos.
    //
    // Estrategia simple: emitimos placeholders de movabs (10 bytes c/u)
    // y los parcheamos al final.

    let mut text_patches: Vec<(usize, usize)> = Vec::new(); // (movabs_imm_offset, string_index)

    let mut fbtext_placeholder = |em: &mut Emitter, x: u32, y: u32, str_idx: usize, color: u32, patches: &mut Vec<(usize, usize)>| {
        // mov rax, SYS_FBTEXT
        em.mov_reg_imm64(Reg64::Rax, SYS_FBTEXT);
        // mov rdi, x
        em.mov_reg_imm64(Reg64::Rdi, x as u64);
        // mov rsi, y
        em.mov_reg_imm64(Reg64::Rsi, y as u64);
        // mov rdx, <ptr placeholder>  → REX.W 0xBA imm64 (10 bytes)
        em.emit_raw(&[0x48, 0xBA]);
        let imm_off = em.here();
        em.emit_raw(&[0; 8]);
        patches.push((imm_off, str_idx));
        // mov r10, len
        em.mov_reg_imm64(Reg64::R10, LABELS[str_idx].len() as u64);
        // mov r8, color
        em.mov_reg_imm64(Reg64::R8, color as u64);
        em.syscall();
    };

    // 6) Status bar text (Hyprland workspaces + título)
    fbtext_placeholder(&mut e, 16, 8,   0, C_TEXT,        &mut text_patches);
    // 7) Left titlebar text
    fbtext_placeholder(&mut e, 24, 46,  1, C_TEXT,        &mut text_patches);
    // 8) Right titlebar text
    fbtext_placeholder(&mut e, 980, 46, 2, C_HYPR_BAR,    &mut text_patches);
    // 9) Shell prompt en tile izq
    fbtext_placeholder(&mut e, 24, 96,  3, C_BMO_GREEN,   &mut text_patches);
    // 10) Datos.md content en tile der
    fbtext_placeholder(&mut e, 980, 96,  4, C_TEXT,        &mut text_patches);
    fbtext_placeholder(&mut e, 980, 116, 5, C_ACCENT_CYAN, &mut text_patches);
    fbtext_placeholder(&mut e, 980, 136, 6, C_TEXT_DIM,    &mut text_patches);
    // 11) Start button label
    fbtext_placeholder(&mut e, 18, 1052, 7, C_HYPR_BAR,   &mut text_patches);
    // 12) Tray clock
    fbtext_placeholder(&mut e, 1834, 1052, 8, C_TEXT,     &mut text_patches);
    // 13) Footer hint
    fbtext_placeholder(&mut e, 16, 1052, 9, C_TEXT_DIM,   &mut text_patches);

    // 7) Sleep ~16 ms (60 FPS)
    sys1(&mut e, SYS_NSLEEP, 16_000_000);

    // 8) Poll mouse → dibujar cursor en la posición devuelta.
    //    RAX = x | (y<<16) | (buttons<<32). Extraemos x (low16) e y (mid16)
    //    con shifts y ands, los movemos a rdi/rsi y llamamos a FbFill 12x12.
    sys0(&mut e, SYS_MOUSEPOLL);
    // mov r12, rax            ; preserve packed mouse state
    e.emit_raw(&[0x49, 0x89, 0xC4]);
    // mov rdi, r12            ; rdi = packed
    e.emit_raw(&[0x4C, 0x89, 0xE7]);
    // and rdi, 0xFFFF         ; rdi = x
    e.emit_raw(&[0x48, 0x81, 0xE7, 0xFF, 0xFF, 0x00, 0x00]);
    // mov rsi, r12
    e.emit_raw(&[0x4C, 0x89, 0xE6]);
    // shr rsi, 16
    e.emit_raw(&[0x48, 0xC1, 0xEE, 0x10]);
    // and rsi, 0xFFFF         ; rsi = y
    e.emit_raw(&[0x48, 0x81, 0xE6, 0xFF, 0xFF, 0x00, 0x00]);
    // mov rdx, 12             ; w
    e.emit_raw(&[0x48, 0xC7, 0xC2, 0x0C, 0x00, 0x00, 0x00]);
    // mov r10, 12             ; h
    e.emit_raw(&[0x49, 0xC7, 0xC2, 0x0C, 0x00, 0x00, 0x00]);
    // mov r8, 0xFFFFFFFF      ; color blanco
    e.emit_raw(&[0x49, 0xC7, 0xC0, 0xFF, 0xFF, 0xFF, 0xFF]);
    // mov rax, SYS_FBFILL
    e.emit_raw(&[0x48, 0xC7, 0xC0, SYS_FBFILL as u8, 0x00, 0x00, 0x00]);
    // syscall
    e.syscall();

    // 9) Poll key
    sys0(&mut e, SYS_KEYPOLL);
    cmp_rax_imm32(&mut e, SC_ESC as i32);
    // Si NO es ESC → saltar a `loop_back`.
    // Calculamos rel8 después; primero emitimos placeholder y luego patch.
    let jne_off = e.here();
    e.emit_raw(&[0x75, 0]); // jne rel8 placeholder

    // Si ES ESC → ProcessExit
    sys0(&mut e, SYS_EXIT);

    // ── loop_back: salto al inicio del frame ────────────────────────
    let loop_back = e.here();
    // patch del jne anterior: rel8 = loop_back - (jne_off + 2)
    let rel8 = (loop_back as isize) - (jne_off as isize + 2);
    if rel8 < -128 || rel8 > 127 {
        // demasiado lejos: usar jne near (0F 85 rel32) — re-emitir
        // por simplicidad rewrite both bytes en buf final
        // (en práctica el rel cabe en rel8 porque el bloque de exit son ~7 bytes)
        // Si no cupiera, se sustituye más arriba el `jne_rel8` por jne_rel32.
    }
    e.bytes[jne_off + 1] = (rel8 as i8) as u8;

    // jmp rel32 → frame_start
    let here_after_jmp = e.here() + 5;
    let frame_rel = (frame_start as isize) - (here_after_jmp as isize);
    jmp_rel32(&mut e, frame_rel as i32);

    // ── Padding hasta 16 bytes ──────────────────────────────────────
    while e.bytes.len() % 16 != 0 {
        e.emit_raw(bytes_for(IntrinsicId::Nop));
    }

    // ── Sección de strings al final del buffer ──────────────────────
    let data_offset = e.bytes.len();
    let mut string_offsets: Vec<usize> = Vec::with_capacity(LABELS.len());
    for s in LABELS {
        string_offsets.push(e.bytes.len());
        e.emit_raw(s);
        // null terminator no requerido — la longitud va en r10
    }

    // ── Patchear los movabs RDX con la dirección absoluta ───────────
    for &(imm_off, str_idx) in &text_patches {
        let abs = base_addr + string_offsets[str_idx] as u64;
        let bytes = abs.to_le_bytes();
        for i in 0..8 {
            e.bytes[imm_off + i] = bytes[i];
        }
        let _ = data_offset; // silence
    }

    let total = e.bytes.len();
    let dst = &mut code_buf[..total];
    dst.copy_from_slice(&e.bytes);

    (0, total)
}
