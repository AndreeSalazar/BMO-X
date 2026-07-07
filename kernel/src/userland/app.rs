//! userland::app - Cargar y ejecutar apps de Ring 3 desde BEF/ELF.
//!
//! v1.8.8: API de alto nivel que:
//! 1. Detecta el formato del binario (BEF nativo, ELF).
//! 2. Lo carga con `bef::parsers::load` (que lo "devora" si no es BEF).
//! 3. Lo mapea en memoria.
//! 4. Salta a Ring 3 (`parsers::run_entry_point`).
//!
//! Esta funcion es la **puerta de userland** desde BMO Core. El
//! welcome screen (o un futuro shell) llama a esta funcion para
//! lanzar apps desde BEFs.

#![allow(dead_code)]

use bmo_core::bef::parsers::{self, BinaryFormat, Image, LoadError};
use bmo_core::bef::format::header::BefMagic;
use bmo_core::desktop3;

/// Carga y ejecuta un binario (BEF/ELF).
///
/// # Argumentos
///
/// - `bytes`: contenido del binario (puede ser BEF o ELF).
/// - `name`: nombre legible para Cabina.
///
/// # Comportamiento
///
/// 1. Detecta el formato con `BefMagic::detect`.
/// 2. Carga con `bef::parsers::load` (devora si no es BEF nativo).
/// 3. Si el formato es devorado (ELF), lo traduce a BEF interno.
/// 4. Salta a Ring 3.
///
/// # Retorno
///
/// Esta funcion **NO retorna** en exito (salta a Ring 3 con iretq).
/// En error, retorna `false` y el caller puede continuar.
pub fn run(bytes: &[u8], name: &str) -> bool {
    crate::cabina_daemon::info("userland", &alloc::format!("launching: {}", name));

    // 1. Detectar formato.
    let fmt = BefMagic::detect(bytes);
    let fmt_name = match fmt {
        BefMagic::BefNative => "BEF",
        BefMagic::ElfUnix => "ELF (devoured)",
        _ => "unknown",
    };
    crate::cabina_daemon::info("userland", &alloc::format!("format: {}", fmt_name));

    // 2. Cargar (devora si es ELF).
    let img = match parsers::load(bytes) {
        Ok(i) => i,
        Err(e) => {
            crate::cabina_daemon::fault("userland",
                &alloc::format!("load failed: {:?}", e));
            return false;
        }
    };

    // 3. Validar imagen.
    if img.entry_point == 0 {
        crate::cabina_daemon::fault("userland", "entry point is NULL");
        return false;
    }

    if img.format == BinaryFormat::BefNative {
        crate::cabina_daemon::info("userland", "BEF native - direct jump");
    } else {
        crate::cabina_daemon::info("userland", &alloc::format!("{:?} devorado y traducido a BEF",
                                                       img.format));
    }

    // 4. Cabina: evento de auditoria antes de saltar a Ring 3.
    desktop3::observe_launch(name, img.format);

    // 5. Saltar a Ring 3 (no retorna - divergente).
    unsafe { parsers::run_entry_point(&img); }
}

/// Carga un binario sin ejecutarlo (para tests / introspeccion).
pub fn load_only(bytes: &[u8]) -> Result<Image, LoadError> {
    parsers::load(bytes)
}

/// Lista de formatos soportados (para Cabina).
pub const SUPPORTED_FORMATS: &[&str] = &[
    "BEF1 (BMO native)",
    "\\x7FELF (Linux/Unix - devoured)",
];
