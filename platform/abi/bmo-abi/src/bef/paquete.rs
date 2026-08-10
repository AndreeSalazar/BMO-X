//! **Empaquetar: meterle los datos a un `.bex` que ya existe.**
//!
//! ## Que hace y por que no lo hace el compilador
//!
//! Un `.bex` sale del frontend con codigo y nada mas. Los datos de la app --el
//! WAD de DOOM, una fuente, un fichero de configuracion-- no los conoce el
//! compilador y no tiene por que: llegan **despues**, y de quien monta la app.
//!
//! Asi que esto es un paso posterior, y por eso es una funcion y no una opcion
//! del codegen. Coge la imagen ya emitida, le anade la seccion
//! [`SectionKind::Resources`] con su indice, y vuelve a escribir el fichero.
//!
//! ## Por que se REEMITE y no se pega al final
//!
//! Pegar al final seria mas facil --es lo que hace un ZIP, con su directorio en
//! la cola-- y seria un formato **distinto** conviviendo con BEF. La seccion
//! `0x0B` ya esta en el formato desde que se diseno; lo que faltaba era
//! escribirla.
//!
//! El precio de reemitir es que la tabla de secciones crece una entrada y
//! **todos los offsets en fichero se mueven**. Eso da igual, y conviene saber
//! por que da igual:
//!
//! - el cargador lee `file_offset` de la TABLA, no de una constante;
//! - las relocations de BMO son **relativas a la seccion** (`SeccionAbs64`
//!   guarda el indice de la seccion destino y un offset dentro de ella), asi
//!   que mover las secciones no las invalida **mientras no cambie su ORDEN**;
//! - `entry_offset` es un desplazamiento dentro de `.code`, no una direccion.
//!
//! ** De ahi la regla que esta funcion cumple y que hay que no romper: **la
//! seccion nueva va SIEMPRE al final**, y las que ya estaban conservan su
//! indice. Insertarla en medio renumeraria las secciones y cada relocation
//! apuntaria a la de al lado -- un programa que carga, arranca, y lee sus
//! cadenas de otro sitio.

#![allow(dead_code)]

use crate::bmo_abi::bef::{
    header::BEF_MAGIC,
    recursos,
    sections::{SectionEntry, SectionFlags, SectionKind},
    writer::{BefBuilder, BefSection},
};
use crate::bmo_abi::primitives::bx_u64;
use alloc::vec::Vec;

/// Offsets dentro del header de 48 bytes. Se leen a mano y no con un `cast`
/// del struct: el buffer de un fichero no tiene por que estar alineado a 8, y
/// `SectionEntry` lo exige. Un `transmute` sobre un `Vec<u8>` funciona casi
/// siempre, que es la peor frecuencia posible.
const H_MAGIC: usize = 0;
const H_ENTRY_OFFSET: usize = 24;
const H_TABLE_OFFSET: usize = 32;
const H_SECTION_COUNT: usize = 40;
const HEADER_LEN: usize = 48;

/// Offsets dentro de una entrada de 48 bytes.
const E_KIND: usize = 0;
const E_FLAGS: usize = 4;
const E_FILE_OFFSET: usize = 8;
const E_FILE_SIZE: usize = 16;
const E_MEM_SIZE: usize = 24;
const E_ALIGNMENT: usize = 40;

fn u16_en(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_en(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn u64_en(b: &[u8], i: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[i..i + 8]);
    u64::from_le_bytes(v)
}

/// Donde vive la seccion de recursos DENTRO DEL FICHERO, si la hay.
///
/// Devuelve `(file_offset, file_size)`. Es lo que necesita alguien que quiera
/// leer un recurso **sin tener el paquete entero en memoria**: con esto y el
/// indice, un recurso es un `fseek` y un `fread`.
pub fn localizar_recursos(bex: &[u8]) -> Option<(bx_u64, bx_u64)> {
    let (tabla, count) = tabla_de(bex)?;
    for i in 0..count {
        let e = &bex[tabla + i * SectionEntry::SIZE..tabla + (i + 1) * SectionEntry::SIZE];
        if e[E_KIND] == SectionKind::Resources as u8 {
            let off = u64_en(e, E_FILE_OFFSET);
            let size = u64_en(e, E_FILE_SIZE);
            if (off as usize).checked_add(size as usize)? > bex.len() {
                return None;
            }
            return Some((off, size));
        }
    }
    None
}

/// Los bytes de la seccion de recursos, si el paquete entero esta a mano.
pub fn seccion_recursos(bex: &[u8]) -> Option<&[u8]> {
    let (off, size) = localizar_recursos(bex)?;
    bex.get(off as usize..(off + size) as usize)
}

/// El directorio ya validado, en un paso.
pub fn directorio(bex: &[u8]) -> Option<recursos::Directorio<'_>> {
    recursos::Directorio::nuevo(seccion_recursos(bex)?)
}

fn tabla_de(bex: &[u8]) -> Option<(usize, usize)> {
    if bex.len() < HEADER_LEN || u32_en(bex, H_MAGIC) != BEF_MAGIC {
        return None;
    }
    let tabla = u64_en(bex, H_TABLE_OFFSET) as usize;
    let count = u32_en(bex, H_SECTION_COUNT) as usize;
    let fin = tabla.checked_add(count.checked_mul(SectionEntry::SIZE)?)?;
    if fin > bex.len() {
        return None;
    }
    Some((tabla, count))
}

/// Anade (o **reemplaza**) la seccion de recursos de una imagen BEF.
///
/// Reemplaza en vez de acumular a proposito: empaquetar dos veces con listas
/// distintas tiene que dar el paquete de la segunda lista, no los dos indices
/// pegados. Un paquete con dos directorios es un paquete donde "cual gana"
/// depende de quien lo lea.
///
/// Una lista vacia **quita** la seccion, que es como se desempaqueta.
pub fn empaquetar(bex: &[u8], lista: &[(&str, &[u8])]) -> Result<Vec<u8>, &'static str> {
    let (tabla, count) = tabla_de(bex).ok_or("esto no es una imagen BEF")?;

    let mut b = BefBuilder::new();
    // El header se conserva entero salvo lo que `build` recalcula: arquitectura,
    // banderas, extensiones de CPU declaradas y version del ABI son del
    // programa, no de quien lo empaqueta. Sobrescribirlos con los valores por
    // defecto convertiria empaquetar en "recompilar con otras opciones".
    let mut cab = [0u8; HEADER_LEN];
    cab.copy_from_slice(&bex[..HEADER_LEN]);
    b.header = unsafe { core::ptr::read_unaligned(cab.as_ptr() as *const _) };
    b.entry_offset = u64_en(bex, H_ENTRY_OFFSET);

    for i in 0..count {
        let e = &bex[tabla + i * SectionEntry::SIZE..tabla + (i + 1) * SectionEntry::SIZE];
        let kind = SectionKind::from_u8(e[E_KIND]).ok_or("seccion de tipo desconocido")?;
        if kind == SectionKind::Resources {
            continue; // la vieja se tira: esta funcion reemplaza
        }
        // ** LA FIRMA VIEJA TAMBIEN SE TIRA, y hay que tirarla.
        //
        // Sus hashes describen la disposicion ANTERIOR. Al meter recursos, las
        // secciones se recolocan y sus offsets cambian: conservarla dejaria un
        // fichero que declara integridad y no la cumple -- que es peor que uno
        // sin firma, porque el segundo al menos no promete nada.
        //
        // `BefBuilder::build` la regenera al final, con la disposicion nueva y
        // ella la ultima de todas. Ver su cabecera.
        if kind == SectionKind::Signature {
            continue;
        }
        let off = u64_en(e, E_FILE_OFFSET) as usize;
        let size = u64_en(e, E_FILE_SIZE) as usize;
        let datos: Vec<u8> = if kind == SectionKind::Bss || size == 0 {
            Vec::new()
        } else {
            bex.get(off..off + size)
                .ok_or("una seccion apunta fuera del fichero")?
                .to_vec()
        };
        let mut s = BefSection::new(kind, datos);
        s.flags = SectionFlags::from_bits_truncate(u32_en(e, E_FLAGS));
        s.mem_size = u64_en(e, E_MEM_SIZE);
        s.alignment = u16_en(e, E_ALIGNMENT);
        b.add_section(s);
    }

    if !lista.is_empty() {
        let indice = recursos::construir(lista)?;
        let mut s = BefSection::new(SectionKind::Resources, indice);
        // Se lee, no se ejecuta y no se escribe. Y no la mapea nadie: el
        // cargador solo mapea Code/RoData/Data/Bss, asi que estos bytes no
        // ocupan ni una pagina del proceso.
        s.flags = SectionFlags::READ;
        s.alignment = 8;
        b.add_section(s);
    }

    b.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Una imagen minima como la que emite un frontend.
    fn imagen() -> Vec<u8> {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0x90; 32]));
        b.add_section(BefSection::rodata(b"hola\0".to_vec()));
        b.add_section(BefSection::bss(64));
        b.entry_offset = 0;
        b.build().unwrap()
    }

    #[test]
    fn el_paquete_lleva_los_datos_y_se_leen_por_nombre() {
        let wad = vec![0x5Au8; 500];
        let p = empaquetar(&imagen(), &[("doom1.wad", &wad)]).unwrap();

        let d = directorio(&p).expect("el paquete trae directorio");
        let i = d.buscar("doom1.wad").expect("esta");
        assert_eq!(d.datos(i).unwrap(), &wad[..]);
    }

    /// ** LO QUE HACE QUE ESTO SEA UTIL HOY: el paquete **sigue siendo un `.bex`
    /// que arranca**. El cargador mapea Code/RoData/Data/Bss y salta el resto,
    /// asi que los recursos no le cuestan ni una pagina al proceso.
    ///
    /// Se comprueba de la unica forma que vale: **el codigo y el rodata salen
    /// byte a byte iguales** que en la imagen sin empaquetar.
    #[test]
    fn empaquetar_no_toca_el_programa() {
        let base = imagen();
        let p = empaquetar(&base, &[("x", b"12345")]).unwrap();

        for kind in [SectionKind::Code, SectionKind::RoData] {
            let a = seccion(&base, kind).expect("estaba");
            let b = seccion(&p, kind).expect("sigue estando");
            assert_eq!(a, b, "la seccion {kind:?} cambio al empaquetar");
        }
        assert_eq!(
            u64_en(&base, H_ENTRY_OFFSET),
            u64_en(&p, H_ENTRY_OFFSET),
            "el punto de entrada se movio"
        );
    }

    /// ** Y LAS SECCIONES CONSERVAN SU ORDEN. Las relocations de BMO guardan el
    /// INDICE de la seccion destino, asi que insertar la nueva en medio haria
    /// que cada una apuntara a la de al lado: un programa que carga, arranca, y
    /// lee sus cadenas de otro sitio.
    ///
    /// ** Y desde el 2026-08-09 hay DOS reglas de orden, no una: los recursos
    /// van los ultimos **del contenido**, y la firma la ultima **de todas**.
    /// `BefBuilder::build` la regenera al final porque sus hashes describen la
    /// disposicion recien escrita -- conservar la vieja daria un fichero que
    /// declara integridad y no la cumple.
    #[test]
    fn la_seccion_nueva_va_la_ultima() {
        let p = empaquetar(&imagen(), &[("x", b"1")]).unwrap();
        let (tabla, count) = tabla_de(&p).unwrap();
        let tipos: Vec<u8> = (0..count)
            .map(|i| p[tabla + i * SectionEntry::SIZE + E_KIND])
            .collect();
        assert_eq!(
            tipos,
            vec![
                SectionKind::Code as u8,
                SectionKind::RoData as u8,
                SectionKind::Bss as u8,
                SectionKind::Resources as u8,
                // La firma, SIEMPRE la ultima. La pone `build`.
                SectionKind::Signature as u8,
            ]
        );
    }

    /// Empaquetar dos veces REEMPLAZA. Acumular dejaria dos directorios y "cual
    /// gana" dependeria de quien lo lea.
    #[test]
    fn empaquetar_dos_veces_no_acumula() {
        let p1 = empaquetar(&imagen(), &[("viejo", b"1")]).unwrap();
        let p2 = empaquetar(&p1, &[("nuevo", b"2")]).unwrap();
        let d = directorio(&p2).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d.nombre(0), Some("nuevo"));
    }

    /// Y una lista vacia lo DESEMPAQUETA, dejando la imagen como estaba.
    #[test]
    fn una_lista_vacia_desempaqueta() {
        let base = imagen();
        let p = empaquetar(&base, &[("x", b"1")]).unwrap();
        let vuelta = empaquetar(&p, &[]).unwrap();
        assert!(directorio(&vuelta).is_none());
        assert_eq!(vuelta, base, "desempaquetar tiene que devolver la imagen original");
    }

    /// `localizar_recursos` da offset y tamano EN EL FICHERO -- que es lo que
    /// necesita quien va a hacer `fseek`+`fread` sin cargar el paquete entero.
    #[test]
    fn se_puede_localizar_sin_leer_el_paquete() {
        let wad = vec![0x77u8; 200];
        let p = empaquetar(&imagen(), &[("w", &wad)]).unwrap();
        let (off, size) = localizar_recursos(&p).unwrap();
        assert!(off > 0 && size as usize >= wad.len());
        // Y los bytes que hay ahi son un directorio valido.
        let d = recursos::Directorio::nuevo(&p[off as usize..(off + size) as usize]).unwrap();
        assert_eq!(d.datos(d.buscar("w").unwrap()).unwrap(), &wad[..]);
    }

    /// Un `.bex` de hoy no tiene recursos, y eso no es un fallo: se contesta
    /// "no hay".
    #[test]
    fn una_imagen_sin_recursos_lo_dice() {
        assert!(localizar_recursos(&imagen()).is_none());
        assert!(directorio(&imagen()).is_none());
    }

    #[test]
    fn lo_que_no_es_bef_se_rechaza() {
        assert!(empaquetar(b"no soy un bex", &[]).is_err());
        assert!(localizar_recursos(b"no soy un bex").is_none());
    }

    fn seccion(bex: &[u8], kind: SectionKind) -> Option<&[u8]> {
        let (tabla, count) = tabla_de(bex)?;
        for i in 0..count {
            let e = &bex[tabla + i * SectionEntry::SIZE..tabla + (i + 1) * SectionEntry::SIZE];
            if e[E_KIND] == kind as u8 {
                let off = u64_en(e, E_FILE_OFFSET) as usize;
                let size = u64_en(e, E_FILE_SIZE) as usize;
                return bex.get(off..off + size);
            }
        }
        None
    }
}
