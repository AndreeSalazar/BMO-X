//! **TIRAR LO QUE NADIE LLAMA** -- E5b de `docs/plan/PLAN_EL_ENLAZADOR.md`.
//!
//! Sin esto, enlazar contra una biblioteca es un mal negocio, y el numero lo
//! decia: una unidad que solo usa `strncpy` bajaba de 5.242 a 1.698 bytes, pero
//! el programa subia de 5.100 a 28.197 porque la libc entraba ENTERA. El ahorro
//! no estaba en compilar la libc aparte: estaba en no llevarse lo que nadie
//! llama.
//!
//! ## No hace falta inventar formato
//!
//! Todo lo que se necesita ya viaja dentro del objeto:
//!
//! ```text
//!    cada funcion  ->  su simbolo con [offset, size)   =  los TROZOS
//!    cada reloc    ->  quien apunta a quien            =  las ARISTAS
//! ```
//!
//! Se marca desde `main`, se conserva lo alcanzable, y lo demas no se copia.
//!
//! ## Lo que NO se tira, dicho para que nadie lo cuente como hecho
//!
//! * **`rodata` y `data` salen enteros.** Un global con nombre si tiene simbolo,
//!   pero las cadenas son anonimas y se referencian contra el ancla de la
//!   seccion: no hay quien diga donde acaba una. Tirar ahi pide otra casilla y
//!   otra prueba.
//! * **Una unidad cuyo codigo no este ENTERO cubierto por funciones no se
//!   poda.** No se adivina: se conserva entera y se dice cual (`sin_podar`).

use std::collections::HashMap;

use bmo_abi::bef::objeto::{Object, REL_CODE, REL_DATA};
use bmo_abi::bef::relocations::{Relocation, RelocationKind};
use bmo_abi::bef::sections::SectionKind;

use crate::{Definicion, Fallo};

/// A que byte apunta una reloc, ANTES de juntar las unidades.
///
/// Existe para que haya UN solo sitio que conteste esa pregunta: la poda la
/// necesita para saber quien llama a quien, y el parcheo para escribir el
/// numero. Cuando eran dos cuentas parecidas en dos sitios, una de las dos
/// desbordaba -- el `-4` de un `lea` sumado a un `u64`, el 2026-09-17.
pub(crate) struct Blanco {
    pub unidad: usize,
    pub seccion: SectionKind,
    /// El byte exacto dentro de esa seccion de esa unidad, con el addend ya
    /// contado: el `-4` que un `rel32` lleva porque el `rip` apunta detras del
    /// hueco se suma aqui UNA vez y no vuelve a aparecer.
    pub offset: u64,
}

fn seccion_de(codigo: u8) -> SectionKind {
    match codigo {
        REL_CODE => SectionKind::Code,
        REL_DATA => SectionKind::Data,
        _ => SectionKind::RoData,
    }
}

pub(crate) fn blanco_de<'a>(
    objs: &[Object<'a>],
    unidades: &[(String, Vec<u8>)],
    publicos: &HashMap<&'a str, Definicion>,
    i: usize,
    r: &Relocation,
) -> Result<Blanco, Fallo> {
    let ajuste = if r.kind == RelocationKind::Rel32 as u8 { 4 } else { 0 };
    if r.kind == RelocationKind::SeccionAbs64 as u8 {
        return Ok(Blanco {
            unidad: i,
            seccion: seccion_de(r.symbol_idx as u8),
            offset: (r.addend + ajuste).max(0) as u64,
        });
    }
    let Some(s) = objs[i].symbols.get(r.symbol_idx as usize) else {
        return Err(Fallo::NoEsObjeto {
            unidad: unidades[i].0.clone(),
            motivo: format!("una reloc apunta al simbolo {} y no hay tantos", r.symbol_idx),
        });
    };
    let (unidad, seccion, base) = match s.section {
        Some(k) => (i, k, s.offset),
        None => {
            let Some(d) = publicos.get(s.name) else {
                return Err(Fallo::NadieLoDefine {
                    nombre: s.name.to_string(),
                    usado_en: unidades[i].0.clone(),
                });
            };
            (d.unidad, d.seccion, d.offset)
        }
    };
    Ok(Blanco { unidad, seccion, offset: (base as i64 + r.addend + ajuste).max(0) as u64 })
}

/// Un trozo de codigo con nombre: lo que se conserva o se tira entero.
struct Trozo {
    ini: u64,
    fin: u64,
    nombre: String,
}

/// El resultado de la poda: el codigo que se queda, y donde quedo cada byte.
pub(crate) struct Poda {
    /// Lo que sobrevive de la seccion de codigo de cada unidad.
    pub codigo: Vec<Vec<u8>>,
    /// Por unidad, los trozos conservados: `(ini_viejo, fin_viejo, ini_nuevo)`.
    conservados: Vec<Vec<(u64, u64, u64)>>,
    /// Los nombres que se fueron. Es lo que imprime la herramienta: un numero
    /// sin nombres no se puede comprobar.
    pub tiradas: Vec<String>,
    pub bytes: u64,
    /// Unidades que no se pudieron podar. No se calla: se dicen.
    pub sin_podar: Vec<String>,
}

impl Poda {
    /// Donde quedo un offset viejo del codigo, o `None` si se tiro.
    pub fn nuevo(&self, unidad: usize, off: u64) -> Option<u64> {
        self.conservados[unidad]
            .iter()
            .find(|(a, b, _)| off >= *a && off < *b)
            .map(|(a, _, n)| n + (off - a))
    }
}

/// Los trozos de una unidad, y si cubren su codigo ENTERO.
///
/// Que lo cubran no es un detalle: si sobra un byte que no pertenece a ninguna
/// funcion, mover las funciones lo dejaria huerfano o lo duplicaria. Por eso se
/// COMPRUEBA en vez de suponerse.
fn trozos_de(o: &Object<'_>) -> (Vec<Trozo>, bool) {
    let mut t: Vec<Trozo> = o
        .symbols
        .iter()
        .filter(|s| s.function && s.section == Some(SectionKind::Code) && !s.seccion_ancla)
        .map(|s| Trozo { ini: s.offset, fin: s.offset + s.size, nombre: s.name.to_string() })
        .collect();
    t.sort_by_key(|x| x.ini);
    let largo = o.code.len() as u64;
    let mut cubre = true;
    let mut cursor = 0u64;
    for x in &t {
        if x.ini != cursor {
            cubre = false;
        }
        cursor = x.fin;
    }
    if cursor != largo {
        cubre = false;
    }
    (t, cubre)
}

/// **Marca desde `main` y conserva lo alcanzable.**
///
/// `activa` en `false` enlaza sin tirar nada: es lo que permite MEDIR la poda
/// con el mismo mandato, y un limite es tan bueno como el numero con el que se
/// compara.
pub(crate) fn podar<'a>(
    objs: &[Object<'a>],
    unidades: &[(String, Vec<u8>)],
    publicos: &HashMap<&'a str, Definicion>,
    activa: bool,
) -> Result<Poda, Fallo> {
    let mut trozos: Vec<Vec<Trozo>> = Vec::with_capacity(objs.len());
    let mut entera: Vec<bool> = Vec::with_capacity(objs.len());
    let mut sin_podar = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        let (t, cubre) = trozos_de(o);
        if activa && cubre {
            trozos.push(t);
            entera.push(false);
        } else {
            if activa {
                sin_podar.push(unidades[i].0.clone());
            }
            trozos.push(vec![Trozo {
                ini: 0,
                fin: o.code.len() as u64,
                nombre: format!("{} (entera)", unidades[i].0),
            }]);
            entera.push(true);
        }
    }

    let cual = |u: usize, off: u64| -> Option<usize> {
        trozos[u].iter().position(|t| off >= t.ini && off < t.fin)
    };

    // -- Las raices. --
    let mut vivos: Vec<Vec<bool>> = trozos.iter().map(|t| vec![false; t.len()]).collect();
    let mut pila: Vec<(usize, usize)> = Vec::new();
    // Una unidad que no se poda va entera.
    for (u, e) in entera.iter().enumerate() {
        if *e && !trozos[u].is_empty() && !vivos[u][0] {
            vivos[u][0] = true;
            pila.push((u, 0));
        }
    }
    // `main`, que es por donde el programa empieza a correr.
    if let Some(d) = publicos.get("main") {
        if d.seccion == SectionKind::Code {
            if let Some(p) = cual(d.unidad, d.offset) {
                if !vivos[d.unidad][p] {
                    vivos[d.unidad][p] = true;
                    pila.push((d.unidad, p));
                }
            }
        }
    }
    // Y toda direccion de codigo GUARDADA EN UN DATO: a una tabla de punteros a
    // funcion no la llama nadie con un `call`, y sin esta raiz se caeria justo
    // lo que se invoca por puntero.
    for (i, o) in objs.iter().enumerate() {
        for r in &o.relocs {
            if r.target_section == REL_CODE {
                continue;
            }
            let b = blanco_de(objs, unidades, publicos, i, r)?;
            if b.seccion != SectionKind::Code {
                continue;
            }
            if let Some(p) = cual(b.unidad, b.offset) {
                if !vivos[b.unidad][p] {
                    vivos[b.unidad][p] = true;
                    pila.push((b.unidad, p));
                }
            }
        }
    }

    // -- Las aristas: quien llama a quien. --
    let mut llama: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    for (i, o) in objs.iter().enumerate() {
        for r in &o.relocs {
            if r.target_section != REL_CODE {
                continue;
            }
            let Some(desde) = cual(i, r.offset) else { continue };
            let b = blanco_de(objs, unidades, publicos, i, r)?;
            if b.seccion != SectionKind::Code {
                continue;
            }
            if let Some(hasta) = cual(b.unidad, b.offset) {
                llama.entry((i, desde)).or_default().push((b.unidad, hasta));
            }
        }
    }

    while let Some(n) = pila.pop() {
        let Some(hijos) = llama.get(&n) else { continue };
        for (u, p) in hijos.clone() {
            if !vivos[u][p] {
                vivos[u][p] = true;
                pila.push((u, p));
            }
        }
    }

    // -- Copiar lo que vive. --
    let mut poda = Poda {
        codigo: Vec::with_capacity(objs.len()),
        conservados: Vec::with_capacity(objs.len()),
        tiradas: Vec::new(),
        bytes: 0,
        sin_podar,
    };
    for (i, o) in objs.iter().enumerate() {
        let mut bytes = Vec::new();
        let mut mapa = Vec::new();
        for (p, t) in trozos[i].iter().enumerate() {
            if !vivos[i][p] {
                poda.tiradas.push(t.nombre.clone());
                poda.bytes += t.fin - t.ini;
                continue;
            }
            mapa.push((t.ini, t.fin, bytes.len() as u64));
            bytes.extend_from_slice(&o.code[t.ini as usize..t.fin as usize]);
        }
        poda.codigo.push(bytes);
        poda.conservados.push(mapa);
    }
    Ok(poda)
}
