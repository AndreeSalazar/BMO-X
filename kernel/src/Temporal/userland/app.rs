//! `userland::app` — Cargar y ejecutar apps de Ring 3 desde BEF/PE/ELF.
//!
//! v1.8.8: API de alto nivel que:
//! 1. Detecta el formato del binario (BEF nativo, PE, ELF).
//! 2. Lo carga con `bef::loader::load` (que lo "devora" si no es BEF).
//! 3. Lo mapea en memoria.
//! 4. Salta a Ring 3 (`loader::run_entry_point`).
//!
//! Esta función es la **puerta de userland** desde BMO Core. El
//! welcome screen (o un futuro shell) llama a esta función para
//! lanzar apps desde BEFs.

#![allow(dead_code)]

use crate::bmo_core::bef::loader::{self, BinaryFormat, Image};
use crate::bmo_core::bef::header::BefMagic;
use crate::bmo_core::desktop3;

/// Carga y ejecuta un binario (BEF/PE/ELF).
///
/// # Argumentos
///
/// - `bytes`: contenido del binario (puede ser BEF, PE, o ELF).
/// - `name`: nombre legible para Cabina.
///
/// # Comportamiento
///
/// 1. Detecta el formato con `BefMagic::detect`.
/// 2. Carga con `bef::loader::load` (devora si no es BEF nativo).
/// 3. Si el formato es devorado (PE/ELF), lo traduce a BEF interno.
/// 4. Salta a Ring 3.
///
/// # Retorno
///
/// Esta función **NO retorna** en éxito (salta a Ring 3 con iretq).
/// En error, retorna `false` y el caller puede continuar.
pub fn run(bytes: &[u8], name: &str) -> bool {
    crate::cabina::info("userland", &alloc::format!("launching: {}", name));

    // 1. Detectar formato.
    let fmt = BefMagic::detect(bytes);
    let fmt_name = match fmt {
        BefMagic::BefNative => "BEF",
        BefMagic::PeWindows => "PE (devoured)",
        BefMagic::ElfUnix => "ELF (devoured)",
        BefMagic::Unknown => "unknown",
    };
    crate::cabina::info("userland", &alloc::format!("format: {}", fmt_name));

    // 2. Cargar (devora si es PE/ELF).
    let img = match loader::load(bytes) {
        Ok(i) => i,
        Err(e) => {
            crate::cabina::fault("userland",
                &alloc::format!("load failed: {:?}", e));
            return false;
        }
    };

    // 3. Validar imagen.
    if img.entry_point == 0 {
        crate::cabina::fault("userland", "entry point is NULL");
        return false;
    }

    if img.format == BinaryFormat::BefNative {
        crate::cabina::info("userland", "BEF native — direct jump");
    } else {
        crate::cabina::info("userland", &alloc::format!("{:?} devorado y traducido a BEF",
                                                       img.format));
    }

    // 4. Cabina: evento de auditoría antes de saltar a Ring 3.
    desktop3::observe_launch(name, img.format);

    // 5. Saltar a Ring 3 (no retorna en éxito).
    unsafe { loader::run_entry_point(&img); }
}

/// Carga un binario sin ejecutarlo (para tests / introspección).
pub fn load_only(bytes: &[u8]) -> Result<Image, loader::LoadError> {
    loader::load(bytes)
}

/// Lista de formatos soportados (para Cabina).
pub const SUPPORTED_FORMATS: &[&str] = &[
    "BEF1 (FastOS native)",
    "MZ (PE/Windows .exe/.dll — devoured)",
    "\\x7FELF (Linux/Unix — devoured)",
];
