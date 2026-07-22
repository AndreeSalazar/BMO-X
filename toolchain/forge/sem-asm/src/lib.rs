//! Motor de codificación semántica (`sem-asm`) — la 3ª librería del pipeline.
//!
//! Lee las tablas TOML de `tables/` (movidas aquí desde `toolchain/sem-asm`)
//! y encodea instrucciones → bytes precisos. Reemplaza el hardcodeo de bytes
//! que hoy está DUPLICADO en `lang/c/src/codegen.rs` y
//! `lang/cobol/src/codegen.rs` (ambos escriben 0x48/0xB8… a mano).
//!
//! Es una **librería que el frontend ELIGE enlazar**, no un embudo. C/C++
//! pueden bajar directo aquí para control de bytes (= inline asm); COBOL la
//! usa si quiere o trae su propio encoder.
//!
//! Estado: esqueleto. El siguiente paso es leer
//! `tables/arch/x86_64/instructions.toml` y exponer `encode(mnemonic, ops)`.

use std::path::{Path, PathBuf};

/// Ubica el directorio `tables/` relativo a este crate, sin depender de las
/// rutas muertas `X:\FastOS\...\Semantic_ASM` que arrastra el frontend de C.
pub fn tables_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tables")
}

/// Bytes de una instrucción codificada. `Vec<u8>` porque una instrucción
/// x86-64 varía de 1 a 15 bytes.
pub type Encoded = Vec<u8>;

/// TODO(migración paso 2): parsear `tables/arch/x86_64/instructions.toml`
/// a esta tabla en memoria y encodear por mnemónico + operandos.
///
/// Firma objetivo:
/// ```ignore
/// pub fn encode(mnemonic: &str, operands: &[Operand]) -> Result<Encoded, EncodeError>;
/// ```
/// Por ahora expone el acceso a las tablas para que el resto del pipeline ya
/// pueda compilar contra este crate mientras se construye el motor.
pub fn tables_available() -> bool {
    tables_dir().join("arch/x86_64/instructions.toml").exists()
}
