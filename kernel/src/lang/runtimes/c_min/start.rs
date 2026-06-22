//! `c_min::start` — Punto de entrada `_start`.
//!
//! El linker pone este símbolo como entry point del binario BEF.
//! Se encarga de:
//! 1. Configurar argc/argv desde el stack del kernel.
//! 2. Llamar a `main(argc, argv)`.
//! 3. Pasar el retorno a `_exit(rc)`.
//!
//! ## ABI esperado del kernel
//!
//! Cuando el kernel hace `iretq` al entry point, el stack tiene:
//! - `[rsp]`     = argc (u64)
//! - `[rsp+8]`   = argv[0]
//! - `[rsp+16]`  = argv[1]
//! - ...
//! - `[rsp+8*(argc+1)]` = 0 (null terminator)
//!
//! `main` recibe `argc` en `RDI` y `argv` en `RSI` (SysV AMD64).

#![allow(dead_code)]

/// Bytes del entry point `_start` (x86-64).
///
/// v1.8.8: solo declaraciones. El linker los emite al inicio del binario.
pub const _START_BYTES: &[u8] = &[
    // Sub rsp, 8 (align stack to 16)
    0x48, 0x83, 0xEC, 0x08,
    // Mov rdi, [rsp+8]  (argc)
    0x48, 0x8B, 0x7C, 0x24, 0x10,
    // Lea rsi, [rsp+16] (argv)
    0x48, 0x8D, 0x74, 0x24, 0x18,
    // Call main
    0xE8, 0x00, 0x00, 0x00, 0x00, // rel32 = 0, parchado por linker
    // Mov rdi, rax
    0x48, 0x89, 0xC7,
    // Call _exit
    0xE8, 0x00, 0x00, 0x00, 0x00, // rel32 = 0, parchado por linker
    // Hlt (nunca llega aquí)
    0xF4,
];

/// Símbolo que el linker busca.
pub const _START_SYMBOL: &str = "_start";
