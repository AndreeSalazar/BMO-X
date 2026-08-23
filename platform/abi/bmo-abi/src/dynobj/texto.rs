//! **LA FORMA DE UN `texto` EN MEMORIA.**
//!
//! ## Es hermano de `lista`, y las diferencias son las que importan
//!
//! `dynobj::lista` ya instancio la cabecera comun. Esto no la reinventa: la usa,
//! y se separa de ella en dos sitios. Los dos salen de la MISMA linea de la
//! gramatica:
//!
//! ```text
//!    `texto`   cadena UTF-8 INMUTABLE
//! ```
//!
//! ### 1. Inmutable, luego SIN `capacidad`
//!
//! Una lista guarda `count` y `capacidad` porque puede crecer dentro de su
//! bloque. Un texto no crece nunca: **pegarle otro texto no lo alarga, produce
//! uno nuevo**. Guardar cuanto "cabe" en algo que no puede cambiar seria un
//! campo que nadie lee y que puede mentir.
//!
//! Asi que la cabecera es exactamente `DynVarHeader` --24 bytes-- y no las 32
//! de una lista.
//!
//! ** Y el precio de la inmutabilidad, dicho por delante: **concatenar dentro de
//! un bucle reserva una vez por vuelta.** Es el mismo precio que paga Java, y la
//! salida es la misma que en todas partes --un constructor aparte-- pero eso es
//! otra pieza y no se va a fingir que sale gratis de esta.
//!
//! ### 2. `bytes`, y NO caracteres
//!
//! `lista` documenta que su `count` son **elementos**. Aqui son **bytes**, y es
//! una decision, no una comodidad:
//!
//! ```text
//!    "anos"   sin la virgulilla  ->  4 bytes,  4 caracteres
//!    "anos"   con la virgulilla  ->  5 bytes,  4 caracteres
//! ```
//!
//! Contar caracteres en UTF-8 **es un recorrido**, no una lectura. Poner ese
//! numero en la cabecera obligaria a mantenerlo y, antes, a decidir que es un
//! caracter -- que es donde empieza el pozo sin fondo que el maestro deja FUERA
//! por escrito: *"Unicode completo (locale, colacion, normalizacion)"*.
//!
//! *** La regla que sale de aqui: **la cabecera guarda lo que se lee de un
//! `mov`.** Lo que cuesta un recorrido se pregunta, y se ve que cuesta.
//!
//! ### 3. La cabecera mide 24 y no un multiplo de 16
//!
//! `lista` eligio 32 *"porque el monton reparte a 16"*. Aqui no hace falta: el
//! monton alinea **cada bloque** al entregarlo, asi que lo que empieza en el
//! byte 24 de un bloque alineado a 16 queda alineado a 8 -- y los bytes de un
//! texto no piden mas de 1.
//!
//! Ahorrar esos ocho bytes importa aqui y no en una lista por una razon de
//! cantidad: **de textos hay muchos, y son cortos.**
//!
//! ## *** LO QUE ESTA FORMA REGALA: EL LITERAL NO CUESTA NADA
//!
//! La seccion 10.2 del maestro lo tenia decidido desde antes de este fichero:
//!
//! ```text
//!    CONGELADO   inmortal. Nadie lo cambia, nadie cuenta sus referencias.
//!                literales, constantes, un modulo cargado
//! ```
//!
//! Un literal de texto es CONGELADO, y eso lo cambia todo:
//!
//! ```text
//!    vive en RoData          no se reserva en el monton
//!    refs = IMMORTAL         nadie toca su contador, jamas
//!    se valida al compilar   el UTF-8 se mira una vez, no en cada arranque
//! ```
//!
//! ** O sea que `x = "hola"` **no necesita runtime**: son bytes en una seccion
//! de solo lectura, y una direccion. Y ya se puede hacer, porque `RoData` y sus
//! reubicaciones llegaron a bytes el 22-08.
//!
//! [!] Lo que SI necesita monton es el texto CONSTRUIDO --leer un fichero,
//! juntar dos-- y eso espera al contador de referencias. Decir *"INTI ya tiene
//! textos"* el dia que solo anden los literales seria exactamente la clase de
//! frase que este proyecto no deja pasar.

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Bytes de la cabecera: `DynVarHeader` y nada mas.
pub const CABECERA_LEN: usize = 24;

const O_REFS: usize = 0;
const O_TIPO: usize = 8;
const O_FLAGS: usize = 12;
const O_BYTES: usize = 16;

/// Lo que se puede saber de un texto mirandolo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Texto {
    pub refs: bx_u64,
    pub type_index: bx_u32,
    pub flags: bx_u32,
    /// **BYTES**, no caracteres. Ver la cabecera del modulo.
    pub bytes: bx_u64,
}

/// Por que un texto no se sostiene.
///
/// ** Variantes y no un booleano por lo mismo que en el gate de los `.bex` y en
/// `lista`: cada una manda a mirar un sitio distinto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    /// Los bytes no llegan ni a la cabecera.
    Corta,
    /// Dice medir mas de lo que hay en el bloque.
    NoCabeEnLoQueMide,
    /// Contador a cero y no es inmortal: nadie lo tiene y sigue vivo.
    SinDuenoYVivo,
    /// **Sus bytes no son UTF-8.**
    ///
    /// ** Existe porque la gramatica lo promete --*"un texto es UTF-8 y se
    /// valida al construirlo"*-- y una promesa que no comprueba nadie no es una
    /// promesa. Es tambien la unica `Falta` de esta familia que mira el
    /// CONTENIDO en vez de la cabecera.
    NoEsUtf8,
}

impl Falta {
    pub fn nombre(self) -> &'static str {
        match self {
            Falta::Corta => "el texto no llega ni a su cabecera",
            Falta::NoCabeEnLoQueMide => "el texto dice medir mas de lo que hay",
            Falta::SinDuenoYVivo => "el texto tiene cero referencias y no es inmortal",
            Falta::NoEsUtf8 => "los bytes del texto no son UTF-8",
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

/// **Cuantos bytes ocupa un texto con `bytes` bytes de contenido.**
///
/// No lleva `ancho` --al contrario que `lista::bytes_para`-- porque lo que mide
/// un byte es uno. Que aqui sobre ese argumento **es la diferencia entre los dos
/// tipos convertida en una firma**.
pub fn bytes_para(bytes: bx_u64) -> Option<u64> {
    bytes.checked_add(CABECERA_LEN as u64)
}

/// **Escribe la cabecera de un texto recien nacido**, con un dueno.
///
/// Para el que se construye en ejecucion. El literal no pasa por aqui: nace
/// [`congelado`].
pub fn nacer(bloque: &mut [u8], type_index: bx_u32, bytes: bx_u64) -> Option<()> {
    escribe_cabecera(bloque, 1, type_index, bytes)
}

/// **Escribe la cabecera de un texto CONGELADO**: el literal.
///
/// *** Nace con el bit 63 puesto, que es lo que hace que **nadie cuente sus
/// referencias nunca**. No es una optimizacion que se aplique despues: es el
/// estado en el que se escribe, y ademas en una seccion de solo lectura.
///
/// Que sean dos funciones y no un booleano es a proposito: en el sitio de la
/// llamada se lee cual de las dos vidas esta naciendo.
pub fn congelado(bloque: &mut [u8], type_index: bx_u32, bytes: bx_u64) -> Option<()> {
    escribe_cabecera(bloque, super::header::IMMORTAL, type_index, bytes)
}

fn escribe_cabecera(bloque: &mut [u8], refs: u64, type_index: bx_u32, bytes: bx_u64) -> Option<()> {
    if bloque.len() < CABECERA_LEN {
        return None;
    }
    bloque[O_REFS..O_REFS + 8].copy_from_slice(&refs.to_le_bytes());
    bloque[O_TIPO..O_TIPO + 4].copy_from_slice(&type_index.to_le_bytes());
    bloque[O_FLAGS..O_FLAGS + 4].copy_from_slice(&0u32.to_le_bytes());
    bloque[O_BYTES..O_BYTES + 8].copy_from_slice(&bytes.to_le_bytes());
    Some(())
}

/// Lee la cabecera. No comprueba nada: para eso esta [`revisar`].
pub fn leer(bloque: &[u8]) -> Option<Texto> {
    if bloque.len() < CABECERA_LEN {
        return None;
    }
    Some(Texto {
        refs: u64_en(bloque, O_REFS),
        type_index: u32_en(bloque, O_TIPO),
        flags: u32_en(bloque, O_FLAGS),
        bytes: u64_en(bloque, O_BYTES),
    })
}

/// Los bytes del contenido, sin la cabecera.
pub fn contenido(bloque: &[u8]) -> Option<&[u8]> {
    let t = leer(bloque)?;
    let fin = bytes_para(t.bytes)? as usize;
    bloque.get(CABECERA_LEN..fin)
}

/// **Que el texto no mienta sobre si mismo, NI SOBRE SUS BYTES.**
///
/// ** Es la unica revision de esta familia que mira el contenido, y no es un
/// capricho: la gramatica promete que un texto es UTF-8 valido. Si eso no lo
/// comprueba nadie, la promesa se convierte en *"casi siempre"* -- y el dia que
/// no lo sea, el fallo sale en la consola, a tres capas de distancia de aqui.
///
/// [!] Del literal se comprueba **al compilar**, una vez, y no en cada arranque.
/// Por eso `congelado` existe aparte de `nacer`.
pub fn revisar(bloque: &[u8]) -> Result<Texto, Falta> {
    let t = leer(bloque).ok_or(Falta::Corta)?;
    if t.refs == 0 && !super::header::is_immortal(t.refs) {
        return Err(Falta::SinDuenoYVivo);
    }
    let hacen_falta = bytes_para(t.bytes).ok_or(Falta::NoCabeEnLoQueMide)?;
    if (bloque.len() as u64) < hacen_falta {
        return Err(Falta::NoCabeEnLoQueMide);
    }
    let datos = contenido(bloque).ok_or(Falta::NoCabeEnLoQueMide)?;
    core::str::from_utf8(datos).map_err(|_| Falta::NoEsUtf8)?;
    Ok(t)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use alloc::vec;

    fn con(s: &str, congelar: bool) -> alloc::vec::Vec<u8> {
        let n = s.len() as u64;
        let mut b = vec![0u8; bytes_para(n).unwrap() as usize];
        if congelar {
            congelado(&mut b, 3, n).unwrap();
        } else {
            nacer(&mut b, 3, n).unwrap();
        }
        b[CABECERA_LEN..].copy_from_slice(s.as_bytes());
        b
    }

    /// La diferencia con `lista` no es un detalle de ahorro: es que un texto no
    /// crece, asi que no tiene `capacidad` que guardar.
    #[test]
    fn la_cabecera_mide_24_y_una_lista_32() {
        assert_eq!(CABECERA_LEN, 24);
        assert_eq!(super::super::lista::CABECERA_LEN, 32);
        assert_eq!(bytes_para(4).unwrap(), 28);
    }

    #[test]
    fn nace_con_un_dueno_y_se_lee_entero() {
        let b = con("hola", false);
        let t = revisar(&b).unwrap();
        assert_eq!(t.refs, 1, "nace con una referencia: la de quien lo pidio");
        assert_eq!(t.bytes, 4);
        assert_eq!(t.type_index, 3);
        assert_eq!(contenido(&b).unwrap(), b"hola");
    }

    /// *** El literal nace INMORTAL, y eso es lo que lo hace gratis.
    #[test]
    fn un_literal_nace_congelado_y_nadie_le_cuenta_las_referencias() {
        let b = con("hola", true);
        let t = revisar(&b).unwrap();
        assert!(
            super::super::header::is_immortal(t.refs),
            "un literal es CONGELADO: seccion 10.2 del maestro"
        );
        // Y sigue valiendo con la parte baja del contador a cero: lo unico que
        // se mira es el bit 63.
        assert_eq!(t.refs & !super::super::header::IMMORTAL, 0);
    }

    /// **BYTES, no caracteres**, y este es el caso que lo demuestra.
    #[test]
    fn la_cabecera_cuenta_bytes_y_no_caracteres() {
        let b = con("anos", false);
        assert_eq!(revisar(&b).unwrap().bytes, 4);

        // La misma palabra con la ene: cuatro caracteres, CINCO bytes.
        let con_ene = "a\u{00f1}os";
        assert_eq!(con_ene.chars().count(), 4);
        let b2 = con(con_ene, false);
        assert_eq!(
            revisar(&b2).unwrap().bytes,
            5,
            "la cabecera guarda lo que se lee de un `mov`, no lo que cuesta un recorrido"
        );
    }

    /// La promesa de la gramatica, comprobada: *"un texto es UTF-8 y se valida
    /// al construirlo"*.
    #[test]
    fn unos_bytes_que_no_son_utf8_se_rechazan() {
        let mut b = vec![0u8; bytes_para(2).unwrap() as usize];
        nacer(&mut b, 3, 2).unwrap();
        // 0xFF no aparece en ninguna secuencia UTF-8 valida.
        b[CABECERA_LEN] = 0xFF;
        b[CABECERA_LEN + 1] = 0xFE;
        assert_eq!(revisar(&b), Err(Falta::NoEsUtf8));
    }

    #[test]
    fn un_texto_que_dice_medir_mas_de_lo_que_hay_se_caza() {
        let mut b = con("hola", false);
        b[O_BYTES..O_BYTES + 8].copy_from_slice(&9999u64.to_le_bytes());
        assert_eq!(revisar(&b), Err(Falta::NoCabeEnLoQueMide));
    }

    #[test]
    fn sin_dueno_y_vivo_no_puede_existir() {
        let mut b = con("hola", false);
        b[O_REFS..O_REFS + 8].copy_from_slice(&0u64.to_le_bytes());
        assert_eq!(revisar(&b), Err(Falta::SinDuenoYVivo));
    }

    #[test]
    fn un_bloque_que_no_llega_a_la_cabecera_no_se_lee_a_medias() {
        let b = vec![0u8; 10];
        assert_eq!(leer(&b), None);
        assert_eq!(revisar(&b), Err(Falta::Corta));
    }
}
