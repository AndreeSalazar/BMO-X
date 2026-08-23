//! **LA FORMA DE UNA `lista de T` EN MEMORIA.**
//!
//! ## Que se decide aqui, y que estaba decidido ya
//!
//! Casi nada de esto es nuevo, y decirlo importa. `dynobj::header` lleva escrito
//! el contrato desde antes de que existiera un interprete:
//!
//! ```text
//!    refs         DynHeader        contador, bit 63 = INMORTAL
//!    type_index   DynHeader        INDICE en la tabla de tipos, nunca puntero
//!    flags        DynHeader        LENT / TRACKED
//!    count        DynVarHeader     ELEMENTOS, no bytes
//! ```
//!
//! Y `DynVarHeader` dice en su propia cabecera para que es: *"a dynamic object
//! whose size is known only at runtime: **list**, str, tuple, int"*.
//!
//! ** Es la cuarta vez que pasa lo mismo en este arbol: `Resources = 0x0B`,
//! `Manifest = 0x09`, `Requisitos = 0x15` y ahora esto. **El contrato estaba
//! completo y no lo habia instanciado nadie.** Lo dice el propio `dynobj`:
//! *"the CONTRACT may be complete, the IMPLEMENTATION is the seed"*.
//!
//! ## *** LO QUE INTI ANADE, Y POR QUE NO PODIA SALIR DE OTRO SITIO
//!
//! **`capacidad`**: cuantos elementos CABEN, frente a los `count` que hay.
//!
//! La primera idea fue no guardarla y sacarla del asignador -- si el monton sabe
//! lo que mide el bloque, la capacidad es una resta. **Y no se puede**, y el
//! motivo esta medido en `MONTON.md`:
//!
//! ```text
//!    pide(monton, cuantos)    reserva, alineado a 16
//!    suelta(monton, trozo)    hoy no hace nada
//! ```
//!
//! Es un asignador de EMPUJE: no hay cabecera por bloque, asi que **un bloque no
//! sabe cuanto mide**. La capacidad tiene que viajar en el objeto o no existe.
//!
//! ## Y lo que NO se guarda, a proposito: el ancho del elemento
//!
//! Sale del `type_index`, y `DynVarHeader` lo deja escrito con su motivo:
//! *"the element width comes from the type, and confusing the two is the oldest
//! bug in this shape of code"*.
//!
//! ** Guardarlo aqui haria la lista autodescriptiva HOY --y la tabla de tipos
//! todavia no existe-- a cambio de tener el mismo dato en dos sitios. Se
//! prefiere la dependencia visible: **`SectionKind::TypeMap = 0x10` es el quinto
//! hueco declarado y vacio**, y esa es la pieza que falta.
//!
//! ## La disposicion
//!
//! ```text
//!   cabecera (32 B, y 32 es multiplo de 16 porque el monton reparte a 16)
//!     0..8    refs         bit 63 = INMORTAL
//!     8..12   type_index   indice, nunca puntero
//!     12..16  flags        LENT / TRACKED
//!     16..24  count        elementos OCUPADOS
//!     24..32  capacidad    elementos que CABEN
//!
//!   los elementos, desde el byte 32
//! ```
//!
//! `count` y `capacidad` miden lo mismo --elementos-- y son del mismo ancho a
//! proposito: dos campos que se comparan en cada `anade` y que no pueden
//! desbordar el uno contra el otro por ser de distinto tamano.
//!
//! ## *** LO QUE ESTA FORMA NO PROMETE TODAVIA
//!
//! **El contador cuenta y no libera.** `release` puede llegar a cero y la
//! memoria no vuelve a ningun sitio, porque `suelta` no suelta. Eso no es un
//! fallo de esta cabecera --el contador es correcto-- pero decir *"INTI libera
//! al instante"* seria falso hasta que el monton sepa recibir.
//!
//! Se escribe aqui porque es exactamente la clase de frase que se queda en una
//! portada y nadie vuelve a comprobar.

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Bytes de la cabecera. Multiplo de 16, como reparte el monton.
pub const CABECERA_LEN: usize = 32;

const O_REFS: usize = 0;
const O_TIPO: usize = 8;
const O_FLAGS: usize = 12;
const O_COUNT: usize = 16;
const O_CAPACIDAD: usize = 24;

/// Lo que se puede saber de una lista mirandola.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lista {
    pub refs: bx_u64,
    pub type_index: bx_u32,
    pub flags: bx_u32,
    /// Elementos ocupados.
    pub count: bx_u64,
    /// Elementos que caben.
    pub capacidad: bx_u64,
}

/// Por que una lista no se sostiene.
///
/// ** Variantes y no un booleano por lo mismo que en el gate de los `.bex`:
/// cada una manda a mirar un sitio distinto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    /// Los bytes no llegan ni a la cabecera.
    Corta,
    /// Dice tener mas elementos de los que caben.
    MasDeLosQueCaben,
    /// Los elementos no caben en los bytes que hay.
    NoCabeEnLoQueMide,
    /// Contador a cero y no es inmortal: nadie la tiene y sigue viva.
    SinDuenoYViva,
}

impl Falta {
    pub fn nombre(self) -> &'static str {
        match self {
            Falta::Corta => "la lista no llega ni a su cabecera",
            Falta::MasDeLosQueCaben => "la lista dice tener mas elementos de los que caben",
            Falta::NoCabeEnLoQueMide => "los elementos de la lista no caben en su bloque",
            Falta::SinDuenoYViva => "la lista tiene cero referencias y no es inmortal",
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

/// **Escribe la cabecera de una lista recien nacida.**
///
/// Nace con UNA referencia --la de quien la pidio-- y con `count` a cero: tiene
/// sitio y no tiene nada. Devuelve `None` si el bloque no da ni para la
/// cabecera, en vez de escribir media.
pub fn nacer(bloque: &mut [u8], type_index: bx_u32, capacidad: bx_u64) -> Option<()> {
    if bloque.len() < CABECERA_LEN {
        return None;
    }
    bloque[O_REFS..O_REFS + 8].copy_from_slice(&1u64.to_le_bytes());
    bloque[O_TIPO..O_TIPO + 4].copy_from_slice(&type_index.to_le_bytes());
    bloque[O_FLAGS..O_FLAGS + 4].copy_from_slice(&0u32.to_le_bytes());
    bloque[O_COUNT..O_COUNT + 8].copy_from_slice(&0u64.to_le_bytes());
    bloque[O_CAPACIDAD..O_CAPACIDAD + 8].copy_from_slice(&capacidad.to_le_bytes());
    Some(())
}

/// Lee la cabecera. No comprueba nada: para eso esta [`revisar`].
pub fn leer(bloque: &[u8]) -> Option<Lista> {
    if bloque.len() < CABECERA_LEN {
        return None;
    }
    Some(Lista {
        refs: u64_en(bloque, O_REFS),
        type_index: u32_en(bloque, O_TIPO),
        flags: u32_en(bloque, O_FLAGS),
        count: u64_en(bloque, O_COUNT),
        capacidad: u64_en(bloque, O_CAPACIDAD),
    })
}

/// **Cuantos bytes ocupa una lista de `capacidad` elementos de `ancho` bytes.**
///
/// El `ancho` entra por argumento y no sale de aqui: **viene del tipo**, y este
/// modulo no conoce la tabla de tipos. Es la misma frontera que `DynVarHeader`
/// dejo escrita.
pub fn bytes_para(capacidad: bx_u64, ancho: bx_u64) -> Option<u64> {
    capacidad
        .checked_mul(ancho)?
        .checked_add(CABECERA_LEN as u64)
}

/// **Que la lista no mienta sobre si misma.**
///
/// `ancho` es lo que mide un elemento de su tipo. Sin ese numero no se puede
/// contestar si los elementos caben, y por eso entra por argumento en vez de
/// suponerse.
pub fn revisar(bloque: &[u8], ancho: bx_u64) -> Result<Lista, Falta> {
    let l = leer(bloque).ok_or(Falta::Corta)?;
    if l.count > l.capacidad {
        return Err(Falta::MasDeLosQueCaben);
    }
    // ** Cero referencias y mortal: nadie la tiene y sigue aqui. Es el estado
    // que no puede existir, y por eso se caza -- una lista que se lee despues
    // de soltarla es el fallo clasico de un contador, y aqui al menos se ve.
    if l.refs == 0 && !super::header::is_immortal(l.refs) {
        return Err(Falta::SinDuenoYViva);
    }
    let hacen_falta = bytes_para(l.capacidad, ancho).ok_or(Falta::NoCabeEnLoQueMide)?;
    if (bloque.len() as u64) < hacen_falta {
        return Err(Falta::NoCabeEnLoQueMide);
    }
    Ok(l)
}

/// Donde empieza el elemento `i`, en bytes desde el principio de la lista.
///
/// ** Es una multiplicacion y una suma, y esa es la mitad del motivo por el que
/// la **Regla 2 sale casi gratis** de esta forma: el limite contra el que
/// comparar --`count`-- esta a un `mov` de distancia, en un sitio fijo de la
/// cabecera. Un `bufer` no lo tiene, y por eso indexarlo pide `crudo`.
pub fn donde(i: bx_u64, ancho: bx_u64) -> Option<u64> {
    i.checked_mul(ancho)?.checked_add(CABECERA_LEN as u64)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use alloc::vec;

    fn bloque(capacidad: u64, ancho: u64) -> alloc::vec::Vec<u8> {
        vec![0u8; bytes_para(capacidad, ancho).unwrap() as usize]
    }

    #[test]
    fn nace_con_un_dueno_y_sin_elementos() {
        let mut b = bloque(4, 8);
        nacer(&mut b, 7, 4).unwrap();
        let l = revisar(&b, 8).unwrap();
        assert_eq!(l.refs, 1, "nace con una referencia: la de quien la pidio");
        assert_eq!(l.count, 0, "tiene sitio y no tiene nada");
        assert_eq!(l.capacidad, 4);
        assert_eq!(l.type_index, 7);
    }

    /// **La cabecera mide 32 y 32 es multiplo de 16.**
    ///
    /// ** No es cosmetica: el monton reparte alineado a 16, asi que una cabecera
    /// que no fuera multiplo dejaria los elementos desalineados detras -- y una
    /// lectura desalineada de 8 bytes es lenta en el mejor caso y una excepcion
    /// en el peor.
    #[test]
    fn la_cabecera_respeta_la_alineacion_del_monton() {
        assert_eq!(CABECERA_LEN % 16, 0);
        assert_eq!(donde(0, 8).unwrap() % 16, 0, "el primer elemento va alineado");
    }

    /// **Mas elementos de los que caben se caza.** Es la mentira que esta forma
    /// existe para cerrar, y la que haria que un `anade` escribiera fuera.
    #[test]
    fn una_lista_que_dice_tener_de_mas_no_pasa() {
        let mut b = bloque(4, 8);
        nacer(&mut b, 0, 4).unwrap();
        b[O_COUNT..O_COUNT + 8].copy_from_slice(&9u64.to_le_bytes());
        assert_eq!(revisar(&b, 8), Err(Falta::MasDeLosQueCaben));
    }

    /// Y una capacidad que no cabe en el bloque tampoco.
    #[test]
    fn una_capacidad_que_no_cabe_en_su_bloque_no_pasa() {
        let mut b = bloque(4, 8);
        nacer(&mut b, 0, 400).unwrap();
        assert_eq!(revisar(&b, 8), Err(Falta::NoCabeEnLoQueMide));
    }

    /// **Cero referencias y mortal es el estado que no puede existir.**
    #[test]
    fn una_lista_sin_dueno_y_viva_no_pasa() {
        let mut b = bloque(2, 8);
        nacer(&mut b, 0, 2).unwrap();
        b[O_REFS..O_REFS + 8].copy_from_slice(&0u64.to_le_bytes());
        assert_eq!(revisar(&b, 8), Err(Falta::SinDuenoYViva));
    }

    /// Una lista INMORTAL no cuenta referencias, y eso no la invalida.
    #[test]
    fn una_lista_inmortal_no_necesita_dueno() {
        let mut b = bloque(2, 8);
        nacer(&mut b, 0, 2).unwrap();
        b[O_REFS..O_REFS + 8]
            .copy_from_slice(&super::super::header::IMMORTAL_REFS.to_le_bytes());
        let l = revisar(&b, 8).unwrap();
        assert!(super::super::header::is_immortal(l.refs));
    }

    /// **Los elementos empiezan donde dice `donde`, y no se solapan.**
    #[test]
    fn los_elementos_van_uno_detras_de_otro() {
        assert_eq!(donde(0, 8).unwrap(), 32);
        assert_eq!(donde(1, 8).unwrap(), 40);
        assert_eq!(donde(10, 4).unwrap(), 72);
    }

    /// Una multiplicacion que se sale NO devuelve un numero pequeno.
    ///
    /// ** Es el fallo clasico de esta forma de codigo: `capacidad * ancho` da la
    /// vuelta y el bloque parece caber. Aqui da `None`, que es una respuesta.
    #[test]
    fn una_cuenta_que_desborda_dice_que_no() {
        assert_eq!(bytes_para(u64::MAX, 8), None);
        assert_eq!(donde(u64::MAX, 8), None);
    }
}
