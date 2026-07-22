//! `bmo-lower` — Descenso al ABI (2ª librería del pipeline).
//!
//! Traduce las **exigencias** de cada lenguaje a operaciones del BMO ABI
//! congelado: `INVOKE` / `CHANNEL_KICK` / `WAIT` + subsyscalls. Ejemplo:
//! el `DISPLAY` de COBOL y el `printf` de C bajan al MISMO nodo lógico
//! "print" → `INVOKE(console, CONSOLE_WRITE, bytes)`.
//!
//! Mantiene la esencia: el lenguaje decide QUÉ baja; esta librería solo
//! provee el **vocabulario del ABI** (cómo se ve un INVOKE). Es opcional:
//! un frontend la enlaza si no quiere reimplementar la emisión del ABI.
//!
//! Hoy esa emisión está duplicada — `lang/cobol/ir_emit.rs` construye
//! `IrExpr::Syscall{..}` a mano; `lang/c` hace lo suyo. Aquí se centraliza
//! el PATRÓN (no el flujo), como librería que se llama.
//!
//! Estado: esqueleto. Paso de migración 4.

/// Los 3 syscalls congelados (espejo de bmo-abi surface — fuente única).
pub const NR_INVOKE: u32 = 0x00;
pub const NR_CHANNEL_KICK: u32 = 0x01;
pub const NR_WAIT: u32 = 0x02;

/// Pseudo-capability de la tarea actual.
pub const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;

/// Subsyscalls sobre la tarea (espejo del kernel; el drift-guard de build.ps1
/// valida que no diverjan).
pub const TASK_OP_EXIT: u64 = 0x04;
pub const TASK_OP_CONSOLE_WRITE: u64 = 0x06;

/// TODO(migración paso 4): API tipo
/// `fn lower_print(bytes: &[u8]) -> Vec<IrStmt>` y
/// `fn lower_exit() -> Vec<IrStmt>` que ambos frontends llamen en vez de
/// construir el `IrExpr::Syscall` a mano.
pub fn abi_frozen() -> bool {
    true
}
