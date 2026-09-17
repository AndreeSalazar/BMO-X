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
//! * **Ya TIRA lo que no se usa** (E5b, `tirar.rs`): se marca desde `main` y
//!   lo que no alcanza nadie no se copia. Se nota en el tamano y no en el
//!   comportamiento, que es como se sabe que esta bien hecho.
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

mod tirar;

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
    /// Un trozo que se conserva apunta a uno que se tiro. Es un bug de la poda
    /// (E5b) y no un fallo de quien enlaza: por eso lo dice asi.
    PodaIncoherente { nombre: String, unidad: String },
    /// Un puntero GUARDADO EN UN DATO que apunta al `bss`. El objeto sabe
    /// nombrar tres secciones --codigo, datos, rodata-- y `bss` no es una de
    /// ellas, asi que esa direccion no se puede expresar todavia.
    BssNoSeSabeNombrar { unidad: String, nombre: String },
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
            Fallo::PodaIncoherente { nombre, unidad } => write!(
                f,
                "la poda tiro algo que {unidad} sigue usando ('{nombre}'): esto es un bug del enlazador"
            ),
            Fallo::BssNoSeSabeNombrar { unidad, nombre } => write!(
                f,
                "{unidad}: '{nombre}' guarda en un dato la direccion de algo del bss, y el objeto solo sabe nombrar codigo, datos y rodata"
            ),
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
pub(crate) struct Definicion {
    pub unidad: usize,
    pub seccion: SectionKind,
    pub offset: u64,
}

fn a_pagina(n: u64) -> u64 {
    n.div_ceil(PAGINA) * PAGINA
}

fn alinear(v: &mut Vec<u8>, a: usize) {
    while v.len() % a != 0 {
        v.push(0);
    }
}

/// **Lo que salio de enlazar**, con lo que se tiro dicho por su nombre.
pub struct Informe {
    pub bytes: Vec<u8>,
    /// Las funciones que nadie llamaba, en el orden en que estaban.
    pub tiradas: Vec<String>,
    pub bytes_tirados: u64,
    /// Unidades que no se pudieron podar (su codigo no lo cubren sus
    /// funciones). Se conservan enteras, y se dicen.
    pub sin_podar: Vec<String>,
}

/// **Junta los objetos en un ejecutable.** `unidades` es `(nombre, bytes)`, y
/// el nombre solo se usa para poder decir en cual esta el problema.
pub fn enlazar(unidades: &[(String, Vec<u8>)]) -> Result<Vec<u8>, Fallo> {
    Ok(enlazar_informado(unidades, true)?.bytes)
}

/// Lo mismo, diciendo que se tiro. Con `poda` en `false` no se tira nada: es el
/// MISMO enlace sin E5b, y existe para poder medir -- un limite es tan bueno
/// como el numero con el que se compara.
pub fn enlazar_informado(unidades: &[(String, Vec<u8>)], poda: bool) -> Result<Informe, Fallo> {
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

    // -- 2. Resolver. Un nombre publico, una definicion. --
    //
    // Va ANTES de colocar porque la poda lo necesita: para saber quien llama a
    // quien hay que poder seguir un nombre hasta la unidad que lo define.
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

    // -- 3. TIRAR lo que nadie llama (E5b). Ver `tirar.rs`. --
    let poda = tirar::podar(&objs, unidades, &publicos, poda)?;

    // -- 4. Colocar. El orden es el que se dio: mismo mandato, mismos bytes. --
    let (mut code, mut rodata, mut data) = (Vec::new(), Vec::new(), Vec::new());
    let mut bss = 0u64;
    let mut sitio: Vec<Sitio> = Vec::with_capacity(objs.len());
    for (i, o) in objs.iter().enumerate() {
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
        code.extend_from_slice(&poda.codigo[i]);
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
    let codigo_de = |k: SectionKind| match k {
        SectionKind::Code => Some(REL_CODE),
        SectionKind::Data => Some(REL_DATA),
        SectionKind::RoData => Some(REL_RODATA),
        // El objeto sabe nombrar TRES secciones, y `bss` no es una de ellas.
        _ => None,
    };

    // -- 5. Parchear. --
    let mut salida: Vec<Relocation> = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        for r in &o.relocs {
            let patch_sec = match r.target_section {
                REL_CODE => SectionKind::Code,
                REL_DATA => SectionKind::Data,
                _ => SectionKind::RoData,
            };
            // Si el sitio que habia que parchear se fue con la poda, la reloc
            // se va con el: ya no hay bytes que escribir.
            let en_unidad = if patch_sec == SectionKind::Code {
                match poda.nuevo(i, r.offset) {
                    Some(x) => x,
                    None => continue,
                }
            } else {
                r.offset
            };
            let en = sitio_de(&sitio[i], patch_sec) + en_unidad;

            // A donde apunta. UNA sola cuenta, en `tirar.rs`: la poda usa la
            // misma para saber quien llama a quien.
            let b = tirar::blanco_de(&objs, unidades, &publicos, i, r)?;
            let nombre = || match o.symbols.get(r.symbol_idx as usize) {
                Some(s) if r.kind != RelocationKind::SeccionAbs64 as u8 => s.name.to_string(),
                _ => String::from("(una seccion)"),
            };
            let destino_en_unidad = if b.seccion == SectionKind::Code {
                // Un trozo vivo no puede apuntar a uno tirado: si pasa, la poda
                // se equivoco, y eso se dice -- no se enlaza igual.
                match poda.nuevo(b.unidad, b.offset) {
                    Some(x) => x,
                    None => {
                        return Err(Fallo::PodaIncoherente {
                            nombre: nombre(),
                            unidad: unidades[b.unidad].0.clone(),
                        })
                    }
                }
            } else {
                b.offset
            };
            let destino = sitio_de(&sitio[b.unidad], b.seccion) + destino_en_unidad;

            let at = en as usize;
            let buffer = match patch_sec {
                SectionKind::Code => &mut code,
                SectionKind::Data => &mut data,
                _ => &mut rodata,
            };
            if r.kind == RelocationKind::Rel32 as u8 {
                // Una DISTANCIA dentro de la imagen: se sabe aqui. El `rip`
                // apunta detras del hueco de 4 bytes, de ahi el -4.
                let p = (va_de(patch_sec) + en) as i64;
                let s = (va_de(b.seccion) + destino) as i64;
                let Ok(disp) = i32::try_from(s - p - 4) else {
                    return Err(Fallo::DemasiadoLejos {
                        unidad: unidades[i].0.clone(),
                        nombre: nombre(),
                    });
                };
                buffer[at..at + 4].copy_from_slice(&disp.to_le_bytes());
            } else {
                // Una DIRECCION: depende de donde cargue el programa, y eso lo
                // sabe el cargador. Se reescribe contra la seccion ya junta.
                let (Some(hacia), Some(desde)) = (codigo_de(b.seccion), codigo_de(patch_sec))
                else {
                    return Err(Fallo::BssNoSeSabeNombrar {
                        unidad: unidades[i].0.clone(),
                        nombre: nombre(),
                    });
                };
                buffer[at..at + 8].copy_from_slice(&0u64.to_le_bytes());
                salida.push(Relocation {
                    offset: en,
                    symbol_idx: hacia as u32,
                    kind: RelocationKind::SeccionAbs64 as u8,
                    target_section: desde,
                    _pad: [0; 2],
                    addend: destino as i64,
                });
            }
        }
    }

    // -- El punto de entrada. --
    let Some(principal) = publicos.get("main").copied() else {
        return Err(Fallo::SinMain);
    };
    let Some(entry_en_unidad) = poda.nuevo(principal.unidad, principal.offset) else {
        return Err(Fallo::PodaIncoherente {
            nombre: String::from("main"),
            unidad: unidades[principal.unidad].0.clone(),
        });
    };
    let entry = sitio_de(&sitio[principal.unidad], SectionKind::Code) + entry_en_unidad;

    // -- 6. Escribir y verificar. --
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
            // Una funcion que se tiro no deja nombre: el simbolo apuntaria a
            // los bytes de otra, y una autopsia acusaria a quien no fue.
            let Some(off) = poda.nuevo(i, s.offset) else { continue };
            let name_off = cadenas.len() as u32;
            cadenas.extend_from_slice(s.name.as_bytes());
            cadenas.push(0);
            entradas.push(Symbol {
                name_off,
                name_hash: name_hash(s.name),
                virt_addr: sitio[i].code + off,
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
    Ok(Informe {
        bytes,
        tiradas: poda.tiradas,
        bytes_tirados: poda.bytes,
        sin_podar: poda.sin_podar,
    })
}

#[cfg(test)]
mod pruebas;
