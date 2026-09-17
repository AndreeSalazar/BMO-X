//! **EL ENLAZADOR ESTATICO** -- N objetos (`.bo`) y un `.bex` que corre.
//!
//! E3 de `docs/plan/PLAN_EL_ENLAZADOR.md`. La decision del dueno (E0) fue
//! ESTATICO: todo lo que un programa ejecuta viaja dentro de su `.bex`, asi que
//! su firma lo cubre entero y corre igual en cualquier BMO-X.
//!
//! ## Lo que hace, en orden
//!
//! ```text
//!   1. LEER      cada objeto con su contrato (`bmo_abi::bef::objeto`)
//!   2. COLOCAR   las secciones de todas las unidades, una detras de otra
//!   3. RESOLVER  cada nombre: definido aqui, o definido por otra unidad
//!   4. PARCHEAR  lo que ya se sabe; dejar para el CARGADOR lo que depende
//!                de donde se cargue el programa
//!   5. VERIFICAR con `bmo-verify`, y solo entonces entregar los bytes
//! ```
//!
//! ## La linea que divide el trabajo con el cargador
//!
//! Un `rel32` es una DISTANCIA entre dos sitios de la misma imagen: se conoce
//! aqui y se escribe aqui. Un puntero de 64 bits guardado en un dato es una
//! DIRECCION, y esa depende de donde cargue el programa -- eso se reescribe
//! como `SeccionAbs64` y lo cierra el cargador, que es quien lo sabe. Es la
//! misma division que ya usaba BMO C consigo mismo.
//!
//! ## Lo que NO hace, dicho para que no crezca solo
//!
//! * **No tira lo que no se usa.** Lo que entra, sale. Es E5b del plan, y el
//!   dia que se haga se nota en el tamano y no en el comportamiento.
//! * **No admite `Weak`.** El contrato lo rechaza: una promesa menos.
//! * **No ordena por optimizacion.** Las unidades salen en el orden en que se
//!   dan, para que el mismo mandato produzca los mismos bytes siempre.

use std::collections::HashMap;

use bmo_abi::bef::header::BefFlags;
use bmo_abi::bef::objeto::{self, Object, REL_CODE, REL_DATA, REL_RODATA};
use bmo_abi::bef::relocations::{Relocation, RelocationKind};
use bmo_abi::bef::sections::SectionKind;
use bmo_abi::bef::symbols::{name_hash, Symbol, SymbolBinding, SymbolKind, SymbolVisibility};
use bmo_abi::bef::writer::{BefBuilder, BefSection};

/// El tamano de pagina con el que el cargador coloca cada seccion
/// (`ring0/task/proc.rs`: `va_cursor = va_start + pages * PAGE`).
const PAGINA: u64 = 4096;

/// Por que no se pudo enlazar. Cada una dice QUE arreglar y DONDE.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fallo {
    /// Sin unidades no hay programa.
    NadaQueEnlazar,
    /// `unidad` no es un objeto valido.
    NoEsObjeto { unidad: String, motivo: String },
    /// El mismo nombre publico, definido en dos unidades.
    DefinidoDosVeces { nombre: String, en: String, y_en: String },
    /// Alguien lo usa y nadie lo define.
    NadieLoDefine { nombre: String, usado_en: String },
    /// Un `rel32` cuyo destino queda a mas de 2 GiB: el programa es demasiado
    /// grande para una llamada directa.
    DemasiadoLejos { unidad: String, nombre: String },
    /// No hay `main` publico: eso es una biblioteca, no un programa.
    SinMain,
    /// Lo enlazado no pasa el gate. Es un bug del enlazador, y se dice asi.
    NoPasaElGate(Vec<String>),
}

impl core::fmt::Display for Fallo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Fallo::NadaQueEnlazar => write!(f, "no se dio ningun objeto"),
            Fallo::NoEsObjeto { unidad, motivo } => write!(f, "{unidad}: no es un objeto valido ({motivo})"),
            Fallo::DefinidoDosVeces { nombre, en, y_en } => write!(
                f,
                "'{nombre}' esta definido en {en} y en {y_en}. Si viene de una cabecera del sistema, \
                 eso es E2b del plan del enlazador; si no, sobra una de las dos o le falta `static`"
            ),
            Fallo::NadieLoDefine { nombre, usado_en } => {
                write!(f, "'{nombre}' lo usa {usado_en} y no lo define nadie")
            }
            Fallo::DemasiadoLejos { unidad, nombre } => {
                write!(f, "{unidad}: '{nombre}' queda a mas de 2 GiB de quien lo llama")
            }
            Fallo::SinMain => write!(f, "ninguna unidad define 'main': esto es una biblioteca, no un programa"),
            Fallo::NoPasaElGate(r) => write!(f, "lo enlazado no pasa el gate: {}", r.join("; ")),
        }
    }
}

/// Donde acabo una seccion de una unidad dentro de la seccion junta.
#[derive(Clone, Copy, Default)]
struct Sitio {
    code: u64,
    rodata: u64,
    data: u64,
    bss: u64,
}

/// Un nombre publico, y donde vive.
#[derive(Clone, Copy)]
struct Definicion {
    unidad: usize,
    seccion: SectionKind,
    offset: u64,
}

fn a_pagina(n: u64) -> u64 {
    n.div_ceil(PAGINA) * PAGINA
}

fn alinear(v: &mut Vec<u8>, a: usize) {
    while v.len() % a != 0 {
        v.push(0);
    }
}

/// **Junta los objetos en un ejecutable.** `unidades` es `(nombre, bytes)`, y
/// el nombre solo se usa para poder decir en cual esta el problema.
pub fn enlazar(unidades: &[(String, Vec<u8>)]) -> Result<Vec<u8>, Fallo> {
    if unidades.is_empty() {
        return Err(Fallo::NadaQueEnlazar);
    }
    let mut objs: Vec<Object<'_>> = Vec::with_capacity(unidades.len());
    for (nombre, bytes) in unidades {
        match objeto::read(bytes) {
            Ok(o) => objs.push(o),
            Err(e) => {
                return Err(Fallo::NoEsObjeto { unidad: nombre.clone(), motivo: format!("{e:?}") })
            }
        }
    }

    // -- 2. Colocar. El orden es el que se dio: mismo mandato, mismos bytes. --
    let (mut code, mut rodata, mut data) = (Vec::new(), Vec::new(), Vec::new());
    let mut bss = 0u64;
    let mut sitio: Vec<Sitio> = Vec::with_capacity(objs.len());
    for o in &objs {
        // El codigo, a 16: una funcion que empieza en frontera es lo que espera
        // cualquier CPU moderno para no partir una linea de cache en la entrada.
        alinear(&mut code, 16);
        alinear(&mut rodata, 8);
        alinear(&mut data, 8);
        bss = bss.div_ceil(8) * 8;
        sitio.push(Sitio {
            code: code.len() as u64,
            rodata: rodata.len() as u64,
            data: data.len() as u64,
            bss,
        });
        code.extend_from_slice(o.code);
        rodata.extend_from_slice(o.rodata);
        data.extend_from_slice(o.data);
        bss += o.bss;
    }

    // Las direcciones virtuales, con la regla del cargador: cada seccion
    // empieza en la pagina siguiente a las que ocupa la anterior.
    let va_code = 0u64;
    let va_rodata = a_pagina(code.len() as u64);
    let va_data = va_rodata + a_pagina(rodata.len() as u64);
    let va_bss = va_data + a_pagina(data.len() as u64);
    let va_de = |k: SectionKind| match k {
        SectionKind::Code => va_code,
        SectionKind::RoData => va_rodata,
        SectionKind::Data => va_data,
        _ => va_bss,
    };
    let sitio_de = |s: &Sitio, k: SectionKind| match k {
        SectionKind::Code => s.code,
        SectionKind::RoData => s.rodata,
        SectionKind::Data => s.data,
        _ => s.bss,
    };

    // -- 3. Resolver. Un nombre publico, una definicion. --
    let mut publicos: HashMap<&str, Definicion> = HashMap::new();
    for (i, o) in objs.iter().enumerate() {
        for s in &o.symbols {
            let (Some(seccion), true) = (s.section, s.global) else { continue };
            if s.seccion_ancla {
                continue;
            }
            if let Some(ya) = publicos.get(s.name) {
                return Err(Fallo::DefinidoDosVeces {
                    nombre: s.name.to_string(),
                    en: unidades[ya.unidad].0.clone(),
                    y_en: unidades[i].0.clone(),
                });
            }
            publicos.insert(s.name, Definicion { unidad: i, seccion, offset: s.offset });
        }
    }

    // -- 4. Parchear. --
    let mut salida: Vec<Relocation> = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        for r in &o.relocs {
            let patch_sec = match r.target_section {
                REL_CODE => SectionKind::Code,
                REL_DATA => SectionKind::Data,
                _ => SectionKind::RoData,
            };
            let en = sitio_de(&sitio[i], patch_sec) + r.offset;
            let buffer = match patch_sec {
                SectionKind::Code => &mut code,
                SectionKind::Data => &mut data,
                _ => &mut rodata,
            };

            // Donde apunta: una seccion de esta unidad, o un nombre. El
            // ADDEND se lleva aparte hasta el final -- sumarlo aqui desborda
            // cuando el destino es el principio de una seccion y el addend es
            // el `-4` de un `lea` (paso el 2026-09-17, y la resta iba a un u64).
            let (destino_sec, destino_off) = if r.kind == RelocationKind::SeccionAbs64 as u8 {
                let k = match r.symbol_idx as u8 {
                    REL_CODE => SectionKind::Code,
                    REL_DATA => SectionKind::Data,
                    _ => SectionKind::RoData,
                };
                (k, sitio_de(&sitio[i], k))
            } else {
                let s = &o.symbols[r.symbol_idx as usize];
                match s.section {
                    // Definido en esta unidad (o un ancla de seccion suya).
                    Some(k) => (k, sitio_de(&sitio[i], k) + s.offset),
                    // De otra unidad.
                    None => {
                        let Some(d) = publicos.get(s.name) else {
                            return Err(Fallo::NadieLoDefine {
                                nombre: s.name.to_string(),
                                usado_en: unidades[i].0.clone(),
                            });
                        };
                        (d.seccion, sitio_de(&sitio[d.unidad], d.seccion) + d.offset)
                    }
                }
            };

            if r.kind == RelocationKind::Rel32 as u8 {
                // Una DISTANCIA dentro de la imagen: se sabe aqui.
                let p = va_de(patch_sec) + en;
                let s = va_de(destino_sec) + destino_off;
                let disp = s as i64 + r.addend - p as i64;
                let Ok(disp) = i32::try_from(disp) else {
                    let nombre = if r.kind == RelocationKind::SeccionAbs64 as u8 {
                        String::from("(una seccion)")
                    } else {
                        o.symbols[r.symbol_idx as usize].name.to_string()
                    };
                    return Err(Fallo::DemasiadoLejos { unidad: unidades[i].0.clone(), nombre });
                };
                let at = en as usize;
                buffer[at..at + 4].copy_from_slice(&disp.to_le_bytes());
            } else {
                // Una DIRECCION: depende de donde cargue el programa, y eso lo
                // sabe el cargador. Se reescribe contra la seccion ya junta.
                let codigo_de = |k: SectionKind| match k {
                    SectionKind::Code => REL_CODE,
                    SectionKind::Data => REL_DATA,
                    _ => REL_RODATA,
                };
                let at = en as usize;
                buffer[at..at + 8].copy_from_slice(&0u64.to_le_bytes());
                salida.push(Relocation {
                    offset: en,
                    symbol_idx: codigo_de(destino_sec) as u32,
                    kind: RelocationKind::SeccionAbs64 as u8,
                    target_section: codigo_de(patch_sec),
                    _pad: [0; 2],
                    addend: destino_off as i64 + r.addend,
                });
            }
        }
    }

    // -- El punto de entrada. --
    let Some(principal) = publicos.get("main").copied() else {
        return Err(Fallo::SinMain);
    };
    let entry = sitio_de(&sitio[principal.unidad], SectionKind::Code) + principal.offset;

    // -- 5. Escribir y verificar. --
    let mut b = BefBuilder::new();
    let quiere_pantalla = unidades.iter().any(|(_, bytes)| {
        let f = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        BefFlags::from_bits_truncate(f).contains(BefFlags::WANTS_SCREEN)
    });
    if quiere_pantalla {
        b.header.flags |= BefFlags::WANTS_SCREEN.bits();
    }
    b.entry_offset = entry;

    let mut code_sec = BefSection::code(code);
    code_sec.alignment = PAGINA as u16;
    b.add_section(code_sec);
    if !rodata.is_empty() {
        let mut s = BefSection::rodata(rodata);
        s.alignment = PAGINA as u16;
        b.add_section(s);
    }
    if !data.is_empty() {
        let mut s = BefSection::data(data);
        s.alignment = PAGINA as u16;
        b.add_section(s);
    }
    if bss > 0 {
        let mut s = BefSection::bss(bss);
        s.alignment = PAGINA as u16;
        b.add_section(s);
    }

    // Los simbolos de FUNCION, ya con su sitio definitivo: es lo que convierte
    // un `rip` de una autopsia en un nombre. No es opcional por eso.
    let mut entradas = Vec::new();
    let mut cadenas: Vec<u8> = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        for s in &o.symbols {
            if !s.function || s.section != Some(SectionKind::Code) {
                continue;
            }
            let name_off = cadenas.len() as u32;
            cadenas.extend_from_slice(s.name.as_bytes());
            cadenas.push(0);
            entradas.push(Symbol {
                name_off,
                name_hash: name_hash(s.name),
                virt_addr: sitio[i].code + s.offset,
                size: s.size,
                kind: SymbolKind::Function as u8,
                binding: if s.global { SymbolBinding::Global } else { SymbolBinding::Local } as u8,
                visibility: SymbolVisibility::Default as u8,
                section_idx: 0,
                _reserved: 0,
            });
        }
    }
    if !entradas.is_empty() {
        b.add_section(BefSection::symbols(entradas, cadenas));
    }
    if !salida.is_empty() {
        b.add_section(BefSection::relocs(salida));
    }

    let bytes = b.build().unwrap_or_default();
    if let bmo_verify::Verdict::Rejected(razones) = bmo_verify::verify(&bytes) {
        return Err(Fallo::NoPasaElGate(razones));
    }
    Ok(bytes)
}

#[cfg(test)]
mod pruebas;
