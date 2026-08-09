//! **El directorio de RECURSOS de un paquete BEF** -- la seccion `0x0B`.
//!
//! ## Que problema resuelve
//!
//! Un programa de verdad no es un fichero de codigo: DOOM son 1,3 MB de
//! instrucciones **y 4,2 MB de WAD**, y hoy esas dos mitades viajan sueltas.
//! Copiar una app a otra maquina es copiar un `.bex` y acordarse de sus datos,
//! y ejecutarla es que los datos esten en la ruta que el programa espera.
//!
//! Un paquete es **un solo fichero** que lleva las dos cosas dentro. BMO-X lo
//! lee como lo que es; para Windows es un fichero opaco, que es exactamente lo
//! que se quiere de un binario firmado en el disco de datos.
//!
//! ## Esto NO es formato nuevo
//!
//! `SectionKind::Resources = 0x0B` esta declarada en `sections.rs` desde que se
//! diseno BEF --*"recursos arbitrarios (texturas BC7, audio Opus, fonts)"*-- y
//! hasta hoy **nadie la escribia y nadie la leia**. Y el cargador del kernel ya
//! esta preparado para su mitad: `bex::is_loadable` mapea Code/RoData/Data/Bss
//! y **el resto lo salta y lo cuenta**. O sea que un `.bex` con recursos dentro
//! **arranca hoy**: el kernel mapea el codigo e ignora el resto.
//!
//! Lo que faltaba no era guardar los datos: era **poder pedirlos de vuelta**.
//! Esto es el indice que lo permite.
//!
//! ## La disposicion, y por que es tan rigida
//!
//! ```text
//!   cabecera (16 B)
//!     0..4    magic "BRES"
//!     4..8    cuantos recursos
//!     8..16   cero (reservado)
//!
//!   entrada (64 B cada una, `count` seguidas)
//!     0..8    offset  -- DESDE EL INICIO DE ESTA SECCION, no del fichero
//!     8..16   tamano
//!     16      largo del nombre
//!     17..64  el nombre, 47 bytes, sin terminador
//!
//!   los datos, uno detras de otro
//! ```
//!
//! **Entradas de tamano fijo**, a proposito. Un indice con nombres de longitud
//! variable se recorre saltando por punteros, y eso pide o reservar memoria o
//! escribir un parser con estado. Con 64 bytes clavados, la entrada `i` esta en
//! `16 + i*64` y el lector es una multiplicacion -- que es lo que hace que el
//! mismo formato se pueda leer desde Rust, desde el kernel sin `alloc`, y desde
//! **C con veinte lineas**, que es quien de verdad lo va a leer.
//!
//! ** Y los offsets son **relativos a la seccion**, no al fichero. Asi el
//! bloque que produce [`construir`] es el mismo lo coloque el escritor donde lo
//! coloque: quien empaqueta no tiene que saber en que byte va a acabar su
//! seccion, y volver a emitir el `.bex` con otra disposicion no invalida el
//! indice. Un offset absoluto habria que reescribirlo, y un indice que hay que
//! reescribir es un indice que un dia se queda sin reescribir.

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u32, bx_u64};
use alloc::vec::Vec;

/// `"BRES"` en little-endian. Va **dentro** de la seccion, no en el header del
/// fichero: el `.bex` sigue siendo un `.bex` y el magic de BEF no cambia.
pub const RECURSOS_MAGIC: bx_u32 = u32::from_le_bytes(*b"BRES");

/// Bytes de la cabecera del directorio.
pub const CABECERA_LEN: usize = 16;
/// Bytes de cada entrada.
pub const ENTRADA_LEN: usize = 64;
/// Caracteres de nombre que caben. `doom1.wad` son nueve.
pub const NOMBRE_MAX: usize = ENTRADA_LEN - 17;

/// Construye la seccion `Resources` a partir de los recursos dados.
///
/// El orden se conserva: quien empaqueta decide, y dos paquetes con la misma
/// lista dan **los mismos bytes** -- que es lo que hace que una firma sobre esta
/// seccion sea reproducible.
pub fn construir(recursos: &[(&str, &[u8])]) -> Result<Vec<u8>, &'static str> {
    let n = recursos.len();
    if n > u32::MAX as usize {
        return Err("demasiados recursos");
    }
    let inicio_datos = CABECERA_LEN + n * ENTRADA_LEN;
    let mut total = inicio_datos;
    for (nombre, datos) in recursos {
        if nombre.is_empty() {
            return Err("un recurso sin nombre no se puede pedir de vuelta");
        }
        if nombre.len() > NOMBRE_MAX {
            return Err("el nombre de un recurso no cabe en 47 bytes");
        }
        total += datos.len();
    }

    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&RECURSOS_MAGIC.to_le_bytes());
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 8]);

    let mut cursor = inicio_datos as u64;
    for (nombre, datos) in recursos {
        out.extend_from_slice(&cursor.to_le_bytes());
        out.extend_from_slice(&(datos.len() as u64).to_le_bytes());
        out.push(nombre.len() as u8);
        let mut relleno = [0u8; NOMBRE_MAX];
        relleno[..nombre.len()].copy_from_slice(nombre.as_bytes());
        out.extend_from_slice(&relleno);
        cursor += datos.len() as u64;
    }
    for (_, datos) in recursos {
        out.extend_from_slice(datos);
    }
    Ok(out)
}

/// Un directorio ya validado, sobre los bytes de la seccion.
///
/// No copia nada y no reserva nada: es una vista. Se puede construir en el
/// kernel, en una herramienta o en un test con el mismo codigo.
pub struct Directorio<'a> {
    seccion: &'a [u8],
    count: usize,
}

/// Lo que se sabe de un recurso sin haber leido sus datos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entrada {
    /// Desde el inicio de la seccion.
    pub offset: bx_u64,
    pub size: bx_u64,
}

impl<'a> Directorio<'a> {
    /// Valida la cabecera y la tabla. `None` si esto no es un directorio.
    ///
    /// [!] Se comprueba que **cada entrada cae dentro de la seccion**, aqui y
    /// no en quien lee. Estos bytes vienen de un fichero de fuera, y un offset
    /// que se sale es la forma normal en que un formato de paquete se convierte
    /// en una lectura arbitraria de memoria.
    pub fn nuevo(seccion: &'a [u8]) -> Option<Self> {
        if seccion.len() < CABECERA_LEN {
            return None;
        }
        let magic = u32::from_le_bytes(seccion[0..4].try_into().ok()?);
        if magic != RECURSOS_MAGIC {
            return None;
        }
        let count = u32::from_le_bytes(seccion[4..8].try_into().ok()?) as usize;
        let tabla_fin = CABECERA_LEN.checked_add(count.checked_mul(ENTRADA_LEN)?)?;
        if tabla_fin > seccion.len() {
            return None;
        }
        let d = Directorio { seccion, count };
        for i in 0..count {
            let e = d.entrada_cruda(i)?;
            let fin = (e.offset as usize).checked_add(e.size as usize)?;
            if fin > seccion.len() || (e.offset as usize) < tabla_fin {
                return None;
            }
        }
        Some(d)
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn entrada_cruda(&self, i: usize) -> Option<Entrada> {
        if i >= self.count {
            return None;
        }
        let base = CABECERA_LEN + i * ENTRADA_LEN;
        let offset = u64::from_le_bytes(self.seccion[base..base + 8].try_into().ok()?);
        let size = u64::from_le_bytes(self.seccion[base + 8..base + 16].try_into().ok()?);
        Some(Entrada { offset, size })
    }

    /// El nombre del recurso `i`. `None` si el largo declarado no cabe -- un
    /// nombre de 200 bytes en un campo de 47 es un fichero corrupto, no un
    /// nombre largo.
    pub fn nombre(&self, i: usize) -> Option<&'a str> {
        if i >= self.count {
            return None;
        }
        let base = CABECERA_LEN + i * ENTRADA_LEN;
        let largo = self.seccion[base + 16] as usize;
        if largo == 0 || largo > NOMBRE_MAX {
            return None;
        }
        core::str::from_utf8(&self.seccion[base + 17..base + 17 + largo]).ok()
    }

    /// Donde y cuanto mide el recurso `i`, sin leerlo.
    ///
    /// Es lo que necesita quien va a hacer un `fseek` + `fread`: **no hace
    /// falta tener el paquete entero en memoria para saber donde esta un
    /// recurso**, que es todo el motivo por el que hay un indice.
    pub fn entrada(&self, i: usize) -> Option<Entrada> {
        self.entrada_cruda(i)
    }

    /// Los bytes del recurso `i`, si la seccion entera esta a mano.
    pub fn datos(&self, i: usize) -> Option<&'a [u8]> {
        let e = self.entrada_cruda(i)?;
        let ini = e.offset as usize;
        Some(&self.seccion[ini..ini + e.size as usize])
    }

    /// Busca por nombre. Devuelve el indice.
    ///
    /// Recorrido lineal y sin ordenar: un paquete tiene unidades de recursos,
    /// no miles. Una tabla hash aqui seria mas codigo, mas formato que validar,
    /// y ni un microsegundo que nadie note.
    pub fn buscar(&self, nombre: &str) -> Option<usize> {
        (0..self.count).find(|&i| self.nombre(i) == Some(nombre))
    }
}

/// Cabe este nombre en una entrada?
pub fn nombre_valido(nombre: &str) -> bool {
    !nombre.is_empty() && nombre.len() <= NOMBRE_MAX
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn ida_y_vuelta() {
        let wad = vec![0xAAu8; 300];
        let cfg = b"volumen=8\n".to_vec();
        let bytes = construir(&[("doom1.wad", &wad), ("default.cfg", &cfg)]).unwrap();

        let d = Directorio::nuevo(&bytes).expect("es un directorio");
        assert_eq!(d.len(), 2);
        assert_eq!(d.nombre(0), Some("doom1.wad"));
        assert_eq!(d.nombre(1), Some("default.cfg"));
        assert_eq!(d.datos(0).unwrap(), &wad[..]);
        assert_eq!(d.datos(1).unwrap(), &cfg[..]);
        assert_eq!(d.buscar("default.cfg"), Some(1));
        assert_eq!(d.buscar("no-esta"), None);
    }

    /// ** Los offsets son RELATIVOS A LA SECCION, y esta fila es la que lo
    /// fija. Si alguien los hiciera absolutos al fichero, el indice dejaria de
    /// valer en cuanto el escritor de BEF colocara la seccion en otro sitio --
    /// y eso pasa con solo anadir una seccion delante.
    #[test]
    fn los_offsets_no_dependen_de_donde_caiga_la_seccion() {
        let a = construir(&[("x", b"12345")]).unwrap();
        let e = Directorio::nuevo(&a).unwrap().entrada(0).unwrap();
        assert_eq!(e.offset, (CABECERA_LEN + ENTRADA_LEN) as u64);
        assert_eq!(e.size, 5);
    }

    /// Dos paquetes con la misma lista dan los MISMOS bytes. Sin eso, una firma
    /// sobre esta seccion no seria reproducible y "el paquete cambio" dejaria
    /// de significar nada.
    #[test]
    fn construir_es_determinista() {
        let a = construir(&[("a", b"uno"), ("b", b"dos")]).unwrap();
        let b = construir(&[("a", b"uno"), ("b", b"dos")]).unwrap();
        assert_eq!(a, b);
    }

    /// [!] Un offset que se sale de la seccion es la forma normal en que un
    /// formato de paquete se convierte en una lectura de memoria arbitraria.
    /// Se rechaza el DIRECTORIO ENTERO, no la entrada: si una miente, ninguna
    /// merece credito.
    #[test]
    fn un_offset_fuera_de_la_seccion_invalida_el_directorio() {
        let mut bytes = construir(&[("x", b"12345")]).unwrap();
        bytes[CABECERA_LEN..CABECERA_LEN + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(Directorio::nuevo(&bytes).is_none());
    }

    /// Y un tamano que desborda al sumarlo. `offset + size` con los dos cerca
    /// del tope da la vuelta, y una comprobacion escrita como
    /// `offset + size > len` diria que si cabe.
    #[test]
    fn un_tamano_que_desborda_no_pasa_la_suma() {
        let mut bytes = construir(&[("x", b"12345")]).unwrap();
        bytes[CABECERA_LEN + 8..CABECERA_LEN + 16].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(Directorio::nuevo(&bytes).is_none());
    }

    /// Un recurso NO puede empezar dentro de la tabla: seria un recurso que se
    /// solapa con el indice que lo describe.
    #[test]
    fn un_recurso_no_puede_pisar_la_tabla() {
        let mut bytes = construir(&[("x", b"12345")]).unwrap();
        bytes[CABECERA_LEN..CABECERA_LEN + 8].copy_from_slice(&0u64.to_le_bytes());
        assert!(Directorio::nuevo(&bytes).is_none());
    }

    #[test]
    fn lo_que_no_es_un_directorio_se_dice() {
        assert!(Directorio::nuevo(b"").is_none());
        assert!(Directorio::nuevo(b"no soy un indice de recursos").is_none());
    }

    /// El nombre no cabe -> error al construir, no un nombre recortado. Un
    /// recurso con el nombre a medias es un recurso que nadie encuentra.
    #[test]
    fn un_nombre_demasiado_largo_se_rechaza_al_construir() {
        let largo = "a".repeat(NOMBRE_MAX + 1);
        assert!(construir(&[(largo.as_str(), b"x")]).is_err());
        let justo = "a".repeat(NOMBRE_MAX);
        assert!(construir(&[(justo.as_str(), b"x")]).is_ok());
    }

    /// Un paquete sin recursos es valido y vale cero: es lo que tiene todo
    /// `.bex` de hoy, y el lector tiene que contestar "no hay" y no reventar.
    #[test]
    fn un_directorio_vacio_es_valido() {
        let bytes = construir(&[]).unwrap();
        let d = Directorio::nuevo(&bytes).unwrap();
        assert_eq!(d.len(), 0);
        assert!(d.is_empty());
        assert_eq!(d.buscar("lo que sea"), None);
    }
}
