//! **Un `.bex` entero, montado en el emulador como lo monta el cargador del
//! kernel** (2026-09-18).
//!
//! Esta funcion estaba COPIADA en cuatro bancos de prueba (C, C++, Ada y
//! `bmo-enlazar`), cada uno con su version. Aqui vive la canonica, al lado del
//! emulador, y es la que usa el metro del emisor (`toolchain/tools/metro`).
//!
//! Hace lo que el cargador: las secciones en paginas de 4 KiB (codigo, luego
//! RoData, Data y Bss), y las relocaciones `SeccionAbs64` resueltas contra la
//! base de cada una.

use bmo_abi::bef::relocations::{Relocation, RelocationKind};
use bmo_abi::bef::sections::{SectionEntry, SectionKind};

use super::Machine;

/// Monta `bex` y deja el `rip` en su punto de entrada. Un `.bex` malformado
/// es un `Err` con el motivo, no un panico a medias.
pub fn cargar_bex(bex: &[u8]) -> Result<Machine, String> {
    const PAGE: usize = 4096;
    let u64_en = |i: usize| -> Result<u64, String> {
        bex.get(i..i + 8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| format!("el .bex se acaba en el byte {i}"))
    };
    let entrada = u64_en(24)? as usize;
    let tabla = u64_en(32)? as usize;
    let cuantas = bex.get(40..44)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
        .ok_or("el .bex no llega ni a la cabecera")?;

    let mut imagen = Vec::new();
    let mut base = [usize::MAX; 3];
    for (kind, cod) in [
        (SectionKind::Code, 0usize),
        (SectionKind::RoData, 2usize),
        (SectionKind::Data, 1usize),
        (SectionKind::Bss, usize::MAX),
    ] {
        for i in 0..cuantas {
            let e = tabla + i * SectionEntry::SIZE;
            if *bex.get(e).ok_or("la tabla de secciones se sale del .bex")? != kind as u8 {
                continue;
            }
            let off = u64_en(e + 8)? as usize;
            let size = u64_en(e + 16)? as usize;
            let mem = u64_en(e + 24)? as usize;
            while !imagen.is_empty() && imagen.len() % PAGE != 0 {
                imagen.push(0xCC);
            }
            if cod != usize::MAX {
                base[cod] = imagen.len();
            }
            imagen.extend_from_slice(bex.get(off..off + size).ok_or("una seccion se sale del .bex")?);
            imagen.resize(imagen.len() + mem.saturating_sub(size), 0);
        }
    }
    for i in 0..cuantas {
        let e = tabla + i * SectionEntry::SIZE;
        if bex[e] != SectionKind::Relocs as u8 {
            continue;
        }
        let off = u64_en(e + 8)? as usize;
        let size = u64_en(e + 16)? as usize;
        for k in 0..size / Relocation::SIZE {
            let r = off + k * Relocation::SIZE;
            let donde = u64_en(r)? as usize;
            let destino = u32::from_le_bytes(bex[r + 8..r + 12].try_into().unwrap()) as usize;
            let kind = bex[r + 12];
            let donde_sec = bex[r + 13] as usize;
            let addend = i64::from_le_bytes(bex[r + 16..r + 24].try_into().unwrap());
            if kind != RelocationKind::SeccionAbs64 as u8 {
                return Err(format!("reloc de tipo {kind}: el cargador solo aplica SeccionAbs64"));
            }
            let (Some(&b_donde), Some(&b_destino)) = (base.get(donde_sec), base.get(destino)) else {
                return Err("una reloc nombra una seccion que no existe".into());
            };
            let at = b_donde + donde;
            let valor = (b_destino as i64 + addend) as u64;
            imagen.get_mut(at..at + 8).ok_or("una reloc cae fuera de la imagen")?
                .copy_from_slice(&valor.to_le_bytes());
        }
    }
    let mut m = Machine::new(imagen);
    m.rip = entrada;
    Ok(m)
}
