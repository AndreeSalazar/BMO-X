//! `Token` y `TokenKind` — atomos léxicos de BMO simple.
//!
//! Filosofía: cada keyword es **semántica viviente** que el emisor traduce
//! a bytes precisos. Algo que ningún ASM clásico tiene:
//!   - `pausa`          emite `F3 90` (PAUSE — hint al CPU en spin-loops).
//!   - `atomico { ... }` envuelve un bloque con LOCK prefix automático.
//!   - `cuando cf { }`   condiciona ejecución según CPU flag (sin `jcc` manual).
//!   - `paralelo`       sugiere vectorización SIMD a la siguiente iteración.
//!   - `seccion .code`  cambia section directive sin macros del assembler.
//!   - `align 64`       inserta NOPs hasta alinear a cache line.
//!   - `volatil ptr`    fuerza barrera de optimización.
//!
//! Total: 90+ keywords. Comparado: NASM ~25 directivas + ~1500 mnemonics
//! que dependen del CPU. BMO simple: 90 keywords **cross-CPU portables**.

use crate::barex::abi::primitives::{bx_u32, bx_u64};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // ── Keywords base (fase 1) ───────────────────────────────────────
    KwDef       = 0x01,
    KwLet       = 0x02,
    KwSi        = 0x03,
    KwSino      = 0x04,
    KwMientras  = 0x05,
    KwRetorna   = 0x06,
    KwReg       = 0x07,
    KwEmit      = 0x08,
    KwAloc      = 0x09,
    KwLibre     = 0x0A,

    // ── Tipos básicos ────────────────────────────────────────────────
    TyByte      = 0x10,
    TyNum       = 0x11,
    TyPtr       = 0x12,
    TyArr       = 0x13,
    TyRef       = 0x14,

    // ── Aritmética y lógica (operadores keyword) ─────────────────────
    OpSuma      = 0x20,
    OpResta     = 0x21,
    OpMult      = 0x22,
    OpDiv       = 0x23,
    OpY         = 0x24,
    OpO         = 0x25,
    OpNo        = 0x26,
    OpIgual     = 0x27,
    OpMayor     = 0x28,
    OpMenor     = 0x29,
    OpMod       = 0x2A,   // `mod` — resto
    OpXor       = 0x2B,   // `xor` — bitwise XOR
    OpShl       = 0x2C,   // `shl` — shift left
    OpShr       = 0x2D,   // `shr` — shift right
    OpRol       = 0x2E,   // `rol` — rotate left
    OpRor       = 0x2F,   // `ror` — rotate right

    // ── Control de flujo ─────────────────────────────────────────────
    KwRompe     = 0x30,
    KwContinua  = 0x31,
    KwMatch     = 0x32,
    LitNulo     = 0x33,
    KwCaso      = 0x34,   // `caso` — arm de match
    KwDefecto   = 0x35,   // `defecto` — match catch-all
    KwPara      = 0x36,   // `para i desde 0 hasta N` — for loop
    KwBucle     = 0x37,   // `bucle { ... }` — infinite loop
    KwDesde     = 0x38,   // range start
    KwHasta     = 0x39,   // range end
    KwPaso      = 0x3A,   // range step
    KwSalto     = 0x3B,   // `salto etiqueta` — jmp directo
    KwEtiqueta  = 0x3C,   // `etiqueta nombre:` — label decl
    KwCuando    = 0x3D,   // `cuando cf { }` — condicional por CPU flag
    KwTabla     = 0x3E,   // `tabla salto[N]` — jump table

    // ── OOP (fase 2) ─────────────────────────────────────────────────
    KwTipo      = 0x40,
    KwImpl      = 0x41,
    KwNuevo     = 0x42,
    KwMio       = 0x43,   // `mio T` — ownership move
    KwPrest     = 0x44,   // `prest T` — borrow ref
    KwMut       = 0x45,   // mutabilidad
    KwConst     = 0x46,   // compile-time constant
    KwPuro      = 0x47,   // función pura (sin efectos)

    // ── UI / apps (fase 3) ───────────────────────────────────────────
    KwVentana   = 0x50,
    KwEvento    = 0x51,
    KwDibuja    = 0x52,

    // ── Intrínsecos CPU — emiten bytes específicos directamente ──────
    KwNop       = 0x60,   // → 0x90
    KwPausa     = 0x61,   // → 0xF3 0x90 (PAUSE, hint spin-loop)
    KwInt3      = 0x62,   // → 0xCC (breakpoint)
    KwHlt       = 0x63,   // → 0xF4 (halt)
    KwCli       = 0x64,   // → 0xFA (clear interrupt flag, ring 0)
    KwSti       = 0x65,   // → 0xFB (set interrupt flag, ring 0)
    KwRdtsc     = 0x66,   // → 0x0F 0x31
    KwCpuid     = 0x67,   // → 0x0F 0xA2
    KwLfence    = 0x68,   // → 0x0F 0xAE 0xE8
    KwMfence    = 0x69,   // → 0x0F 0xAE 0xF0
    KwSfence    = 0x6A,   // → 0x0F 0xAE 0xF8
    KwSyscall   = 0x6B,   // → 0x0F 0x05

    // ── Memoria / consistencia ───────────────────────────────────────
    KwAtomico   = 0x70,   // bloque con LOCK prefix (0xF0)
    KwVolatil   = 0x71,   // suprime optimización del load/store
    KwAcquire   = 0x72,   // memory order acquire
    KwRelease   = 0x73,   // memory order release
    KwRelax     = 0x74,   // memory order relaxed
    KwBarr      = 0x75,   // barrera (full mem barrier)
    KwCerca     = 0x76,   // `cerca ptr` — prefetch a L1
    KwMovnt     = 0x77,   // non-temporal store hint

    // ── Vectorización / paralelismo ──────────────────────────────────
    KwParalelo  = 0x80,   // hint SIMD AVX2/AVX-512
    KwSincro    = 0x81,   // barrera de sincronización entre threads
    KwIntrinseco= 0x82,   // `intrinseco nombre` — keyword → opcode

    // ── Directivas (no emiten código, controlan el emisor) ───────────
    KwSeccion   = 0x90,   // `seccion .code` / `.data` / `.rodata` / `.bss`
    KwAlign     = 0x91,   // `align 64` — inserta NOPs hasta alineación
    KwRepetir   = 0x92,   // `repetir N { ... }` — unroll en compile-time
    KwIncluye   = 0x93,   // `incluye "otro.bmo"`
    KwComen     = 0x94,   // bloque de comentario explícito
    KwFin       = 0x95,   // cierre genérico de bloque

    // ── CPU Flags como identificadores (para `cuando`) ───────────────
    FlagCf      = 0xA0,   // carry flag
    FlagZf      = 0xA1,   // zero flag
    FlagSf      = 0xA2,   // sign flag
    FlagOf      = 0xA3,   // overflow flag
    FlagPf      = 0xA4,   // parity flag
    FlagDf      = 0xA5,   // direction flag

    // ── Léxico estructural ───────────────────────────────────────────
    Ident       = 0xB0,
    LitInt      = 0xB1,
    LitHex      = 0xB2,   // 0x... literal
    LitBin      = 0xB3,   // 0b... literal
    LitStr      = 0xB4,
    LitByte     = 0xB5,
    Comment     = 0xB6,   // // ... \n

    LBrace      = 0xC0,   // {
    RBrace      = 0xC1,   // }
    LParen      = 0xC2,   // (
    RParen      = 0xC3,   // )
    LBracket    = 0xC4,   // [
    RBracket    = 0xC5,   // ]
    Comma       = 0xC6,   // ,
    Colon       = 0xC7,   // :
    Semicolon   = 0xC8,   // ;
    Arrow       = 0xC9,   // ->
    Assign      = 0xCA,   // =
    Dot         = 0xCB,   // .

    Eof         = 0xFE,
    Unknown     = 0xFF,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub kind: TokenKind,
    pub _pad: [u8; 3],
    /// Offset de inicio en el source (bytes).
    pub start: bx_u32,
    /// Longitud en bytes.
    pub len: bx_u32,
    /// Valor numérico para `LitInt` / `LitHex` / `LitBin` / `LitByte`.
    pub value: bx_u64,
}

impl Token {
    pub const EOF: Self = Self {
        kind: TokenKind::Eof,
        _pad: [0; 3],
        start: 0, len: 0, value: 0,
    };
}
