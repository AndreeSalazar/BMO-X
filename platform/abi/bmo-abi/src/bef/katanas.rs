//! **LAS KATANAS: que reglas trae este binario y DONDE corta cada una** -- la
//! seccion `0x16`.
//!
//! ## El problema, dicho en una linea
//!
//! Un binario puede decir *"no tengo comportamiento indefinido"* en su
//! manifiesto y no traer ni una comprobacion dentro. **Hoy no hay forma de
//! desmentirlo**: el manifiesto es una declaracion y nadie la contrasta con los
//! bytes.
//!
//! > Declarar sin comprobar es propaganda. Comprobar sin declarar es adivinar.
//! > **Las dos juntas son un contrato.**
//!
//! Esta tabla es la mitad que faltaba: el binario dice **por cada regla, su
//! codigo y donde esta su bloque de trampa**, y eso se puede ir a mirar.
//!
//! ## Por que el FORMATO vive en el ABI y el CONTENIDO no
//!
//! Hoy solo INTI emite reglas, asi que hoy solo INTI escribe esta seccion. Eso
//! podria haber vivido dentro del compilador de INTI, y habria sido un club
//! cerrado: un formato que solo entiende quien lo escribe.
//!
//! ** Vive aqui por lo contrario: **para que cualquiera pueda LEERLO**. Un
//! tercero que reciba un `.bex` puede comprobar las katanas sin pedirle permiso
//! a nadie ni tener el compilador delante. La exclusividad de INTI no esta en
//! que nadie mas pueda leer la tabla -- esta en que casi nadie va a querer pagar
//! lo que cuesta llenarla de verdad.
//!
//! Es la misma decision que puso REX en `tables/`: *ese sitio es la puerta de
//! los terceros.*
//!
//! ## Esto NO es formato nuevo: es el cuarto hueco que estaba vacio
//!
//! `Resources = 0x0B` estuvo declarada y sin escribir hasta que `bmo-pack` la
//! lleno. `Manifest = 0x09` igual, hasta ayer. `Requisitos = 0x15` nacio para
//! que Ring 0 dejara de deducir. Esta es la siguiente, y sigue la misma regla
//! que aquella: **el TOML se queda para humanos y esta se lee sin parser**.
//!
//! ## La disposicion
//!
//! ```text
//!   cabecera (16 B)
//!     0..4    magic "BKAT"
//!     4..8    cuantas katanas
//!     8..10   version de la tabla
//!     10..16  cero (reservado)
//!
//!   katana (16 B cada una, `cuantas` seguidas)
//!     0..4    codigo    -- el E1xxx sin la E: 1001, 1003, 1012
//!     4..8    offset    -- del bloque de trampa, DENTRO de la seccion Code
//!     8..12   longitud  -- del bloque, para poder comprobarlo entero
//!     12..16  cero (reservado)
//! ```
//!
//! **Registros de tamano fijo**, igual que los requisitos y el directorio de
//! recursos: la katana `i` esta en `16 + i*16` y el lector es una
//! multiplicacion. Se lee desde Rust, desde Ring 0 sin `alloc`, y desde C con
//! quince lineas.
//!
//! Los offsets son **relativos a la seccion `Code`** y no al fichero. Reemitir
//! el `.bex` con otra disposicion --meterle recursos, por ejemplo-- mueve todos
//! los offsets del fichero y **no invalida esta tabla**.
//!
//! ## [!] LO QUE ESTA TABLA NO DEMUESTRA, dicho por delante
//!
//! Que las katanas declaradas **esten** donde dice, se comprueba (`revisar`).
//! Que no falte ninguna, **no**: una tabla vacia es una tabla honesta de un
//! binario sin reglas, y desde aqui no se distingue de un binario que las
//! quito. Para eso hay que recorrer el codigo y exigir que cada operacion que
//! pide regla traiga la suya al lado -- y eso pide un decodificador, que es otro
//! trabajo y otro fichero.
//!
//! **Esta tabla cierra la mentira facil, no la dificil.** Decirlo aqui vale mas
//! que descubrirlo el dia que alguien se apoye en ella para algo que no aguanta.

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u16, bx_u32};
use alloc::vec::Vec;

/// `"BKAT"` en little-endian. Va **dentro** de la seccion: el magic del fichero
/// no cambia y un `.bex` con katanas sigue siendo un `.bex`.
pub const KATANAS_MAGIC: bx_u32 = u32::from_le_bytes(*b"BKAT");

/// La unica version que este sistema escribe y lee.
pub const VERSION: bx_u16 = 1;

/// Bytes de la cabecera de la tabla.
pub const CABECERA_LEN: usize = 16;
/// Bytes de cada katana.
pub const KATANA_LEN: usize = 16;

/// Cuantas caben. **Auditable a ojo**, que es el motivo del limite: una tabla
/// que puede declarar un millon de filas es una tabla que puede hacer que el
/// que la recorre tarde un minuto en decir que no.
pub const MAX_KATANAS: usize = 4096;

/// Una regla, y donde corta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Katana {
    /// El codigo que devuelve al atrapar. `1001` es `E1001`.
    pub codigo: bx_u32,
    /// Donde empieza su bloque de trampa, **dentro de la seccion `Code`**.
    pub offset: bx_u32,
    /// Cuanto mide el bloque.
    pub longitud: bx_u32,
}

/// Por que una tabla no vale.
///
/// ** Variantes y no un booleano por lo mismo que en el gate: cada una manda a
/// mirar un sitio distinto. *"la tabla esta mal"* no le sirve a nadie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    /// No llega ni a la cabecera.
    Corta,
    /// El magic no es `BKAT`.
    NoEsUnaTabla,
    /// Una version que este sistema no lee.
    OtraVersion,
    /// Dice mas katanas de las que caben en lo que mide.
    CuentaQueNoCabe,
    /// Mas de [`MAX_KATANAS`].
    Demasiadas,
    /// Los bytes reservados no son cero.
    ReservadoSucio,
    /// Una katana apunta fuera de la seccion de codigo.
    FueraDelCodigo,
    /// Una katana dice medir cero.
    BloqueVacio,
}

impl Falta {
    pub fn nombre(self) -> &'static str {
        match self {
            Falta::Corta => "la tabla de katanas no llega ni a la cabecera",
            Falta::NoEsUnaTabla => "la seccion de katanas no empieza por `BKAT`",
            Falta::OtraVersion => "la tabla de katanas es de otra version",
            Falta::CuentaQueNoCabe => "dice mas katanas de las que caben en la seccion",
            Falta::Demasiadas => "demasiadas katanas",
            Falta::ReservadoSucio => "los bytes reservados de la tabla no son cero",
            Falta::FueraDelCodigo => "una katana apunta fuera de la seccion de codigo",
            Falta::BloqueVacio => "una katana dice que su bloque mide cero",
        }
    }
}

fn u16_en(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_en(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// **Construye la tabla.** La escribe quien emitio los bloques, porque es el
/// unico que sabe en que offset acabo cada uno.
pub fn construir(katanas: &[Katana]) -> Result<Vec<u8>, Falta> {
    if katanas.len() > MAX_KATANAS {
        return Err(Falta::Demasiadas);
    }
    let mut b = Vec::with_capacity(CABECERA_LEN + katanas.len() * KATANA_LEN);
    b.extend_from_slice(&KATANAS_MAGIC.to_le_bytes());
    b.extend_from_slice(&(katanas.len() as u32).to_le_bytes());
    b.extend_from_slice(&VERSION.to_le_bytes());
    b.extend_from_slice(&[0u8; 6]);
    for k in katanas {
        if k.longitud == 0 {
            return Err(Falta::BloqueVacio);
        }
        b.extend_from_slice(&k.codigo.to_le_bytes());
        b.extend_from_slice(&k.offset.to_le_bytes());
        b.extend_from_slice(&k.longitud.to_le_bytes());
        b.extend_from_slice(&[0u8; 4]);
    }
    Ok(b)
}

/// **Lee la tabla, sin reservar nada.** Devuelve cuantas hay; cada una se saca
/// con [`katana`].
pub fn cuantas(seccion: &[u8]) -> Result<usize, Falta> {
    if seccion.len() < CABECERA_LEN {
        return Err(Falta::Corta);
    }
    if u32_en(seccion, 0) != KATANAS_MAGIC {
        return Err(Falta::NoEsUnaTabla);
    }
    if u16_en(seccion, 8) != VERSION {
        return Err(Falta::OtraVersion);
    }
    if seccion[10..16].iter().any(|&x| x != 0) {
        return Err(Falta::ReservadoSucio);
    }
    let n = u32_en(seccion, 4) as usize;
    if n > MAX_KATANAS {
        return Err(Falta::Demasiadas);
    }
    // ** La cuenta tiene que caber en lo que la seccion mide. Un `n` inventado
    // haria que el lector recorriera bytes de otra cosa creyendo que son
    // katanas -- el mismo fallo que la cabecera de `TablaCadenas` viene a
    // cerrar en las secciones de simbolos.
    if CABECERA_LEN + n * KATANA_LEN > seccion.len() {
        return Err(Falta::CuentaQueNoCabe);
    }
    Ok(n)
}

/// La katana `i`, si existe.
pub fn katana(seccion: &[u8], i: usize) -> Option<Katana> {
    let e = CABECERA_LEN + i * KATANA_LEN;
    if e + KATANA_LEN > seccion.len() {
        return None;
    }
    Some(Katana {
        codigo: u32_en(seccion, e),
        offset: u32_en(seccion, e + 4),
        longitud: u32_en(seccion, e + 8),
    })
}

/// **Que la tabla no mienta sobre donde estan sus bloques.**
///
/// `codigo_len` es lo que mide la seccion `Code` del mismo binario. Sin ese
/// numero esto no se puede contestar, y por eso entra por argumento: esta
/// funcion no abre ficheros ni sabe que es un BEF.
///
/// ** Lo que comprueba es que **cada bloque cae dentro del codigo**. Que los
/// bytes de ese bloque sean de verdad una trampa es otra pregunta y de otro
/// --hace falta saber x86, y aqui no se sabe ni se quiere saber.
pub fn revisar(seccion: &[u8], codigo_len: usize) -> Result<usize, Falta> {
    let n = cuantas(seccion)?;
    for i in 0..n {
        let k = katana(seccion, i).ok_or(Falta::CuentaQueNoCabe)?;
        if k.longitud == 0 {
            return Err(Falta::BloqueVacio);
        }
        let fin = (k.offset as usize).checked_add(k.longitud as usize);
        match fin {
            Some(f) if f <= codigo_len => {}
            _ => return Err(Falta::FueraDelCodigo),
        }
    }
    Ok(n)
}

/// **El bloque materializa el numero que la tabla dice?**
///
/// ## Que prueba esto exactamente, que es menos de lo que parece
///
/// Busca los ocho bytes del codigo, en little-endian, dentro del bloque. Si no
/// estan, **el bloque no esta devolviendo ese numero como un inmediato**.
///
/// ** Y ahi acaba lo que se puede afirmar desde aqui, que no sabe x86 ni lo
/// quiere saber. Un bloque podria construir el numero con dos operaciones en
/// vez de cargarlo entero, y esta funcion diria que no lo lleva. Para INTI eso
/// no pasa --su trampa es un `mov` de un inmediato y nada mas-- pero para un
/// binario de otro sitio esto es una condicion **necesaria y no suficiente**.
///
/// Se dice aqui y no en el sitio que la llama porque una funcion que promete de
/// mas es peor que una que no existe: la primera hace que alguien deje de
/// mirar.
pub fn lleva_su_codigo(bloque: &[u8], codigo: bx_u32) -> bool {
    let buscado = (codigo as u64).to_le_bytes();
    bloque.windows(8).any(|v| v == buscado)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn tres() -> Vec<Katana> {
        alloc::vec![
            Katana { codigo: 1001, offset: 100, longitud: 15 },
            Katana { codigo: 1003, offset: 115, longitud: 15 },
            Katana { codigo: 1012, offset: 130, longitud: 15 },
        ]
    }

    #[test]
    fn la_ida_y_la_vuelta() {
        let t = construir(&tres()).unwrap();
        assert_eq!(t.len(), CABECERA_LEN + 3 * KATANA_LEN);
        assert_eq!(cuantas(&t).unwrap(), 3);
        for (i, esperada) in tres().iter().enumerate() {
            assert_eq!(katana(&t, i).unwrap(), *esperada);
        }
        assert_eq!(revisar(&t, 200).unwrap(), 3);
    }

    /// **Una katana que apunta fuera del codigo se caza.** Es la mentira que
    /// esta tabla existe para cerrar.
    #[test]
    fn un_bloque_fuera_del_codigo_no_pasa() {
        let t = construir(&tres()).unwrap();
        // El codigo mide 140: la tercera katana acaba en 145.
        assert_eq!(revisar(&t, 140), Err(Falta::FueraDelCodigo));
        // Y justo: acaba en 145, asi que con 145 cabe.
        assert_eq!(revisar(&t, 145).unwrap(), 3);
    }

    /// Una cuenta inventada haria leer bytes de otra cosa creyendo que son
    /// katanas.
    #[test]
    fn una_cuenta_que_no_cabe_no_pasa() {
        let mut t = construir(&tres()).unwrap();
        t[4..8].copy_from_slice(&999u32.to_le_bytes());
        assert_eq!(cuantas(&t), Err(Falta::CuentaQueNoCabe));
    }

    #[test]
    fn lo_que_no_es_una_tabla_se_dice_con_su_nombre() {
        assert_eq!(cuantas(&[]), Err(Falta::Corta));
        assert_eq!(cuantas(&[0u8; 16]), Err(Falta::NoEsUnaTabla));
        let mut t = construir(&tres()).unwrap();
        t[8..10].copy_from_slice(&7u16.to_le_bytes());
        assert_eq!(cuantas(&t), Err(Falta::OtraVersion));
    }

    /// Los reservados se vigilan HOY. Un mecanismo de crecimiento que no se
    /// valida hasta que hace falta llega tarde: para entonces hay binarios con
    /// basura dentro. Es la misma regla que `header._reserved`.
    #[test]
    fn el_reservado_sucio_no_pasa() {
        let mut t = construir(&tres()).unwrap();
        t[12] = 0xFF;
        assert_eq!(cuantas(&t), Err(Falta::ReservadoSucio));
    }

    /// Una tabla vacia es **valida y honesta**: un binario sin reglas.
    #[test]
    fn ninguna_katana_es_una_respuesta_no_una_ausencia() {
        let t = construir(&[]).unwrap();
        assert_eq!(cuantas(&t).unwrap(), 0);
        assert_eq!(revisar(&t, 0).unwrap(), 0);
    }
}
