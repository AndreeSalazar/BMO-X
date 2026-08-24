//! **LA FORMA DE UNA `tabla de A a B` EN MEMORIA.**
//!
//! La tercera instancia de `dynobj::header`, y la que trae la primera decision
//! que no es de disposicion sino de ALGORITMO.
//!
//! ## La disposicion
//!
//! ```text
//!   cabecera (32 B, la misma que `lista`)
//!     0..8    refs         bit 63 = INMORTAL
//!     8..12   type_index   indice, nunca puntero
//!     12..16  flags
//!     16..24  cuantos      PAREJAS ocupadas
//!     24..32  capacidad    RANURAS que hay -- ver abajo, no es lo mismo
//!
//!   las ranuras, desde el byte 32, de 24 bytes cada una:
//!     +0      marca        0 = vacia. Si no, el hash de la clave
//!     +8      clave        una referencia al objeto de la clave
//!     +16     valor        el valor, o una referencia a el
//! ```
//!
//! ## *** `cuantos` Y `capacidad` NO SIGNIFICAN LO MISMO QUE EN UNA LISTA
//!
//! En `lista`, `capacidad` es *"cuantos caben antes de crecer"* y llenarla del
//! todo es correcto. Aqui **no**: una tabla de direccionamiento abierto que se
//! llena entera **no termina de buscar**. La sonda da vueltas para siempre.
//!
//! ** Por eso `revisar` exige `cuantos < capacidad`, con UNA ranura de margen
//! como minimo. No es una precaucion de estilo: es la condicion que hace que el
//! bucle de busqueda tenga final.
//!
//! ## Y la marca no es un booleano: es el HASH
//!
//! Guardar *"esta ranura esta usada"* costaria lo mismo --ocho bytes-- y daria
//! menos. Con el hash guardado, una sonda que no coincide **se descarta sin
//! mirar la clave**, que es lo caro: comparar dos textos es un recorrido.
//!
//! ** Cero significa vacia, y por eso un hash que salga cero **se sube a uno**.
//! Perder un valor de 2^64 no cambia nada; tener dos significados para el mismo
//! byte, si.
//!
//! ## [!] LO QUE ESTA FORMA NO PROMETE, dicho antes de que alguien lo suponga
//!
//! **No crece sola.** Cuando se llena hasta el margen, `pon` contesta que no en
//! vez de rehacer la tabla con el doble de ranuras. Crecer una tabla es RE-HASH:
//! toda pareja cambia de sitio, asi que no es "reservar y copiar" como en una
//! lista -- es recorrer y volver a colocar.
//!
//! Se dice aqui por lo mismo que `lista` dice que no crece y que `suelta` estuvo
//! meses diciendo que no soltaba: **una promesa que no se cumple es peor que una
//! que no se hace.**

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Bytes de la cabecera. La misma que `lista`: multiplo de 16.
pub const CABECERA_LEN: usize = 32;

/// Lo que ocupa una pareja: marca, clave y valor.
pub const RANURA_LEN: usize = 24;

const O_REFS: usize = 0;
const O_TIPO: usize = 8;
const O_FLAGS: usize = 12;
const O_CUANTOS: usize = 16;
const O_CAPACIDAD: usize = 24;

/// Lo que se puede saber de una tabla mirandola.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tabla {
    pub refs: bx_u64,
    pub type_index: bx_u32,
    pub flags: bx_u32,
    /// Parejas ocupadas.
    pub cuantos: bx_u64,
    /// Ranuras que hay. **Siempre mayor que `cuantos`.**
    pub capacidad: bx_u64,
}

/// Por que una tabla no se sostiene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    /// Los bytes no llegan ni a la cabecera.
    Corta,
    /// Las ranuras no caben en el bloque.
    NoCabeEnLoQueMide,
    /// Contador a cero y no es inmortal: nadie la tiene y sigue viva.
    SinDuenoYViva,
    /// **Llena del todo, o mintiendo sobre cuantas parejas tiene.**
    ///
    /// *** Esta es la que separa una tabla de una lista. Una tabla de
    /// direccionamiento abierto llena **no termina de buscar**: la sonda da
    /// vueltas para siempre. Que `cuantos < capacidad` no es una holgura
    /// recomendada, es la condicion que hace que el bucle tenga final.
    SinMargen,
    /// Capacidad cero: no hay donde buscar.
    SinRanuras,
}

impl Falta {
    pub fn nombre(self) -> &'static str {
        match self {
            Falta::Corta => "la tabla no llega ni a su cabecera",
            Falta::NoCabeEnLoQueMide => "las ranuras de la tabla no caben en su bloque",
            Falta::SinDuenoYViva => "la tabla tiene cero referencias y no es inmortal",
            Falta::SinMargen => "la tabla esta llena: buscar en ella no terminaria",
            Falta::SinRanuras => "la tabla no tiene ni una ranura",
        }
    }
}

fn u32_en(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn u64_en(b: &[u8], i: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[i..i + 8]);
    u64::from_le_bytes(v)
}

/// **Cuantos bytes ocupa una tabla de `capacidad` ranuras.**
pub fn bytes_para(capacidad: bx_u64) -> Option<u64> {
    capacidad
        .checked_mul(RANURA_LEN as u64)?
        .checked_add(CABECERA_LEN as u64)
}

/// Donde empieza la ranura `i`, en bytes desde el principio de la tabla.
pub fn ranura(i: bx_u64) -> Option<u64> {
    i.checked_mul(RANURA_LEN as u64)?
        .checked_add(CABECERA_LEN as u64)
}

/// **Escribe la cabecera de una tabla recien nacida**, vacia y con un dueno.
///
/// [!] Las ranuras tienen que quedar A CERO, y eso NO lo hace esta funcion: lo
/// hace el monton, que entrega paginas limpias. Se dice porque una marca que no
/// sea cero es una ranura "ocupada" con basura dentro.
pub fn nacer(bloque: &mut [u8], type_index: bx_u32, capacidad: bx_u64) -> Option<()> {
    if bloque.len() < CABECERA_LEN || capacidad == 0 {
        return None;
    }
    bloque[O_REFS..O_REFS + 8].copy_from_slice(&1u64.to_le_bytes());
    bloque[O_TIPO..O_TIPO + 4].copy_from_slice(&type_index.to_le_bytes());
    bloque[O_FLAGS..O_FLAGS + 4].copy_from_slice(&0u32.to_le_bytes());
    bloque[O_CUANTOS..O_CUANTOS + 8].copy_from_slice(&0u64.to_le_bytes());
    bloque[O_CAPACIDAD..O_CAPACIDAD + 8].copy_from_slice(&capacidad.to_le_bytes());
    Some(())
}

/// Lee la cabecera. No comprueba nada: para eso esta [`revisar`].
pub fn leer(bloque: &[u8]) -> Option<Tabla> {
    if bloque.len() < CABECERA_LEN {
        return None;
    }
    Some(Tabla {
        refs: u64_en(bloque, O_REFS),
        type_index: u32_en(bloque, O_TIPO),
        flags: u32_en(bloque, O_FLAGS),
        cuantos: u64_en(bloque, O_CUANTOS),
        capacidad: u64_en(bloque, O_CAPACIDAD),
    })
}

/// **Que la tabla no mienta sobre si misma, NI SE PUEDA COLGAR AL BUSCARLA.**
pub fn revisar(bloque: &[u8]) -> Result<Tabla, Falta> {
    let t = leer(bloque).ok_or(Falta::Corta)?;
    if t.refs == 0 && !super::header::is_immortal(t.refs) {
        return Err(Falta::SinDuenoYViva);
    }
    if t.capacidad == 0 {
        return Err(Falta::SinRanuras);
    }
    // *** LA QUE SEPARA UNA TABLA DE UNA LISTA. Ver `Falta::SinMargen`.
    if t.cuantos >= t.capacidad {
        return Err(Falta::SinMargen);
    }
    let hacen_falta = bytes_para(t.capacidad).ok_or(Falta::NoCabeEnLoQueMide)?;
    if (bloque.len() as u64) < hacen_falta {
        return Err(Falta::NoCabeEnLoQueMide);
    }
    Ok(t)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use alloc::vec;

    fn con(capacidad: u64) -> alloc::vec::Vec<u8> {
        let mut b = vec![0u8; bytes_para(capacidad).unwrap() as usize];
        nacer(&mut b, 5, capacidad).unwrap();
        b
    }

    #[test]
    fn nace_vacia_con_un_dueno_y_sus_ranuras() {
        let b = con(8);
        let t = revisar(&b).unwrap();
        assert_eq!(t.refs, 1);
        assert_eq!(t.cuantos, 0, "vacia: tiene sitio y no tiene nada");
        assert_eq!(t.capacidad, 8);
        assert_eq!(t.type_index, 5);
    }

    /// La cabecera es la misma que la de una lista; lo que cambia es la ranura.
    #[test]
    fn la_ranura_mide_veinticuatro_y_la_cabecera_treinta_y_dos() {
        assert_eq!(CABECERA_LEN, super::super::lista::CABECERA_LEN);
        assert_eq!(RANURA_LEN, 24, "marca, clave y valor");
        assert_eq!(ranura(0).unwrap(), 32);
        assert_eq!(ranura(1).unwrap(), 56);
        assert_eq!(bytes_para(2).unwrap(), 32 + 48);
    }

    /// ***UNA TABLA LLENA NO SE ACEPTA, y ese es el punto entero.***
    ///
    /// ** No es una holgura recomendada: en direccionamiento abierto, buscar una
    /// clave que no esta en una tabla llena **da vueltas para siempre**. La
    /// condicion `cuantos < capacidad` es lo que hace que el bucle termine.
    #[test]
    fn una_tabla_llena_no_pasa_la_revision() {
        let mut b = con(4);
        // Cuatro parejas en cuatro ranuras: sin margen.
        b[O_CUANTOS..O_CUANTOS + 8].copy_from_slice(&4u64.to_le_bytes());
        assert_eq!(revisar(&b), Err(Falta::SinMargen));

        // Con tres si: queda una ranura donde la busqueda puede parar.
        b[O_CUANTOS..O_CUANTOS + 8].copy_from_slice(&3u64.to_le_bytes());
        assert!(revisar(&b).is_ok());
    }

    #[test]
    fn una_tabla_sin_ranuras_no_existe() {
        let mut b = vec![0u8; CABECERA_LEN];
        // `nacer` ya se niega, asi que se fabrica a mano para probar `revisar`.
        b[O_REFS..O_REFS + 8].copy_from_slice(&1u64.to_le_bytes());
        assert_eq!(revisar(&b), Err(Falta::SinRanuras));
        assert!(nacer(&mut b, 5, 0).is_none(), "y `nacer` tampoco la deja nacer");
    }

    #[test]
    fn una_tabla_que_dice_tener_mas_ranuras_de_las_que_caben_se_caza() {
        let mut b = con(2);
        b[O_CAPACIDAD..O_CAPACIDAD + 8].copy_from_slice(&9999u64.to_le_bytes());
        assert_eq!(revisar(&b), Err(Falta::NoCabeEnLoQueMide));
    }

    #[test]
    fn sin_dueno_y_viva_no_puede_existir() {
        let mut b = con(4);
        b[O_REFS..O_REFS + 8].copy_from_slice(&0u64.to_le_bytes());
        assert_eq!(revisar(&b), Err(Falta::SinDuenoYViva));
    }
}
