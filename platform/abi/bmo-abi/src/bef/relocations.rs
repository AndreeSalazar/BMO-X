//! Relocations BEF — solo 3 tipos (vs 38 de ELF x86_64, 16 de PE).
//!
//! Modelo BEF:
//!   - **Abs64**  — escribir dirección absoluta de 64 bits.
//!   - **Rel32**  — escribir delta de 32 bits (PC-relative).
//!   - **Got64**  — escribir dirección via Global Offset Table.
//!
//! Eso cubre el 100 % de los casos que ELF resuelve con sus 38 tipos. El
//! resto eran legacy (R_X86_64_8, R_X86_64_16, R_X86_64_TPOFF*, etc.).

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_i64, bx_u32, bx_u64, bx_u8};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RelocationKind {
    /// Escribe `symbol_addr + addend` (64 bits absolutos).
    /// ELF: `R_X86_64_64`. PE: `IMAGE_REL_BASED_DIR64`.
    Abs64 = 0x01,
    /// Escribe `symbol_addr + addend - reloc_addr` (32 bits, PC-relative).
    /// ELF: `R_X86_64_PC32`/`R_X86_64_PLT32`.
    Rel32 = 0x02,
    /// Escribe la dirección del slot GOT del símbolo (64 bits).
    /// ELF: `R_X86_64_GLOB_DAT`/`R_X86_64_JUMP_SLOT`.
    Got64 = 0x03,
    /// ★ Escribe la dirección de **una posición dentro de otra sección de este
    /// mismo binario**, 64 bits. No hay símbolo de por medio.
    ///
    /// `symbol_idx` no es un símbolo aquí: es **el código de sección donde vive
    /// el destino**, con la misma numeración que [`Relocation::target_section`]
    /// (`0` = code, `1` = data, `2` = rodata). Y `addend` es el **offset dentro
    /// de esa sección**.
    ///
    /// # Por qué hacía falta un tipo nuevo
    ///
    /// Los otros tres apuntan a un SÍMBOLO, y BMO **no tiene tabla de
    /// símbolos**: la sección `Symbols` está declarada y nadie la escribe. Lo
    /// que un `.bex` de una sola unidad necesita no es "la dirección de
    /// `printf`" — es "la dirección de la cadena que está en rodata+40", que el
    /// compilador no puede saber porque depende de dónde cargue el programa.
    ///
    /// El caso que lo pidió:
    ///
    /// ```c
    /// char *mapa = "1111...";        // global: guarda una DIRECCIÓN
    /// char *nombres[] = {"a", "b"};  // tabla de punteros
    /// ```
    ///
    /// Antes esto valía **cero en silencio** — y así estuvo el raycaster
    /// leyendo su mapa desde el byte 0 de su propio código.
    ///
    /// # Por qué no se reusó `symbol_idx` sin un tipo propio
    ///
    /// Porque eso es *un campo con dos significados según el contexto*, que es
    /// exactamente el bug que se acababa de arreglar en `bef::writer` (el
    /// `alignment` que servía a la vez para memoria y para fichero, y metía 6
    /// KB de agujeros). Con un `kind` distinto, el significado del campo lo
    /// dice el propio dato y no hace falta saber de dónde vino.
    ///
    /// Aplicar esto es idéntico a [`RelocationKind::Abs64`] —escribir
    /// `destino + addend`— y la diferencia entera está en quién resuelve el
    /// `destino`: allí un símbolo, aquí la VA de una sección.
    SeccionAbs64 = 0x04,
}

impl RelocationKind {
    pub fn from_u8(v: bx_u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Abs64),
            0x02 => Some(Self::Rel32),
            0x03 => Some(Self::Got64),
            0x04 => Some(Self::SeccionAbs64),
            _ => None,
        }
    }
}

/// Una relocation — 24 bytes.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct Relocation {
    /// Offset en la sección target donde aplicar la reloc.
    pub offset: bx_u64,
    /// Índice del símbolo en la sección Symbols (o Imports).
    pub symbol_idx: bx_u32,
    /// Tipo `RelocationKind as u8`.
    pub kind: bx_u8,
    /// `0` = sección target es `.code`, `1` = `.data`, `2` = `.rodata`.
    pub target_section: bx_u8,
    /// Padding.
    pub _pad: [bx_u8; 2],
    /// Addend con signo.
    pub addend: bx_i64,
}
const _: () = assert!(core::mem::size_of::<Relocation>() == 24);

/// ⚠️ LOS CÓDIGOS DE SECCIÓN DE UNA RELOCATION **NO SON LOS DE
/// `SectionKind`**, y esto es una trampa real del formato.
///
/// ```text
///                code   rodata   data
///   SectionKind    1       2       3
///   aquí           0       2       1     <-- ojo: data y rodata cambian
/// ```
///
/// Es la numeración que ya usaba [`Relocation::target_section`] desde que se
/// escribió el struct, y **no se cambia** porque cambiarla ahora rompería
/// cualquier `.bex` que llevara relocs — aunque hoy no haya ninguno, la regla
/// de este proyecto es que un formato publicado no se toca por comodidad.
///
/// Lo que sí se hace es darles nombre, para que nadie vuelva a escribir un `1`
/// creyendo que es rodata: escribir `SEC_DATA` no se puede confundir.
pub const SEC_CODE: bx_u8 = 0;
/// Ver [`SEC_CODE`]. **Vale 1, no 3.**
pub const SEC_DATA: bx_u8 = 1;
/// Ver [`SEC_CODE`]. **Vale 2, igual que en `SectionKind` por casualidad.**
pub const SEC_RODATA: bx_u8 = 2;

impl Relocation {
    pub const SIZE: usize = 24;

    pub fn kind(&self) -> Option<RelocationKind> {
        RelocationKind::from_u8(self.kind)
    }

    /// Una [`RelocationKind::SeccionAbs64`]: *"en `donde_sec + donde_off`
    /// escribe la dirección de `destino_sec + destino_off`"*.
    ///
    /// Existe para que los dos pares (sección, offset) se pasen por nombre. A
    /// mano hay que poner el destino en `symbol_idx` y en `addend`, que no se
    /// llaman como lo que llevan — y con cuatro números del mismo tipo, cruzar
    /// dos es cuestión de tiempo.
    pub fn seccion_abs64(
        donde_sec: bx_u8,
        donde_off: bx_u64,
        destino_sec: bx_u8,
        destino_off: bx_i64,
    ) -> Self {
        Self {
            offset: donde_off,
            symbol_idx: destino_sec as bx_u32,
            kind: RelocationKind::SeccionAbs64 as bx_u8,
            target_section: donde_sec,
            _pad: [0; 2],
            addend: destino_off,
        }
    }
}

/// Aplica una relocation single sobre un buffer mutable que representa la
/// sección target ya cargada en memoria.
///
/// `reloc_va` es la dirección virtual final de `target[reloc.offset]`.
/// `symbol_addr` es la dirección virtual final del símbolo.
pub fn apply(
    reloc: &Relocation,
    target: &mut [u8],
    reloc_va: u64,
    symbol_addr: u64,
) -> Result<(), &'static str> {
    let off = reloc.offset as usize;
    let kind = reloc.kind().ok_or("kind de relocation desconocido")?;
    match kind {
        // Los dos escriben `destino + addend` en 64 bits. La diferencia está en
        // quién resolvió `symbol_addr` antes de llegar aquí: en `Abs64` es la
        // dirección de un símbolo; en `SeccionAbs64`, la de la SECCIÓN que
        // nombra `symbol_idx`. Comparten el brazo a propósito — dos copias del
        // mismo `copy_from_slice` serían dos sitios donde equivocarse.
        RelocationKind::Abs64 | RelocationKind::SeccionAbs64 => {
            if off + 8 > target.len() {
                return Err("offset Abs64 fuera de rango");
            }
            let v = symbol_addr.wrapping_add(reloc.addend as u64);
            target[off..off + 8].copy_from_slice(&v.to_le_bytes());
        }
        RelocationKind::Rel32 => {
            if off + 4 > target.len() {
                return Err("offset Rel32 fuera de rango");
            }
            let pc = reloc_va as i64;
            let v = (symbol_addr as i64)
                .wrapping_add(reloc.addend)
                .wrapping_sub(pc);
            target[off..off + 4].copy_from_slice(&(v as i32).to_le_bytes());
        }
        RelocationKind::Got64 => {
            if off + 8 > target.len() {
                return Err("offset Got64 fuera de rango");
            }
            // En BEF, el GOT slot ya fue resuelto por el loader; aquí escribimos
            // su dirección. El addend se suma como offset dentro del slot (raro).
            let v = symbol_addr.wrapping_add(reloc.addend as u64);
            target[off..off + 8].copy_from_slice(&v.to_le_bytes());
        }
    }
    Ok(())
}
