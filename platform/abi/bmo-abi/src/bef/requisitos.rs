//! **LO QUE UN PROGRAMA REQUIERE, Y EL PORQUE** -- la seccion `0x15`.
//!
//! ## El problema, dicho en una linea
//!
//! Hoy el kernel **deduce** lo que un programa necesita: `bex::necesita` le
//! recorre la tabla de secciones, suma offsets y decide cuanta memoria hay que
//! traer. Eso es **un cerebro en Ring 0** para contestar una pregunta que el
//! fichero deberia traer contestada.
//!
//! Y la deduccion falla exactamente donde importa. `MAX_BEX` ha subido dos
//! veces --1 MiB, 4 MiB-- porque un programa media mas de lo que el kernel
//! habia supuesto de antemano, y el sintoma cada vez fue el mismo: *"no paso la
//! admision"*, un numero, y a mirar el formato cuando lo que fallaba era el
//! supuesto.
//!
//! > **El kernel no tiene que saber a donde quiere ir un programa. Se lo tiene
//! > que decir el programa.**
//!
//! ## Y el PORQUE viaja con el requisito, no aparte
//!
//! Un requisito sin motivo produce un rechazo que no se puede contestar:
//!
//! ```text
//!   run apps/doom.bex
//!   no paso la admision  =3
//! ```
//!
//! Con motivo, el "no" trae el renglon que escribio quien hizo el programa, y
//! quien lo lee sabe si el problema es suyo o de la maquina:
//!
//! ```text
//!   run apps/doom.bex
//!   no: pide 6,3 MB de recursos residentes
//!       "el WAD se lee a demanda, pero la tabla de niveles vive en RAM
//!        mientras el juego corre"
//! ```
//!
//! Por eso el motivo es un campo del formato y no un comentario del manifiesto:
//! **lo que no viaja dentro del fichero no esta ahi el dia del fallo.**
//!
//! ## Esto NO es formato nuevo, es un hueco que estaba vacio
//!
//! Es la tercera vez que pasa lo mismo. `Resources = 0x0B` llevaba meses
//! declarada y sin escribir; `Manifest = 0x09` sigue declarada, sin escribir y
//! sin leer. La diferencia de esta es que **se puede leer sin parser**: el
//! manifiesto es TOML, y meter un lector de texto en el anillo cero seria sacar
//! un cerebro para meter otro.
//!
//! El TOML no muere -- se queda en `Manifest 0x09` para humanos y para el
//! toolchain, y quien empaqueta lo compila a esta tabla. **Dos vistas del mismo
//! hecho, una sola fuente de verdad, y Ring 0 lee la compilada.**
//!
//! ## La disposicion
//!
//! ```text
//!   cabecera (16 B)
//!     0..4    magic "BREQ"
//!     4..8    cuantos requisitos
//!     8..12   donde empiezan los motivos -- DESDE EL INICIO DE ESTA SECCION
//!     12..16  cuanto miden los motivos, todos juntos
//!
//!   requisito (32 B cada uno, `cuantos` seguidos)
//!     0..2    clase     -- que se pide
//!     2..4    unidad    -- en que se mide `cantidad`
//!     4..8    banderas  -- bit 0: OBLIGATORIO
//!     8..16   cantidad
//!     16..20  motivo: offset DENTRO del blob de motivos
//!     20..22  motivo: cuantos bytes
//!     22..32  cero (reservado)
//!
//!   el blob de motivos: ASCII, sin terminadores, uno detras de otro
//! ```
//!
//! **Registros de tamano fijo**, igual que el directorio de recursos y por el
//! mismo motivo: el requisito `i` esta en `16 + i*32` y el lector es una
//! multiplicacion. Se lee desde Rust, desde el kernel sin `alloc`, y desde C
//! con veinte lineas.
//!
//! Los offsets son **relativos a la seccion**. Quien empaqueta no tiene que
//! saber en que byte va a acabar su seccion, y reemitir el `.bex` con otra
//! disposicion no invalida la tabla.
//!
//! ## [!] LA REGLA QUE HACE QUE ESTO PUEDA CRECER
//!
//! Una **clase desconocida** con `OBLIGATORIO` puesto **se rechaza**. Sin
//! `OBLIGATORIO`, se ignora y se cuenta.
//!
//! Es la misma regla que ya mantiene vivo el formato entero --un tipo de seccion
//! que no me incumbe no es un error, es data que no voy a abrir-- pero con el
//! sentido invertido en el caso que importa: si un programa dice *"necesito X o
//! no funciono"* y este sistema no sabe que es X, **la respuesta correcta es no
//! arrancarlo**. Un sistema viejo que ejecuta un programa nuevo ignorando lo que
//! no entiende es un sistema que arranca y falla despues, lejos de la causa.
//!
//! Asi un BMO-X de dentro de un ano puede pedir cosas que este no conoce, y este
//! contesta que no **con el renglon del programa en la mano** en vez de
//! arrancarlo a medias.

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u16, bx_u32, bx_u64};
use alloc::vec::Vec;

/// `"BREQ"` en little-endian. Va **dentro** de la seccion: el magic del fichero
/// no cambia y un `.bex` con requisitos sigue siendo un `.bex`.
pub const REQUISITOS_MAGIC: bx_u32 = u32::from_le_bytes(*b"BREQ");

/// Bytes de la cabecera de la tabla.
pub const CABECERA_LEN: usize = 16;
/// Bytes de cada requisito.
pub const REQUISITO_LEN: usize = 32;
/// Lo que puede medir un motivo. No es una limitacion tecnica: un motivo que no
/// cabe en dos renglones de consola es un motivo que nadie va a leer el dia que
/// salga por pantalla.
pub const MOTIVO_MAX: usize = 160;

// -- Las clases -------------------------------------------------------------
//
// Numeros y no un enum abierto: esto es contrato de fichero. Una clase que se
// retira NO se reutiliza -- el numero se queda quemado, igual que el `1` de
// CHANNEL_KICK.

/// Memoria del proceso que tiene que existir **antes** de la primera
/// instruccion: codigo, datos, pila. En bytes.
pub const CLASE_MEMORIA: bx_u16 = 0x0001;
/// Recursos que el programa quiere RESIDENTES en RAM mientras corre. En bytes.
/// Lo que se lee a demanda por su puerta **no se declara aqui**: eso vive en el
/// disco y no le cuesta RAM a nadie.
pub const CLASE_RECURSOS: bx_u16 = 0x0002;
/// La pantalla, en exclusiva. `cantidad` = 1.
pub const CLASE_PANTALLA: bx_u16 = 0x0003;
/// El aparato de audio. `cantidad` = 1.
pub const CLASE_AUDIO: bx_u16 = 0x0004;
/// Teclado y raton. `cantidad` = 1.
pub const CLASE_ENTRADA: bx_u16 = 0x0005;
/// Extensiones de CPU cuyo estado el sistema tiene que saber preservar en un
/// cambio de contexto. `cantidad` = mascara de bits (la misma de la cabecera).
pub const CLASE_CPU: bx_u16 = 0x0006;
/// Huecos de proceso: un programa que lanza hijos y necesita que quepan.
pub const CLASE_PROCESOS: bx_u16 = 0x0007;
/// **El MONTON de la tarea: lo que el programa reparte en ejecucion.** En bytes.
///
/// *** Y NO ES [`CLASE_MEMORIA`], aunque las dos se midan en bytes. La de
/// arriba es lo que tiene que existir **antes de la primera instruccion**
/// --codigo, datos, pila-- y la decide el CARGADOR mirando el fichero. Esta es
/// lo que la tarea va a pedirle al sistema **despues de arrancar**, y solo la
/// sabe el programa.
///
/// ** Se anade una clase en vez de sumar las dos cantidades porque un sistema
/// que no pueda dar el monton puede querer cargar el programa igual --y dejar
/// que muera con su codigo-- mientras que no poder dar la pila es no poder
/// cargarlo. Son dos decisiones distintas y necesitan dos numeros distintos.
pub const CLASE_MONTON: bx_u16 = 0x0008;

// -- Las unidades -----------------------------------------------------------

/// `cantidad` se cuenta a secas (1 pantalla, 3 procesos).
pub const UNIDAD_UNIDADES: bx_u16 = 0;
/// `cantidad` son bytes.
pub const UNIDAD_BYTES: bx_u16 = 1;
/// `cantidad` es una mascara de bits.
pub const UNIDAD_MASCARA: bx_u16 = 2;

// -- Las banderas -----------------------------------------------------------

/// **Sin esto no funciono.** Si el sistema no puede concederlo --o no sabe lo
/// que es-- el programa no arranca. Ver la regla de arriba.
pub const OBLIGATORIO: bx_u32 = 1 << 0;

/// Un requisito, ya leido. Es una copia: 32 bytes, no compensa prestarlos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requisito {
    pub clase: bx_u16,
    pub unidad: bx_u16,
    pub banderas: bx_u32,
    pub cantidad: bx_u64,
    /// Offset del motivo dentro del blob, y su largo. Se guardan crudos para
    /// que leer un requisito no obligue a validar su texto: quien quiera el
    /// motivo lo pide con [`Tabla::motivo`], y quien solo quiera decidir no
    /// paga por una cadena que no va a mirar.
    pub motivo_off: bx_u32,
    pub motivo_len: bx_u16,
}

impl Requisito {
    /// Sin esto, no arranca?
    pub fn es_obligatorio(&self) -> bool {
        self.banderas & OBLIGATORIO != 0
    }
}

/// **La tabla, leida sobre los bytes de la seccion. Cero copias, cero `alloc`.**
///
/// Esta es la mitad que corre en Ring 0, y por eso no reserva nada y no falla a
/// medias: o la cabecera cuadra y hay tabla, o no hay tabla.
pub struct Tabla<'a> {
    bytes: &'a [u8],
    cuantos: usize,
    motivos_off: usize,
    motivos_len: usize,
}

impl<'a> Tabla<'a> {
    /// Abre la tabla sobre los bytes de la seccion `Requisitos`.
    ///
    /// `None` si esto no es una tabla: magic malo, o una cabecera que promete
    /// mas registros de los que hay bytes. **No se confia en `cuantos`**: es un
    /// numero que viene del disco, y el disco es de quien tenga la maquina.
    pub fn abrir(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < CABECERA_LEN {
            return None;
        }
        if leer_u32(bytes, 0)? != REQUISITOS_MAGIC {
            return None;
        }
        let cuantos = leer_u32(bytes, 4)? as usize;
        let motivos_off = leer_u32(bytes, 8)? as usize;
        let motivos_len = leer_u32(bytes, 12)? as usize;

        let fin_registros = CABECERA_LEN.checked_add(cuantos.checked_mul(REQUISITO_LEN)?)?;
        if fin_registros > bytes.len() {
            return None;
        }
        // El blob puede estar vacio (`motivos_len == 0`); lo que no puede es
        // salirse. Un offset que se sale convierte cada `motivo()` en una
        // comprobacion mas, y esa comprobacion es justo la que un dia falta.
        let fin_motivos = motivos_off.checked_add(motivos_len)?;
        if motivos_len > 0 && (motivos_off < fin_registros || fin_motivos > bytes.len()) {
            return None;
        }
        Some(Self { bytes, cuantos, motivos_off, motivos_len })
    }

    /// Cuantos requisitos declara el programa.
    pub fn cuantos(&self) -> usize {
        self.cuantos
    }

    /// El requisito `i`. `None` si se pide uno que no existe.
    pub fn requisito(&self, i: usize) -> Option<Requisito> {
        if i >= self.cuantos {
            return None;
        }
        let e = CABECERA_LEN + i * REQUISITO_LEN;
        Some(Requisito {
            clase: leer_u16(self.bytes, e)?,
            unidad: leer_u16(self.bytes, e + 2)?,
            banderas: leer_u32(self.bytes, e + 4)?,
            cantidad: leer_u64(self.bytes, e + 8)?,
            motivo_off: leer_u32(self.bytes, e + 16)?,
            motivo_len: leer_u16(self.bytes, e + 20)?,
        })
    }

    /// **El renglon que escribio quien hizo el programa.**
    ///
    /// `""` si no lo trae o si no cuadra. Un motivo ilegible no invalida el
    /// requisito: la decision se toma con `clase` y `cantidad`, que son numeros.
    /// Lo que se pierde es la explicacion, y perder la explicacion no puede
    /// costar el arranque.
    pub fn motivo(&self, r: &Requisito) -> &'a str {
        let n = r.motivo_len as usize;
        if n == 0 || n > MOTIVO_MAX {
            return "";
        }
        let ini = match self.motivos_off.checked_add(r.motivo_off as usize) {
            Some(v) => v,
            None => return "",
        };
        let fin = match ini.checked_add(n) {
            Some(v) => v,
            None => return "",
        };
        if r.motivo_off as usize + n > self.motivos_len || fin > self.bytes.len() {
            return "";
        }
        core::str::from_utf8(&self.bytes[ini..fin]).unwrap_or("")
    }

    /// Recorre los requisitos en el orden en que los escribio el programa.
    pub fn iter(&self) -> impl Iterator<Item = Requisito> + '_ {
        (0..self.cuantos).filter_map(move |i| self.requisito(i))
    }

    /// **Cuanto pide de una clase.** Suma, porque un programa puede declarar la
    /// misma clase dos veces con motivos distintos -- y esa es una funcion util,
    /// no un error que haya que rechazar: *"2 MB para el mapa"* y *"1 MB para
    /// los sonidos"* dicen mas que *"3 MB"*.
    pub fn total_de(&self, clase: bx_u16) -> bx_u64 {
        self.iter()
            .filter(|r| r.clase == clase)
            .fold(0u64, |a, r| a.saturating_add(r.cantidad))
    }
}

/// Que clases entiende ESTE sistema. Lo que no este aqui es desconocido, y
/// entonces manda [`OBLIGATORIO`].
pub fn clase_conocida(clase: bx_u16) -> bool {
    matches!(
        clase,
        CLASE_MEMORIA
            | CLASE_RECURSOS
            | CLASE_PANTALLA
            | CLASE_AUDIO
            | CLASE_ENTRADA
            | CLASE_CPU
            | CLASE_PROCESOS
            | CLASE_MONTON
    )
}

/// Lo que hay que escribir para declarar un requisito. Es la vista del que
/// empaqueta; la del que carga es [`Requisito`].
pub struct Declaracion<'a> {
    pub clase: bx_u16,
    pub unidad: bx_u16,
    pub obligatorio: bool,
    pub cantidad: bx_u64,
    pub motivo: &'a str,
}

/// **Construye la seccion `Requisitos`.** Es la mitad del toolchain.
///
/// El orden se conserva: dos paquetes con la misma lista dan **los mismos
/// bytes**, que es lo que hace que la firma sobre esta seccion sea
/// reproducible.
pub fn construir(decls: &[Declaracion]) -> Result<Vec<u8>, &'static str> {
    let n = decls.len();
    if n > u32::MAX as usize {
        return Err("demasiados requisitos");
    }
    for d in decls {
        if d.motivo.len() > MOTIVO_MAX {
            return Err("el motivo de un requisito no cabe en 160 bytes");
        }
        if d.obligatorio && d.motivo.is_empty() {
            // Un requisito que puede tumbar el arranque **tiene** que decir por
            // que. Si no, se vuelve al rechazo que no se puede contestar, que es
            // el problema que esta seccion existe para matar.
            return Err("un requisito obligatorio sin motivo no se puede contestar");
        }
        if !d.motivo.is_ascii() {
            // La misma frontera que el resto del sistema: los ficheros en ASCII.
            return Err("el motivo tiene que ser ASCII");
        }
    }

    let motivos_off = CABECERA_LEN + n * REQUISITO_LEN;
    let motivos_len: usize = decls.iter().map(|d| d.motivo.len()).sum();

    let mut out = Vec::with_capacity(motivos_off + motivos_len);
    out.extend_from_slice(&REQUISITOS_MAGIC.to_le_bytes());
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out.extend_from_slice(&(motivos_off as u32).to_le_bytes());
    out.extend_from_slice(&(motivos_len as u32).to_le_bytes());

    let mut cursor = 0u32;
    for d in decls {
        let banderas = if d.obligatorio { OBLIGATORIO } else { 0 };
        out.extend_from_slice(&d.clase.to_le_bytes());
        out.extend_from_slice(&d.unidad.to_le_bytes());
        out.extend_from_slice(&banderas.to_le_bytes());
        out.extend_from_slice(&d.cantidad.to_le_bytes());
        out.extend_from_slice(&cursor.to_le_bytes());
        out.extend_from_slice(&(d.motivo.len() as u16).to_le_bytes());
        out.extend_from_slice(&[0u8; 10]);
        cursor += d.motivo.len() as u32;
    }
    for d in decls {
        out.extend_from_slice(d.motivo.as_bytes());
    }
    Ok(out)
}

// -- Lectores acotados -------------------------------------------------------
//
// Los mismos cuatro de siempre y por el mismo motivo: los bytes vienen del
// disco, asi que **nada se indexa sin comprobar**. Un `bytes[o+3]` en un lector
// de formato es un `panic` en Ring 0 esperando a un fichero mal escrito.

fn leer_u16(b: &[u8], o: usize) -> Option<bx_u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
fn leer_u32(b: &[u8], o: usize) -> Option<bx_u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn leer_u64(b: &[u8], o: usize) -> Option<bx_u64> {
    Some(u64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ejemplo() -> Vec<u8> {
        construir(&[
            Declaracion {
                clase: CLASE_MEMORIA,
                unidad: UNIDAD_BYTES,
                obligatorio: true,
                cantidad: 812_000,
                motivo: "codigo y datos del interprete",
            },
            Declaracion {
                clase: CLASE_RECURSOS,
                unidad: UNIDAD_BYTES,
                obligatorio: true,
                cantidad: 6_300_000,
                motivo: "la tabla de niveles vive en RAM mientras el juego corre",
            },
            Declaracion {
                clase: CLASE_PANTALLA,
                unidad: UNIDAD_UNIDADES,
                obligatorio: false,
                cantidad: 1,
                motivo: "",
            },
        ])
        .expect("debe construir")
    }

    #[test]
    fn ida_y_vuelta() {
        let bytes = ejemplo();
        let t = Tabla::abrir(&bytes).expect("debe abrir");
        assert_eq!(t.cuantos(), 3);

        let r0 = t.requisito(0).unwrap();
        assert_eq!(r0.clase, CLASE_MEMORIA);
        assert_eq!(r0.cantidad, 812_000);
        assert!(r0.es_obligatorio());
        assert_eq!(t.motivo(&r0), "codigo y datos del interprete");

        let r1 = t.requisito(1).unwrap();
        assert_eq!(
            t.motivo(&r1),
            "la tabla de niveles vive en RAM mientras el juego corre"
        );

        let r2 = t.requisito(2).unwrap();
        assert!(!r2.es_obligatorio());
        assert_eq!(t.motivo(&r2), "", "sin motivo declarado, cadena vacia");
        assert!(t.requisito(3).is_none(), "no hay un cuarto");
    }

    /// ** Los mismos requisitos tienen que dar LOS MISMOS BYTES.
    ///
    /// Es lo que hace que la firma de esta seccion sea reproducible. Si un dia
    /// se cuela un `HashMap` en `construir`, esto lo caza.
    #[test]
    fn es_reproducible() {
        assert_eq!(ejemplo(), ejemplo());
    }

    #[test]
    fn un_obligatorio_sin_motivo_no_se_escribe() {
        let r = construir(&[Declaracion {
            clase: CLASE_MEMORIA,
            unidad: UNIDAD_BYTES,
            obligatorio: true,
            cantidad: 1,
            motivo: "",
        }]);
        assert!(r.is_err(), "un no que no se puede contestar no se emite");
    }

    /// ** UNA CABECERA QUE MIENTE NO PUEDE HACER QUE EL LECTOR SE SALGA.
    ///
    /// Es el caso que de verdad importa: estos bytes vienen del disco, y el
    /// disco es de quien tenga la maquina. Se trucan los tres numeros de la
    /// cabecera, uno a uno, y ninguno puede acabar en un indexado fuera.
    #[test]
    fn una_cabecera_que_miente_no_abre() {
        let bueno = ejemplo();

        let mut muchos = bueno.clone();
        muchos[4..8].copy_from_slice(&9999u32.to_le_bytes());
        assert!(Tabla::abrir(&muchos).is_none(), "cuantos fuera de rango");

        let mut lejos = bueno.clone();
        lejos[8..12].copy_from_slice(&0xFFFF_0000u32.to_le_bytes());
        assert!(Tabla::abrir(&lejos).is_none(), "motivos fuera del fichero");

        let mut largo = bueno.clone();
        largo[12..16].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert!(Tabla::abrir(&largo).is_none(), "blob mas largo que la seccion");

        let mut magia = bueno.clone();
        magia[0] = b'X';
        assert!(Tabla::abrir(&magia).is_none(), "magic malo");

        assert!(Tabla::abrir(&bueno[..8]).is_none(), "ni la cabecera cabe");
    }

    /// Un motivo que apunta fuera del blob devuelve cadena vacia, **no un
    /// panic** y **no bytes de otro sitio**.
    #[test]
    fn un_motivo_que_miente_sale_vacio() {
        let mut bytes = ejemplo();
        // El motivo del requisito 0 vive en el offset 16 de su registro.
        let e = CABECERA_LEN + 16;
        bytes[e..e + 4].copy_from_slice(&0xFFFFu32.to_le_bytes());
        let t = Tabla::abrir(&bytes).expect("la tabla sigue siendo valida");
        let r0 = t.requisito(0).unwrap();
        assert_eq!(t.motivo(&r0), "");
        assert_eq!(r0.cantidad, 812_000, "y el requisito se sigue pudiendo decidir");
    }

    #[test]
    fn suma_por_clase() {
        let bytes = construir(&[
            Declaracion { clase: CLASE_RECURSOS, unidad: UNIDAD_BYTES, obligatorio: false, cantidad: 2_000_000, motivo: "el mapa" },
            Declaracion { clase: CLASE_RECURSOS, unidad: UNIDAD_BYTES, obligatorio: false, cantidad: 1_000_000, motivo: "los sonidos" },
        ]).unwrap();
        let t = Tabla::abrir(&bytes).unwrap();
        assert_eq!(t.total_de(CLASE_RECURSOS), 3_000_000);
        assert_eq!(t.total_de(CLASE_AUDIO), 0);
    }

    #[test]
    fn una_tabla_vacia_es_valida() {
        let bytes = construir(&[]).unwrap();
        let t = Tabla::abrir(&bytes).expect("cero requisitos es una respuesta");
        assert_eq!(t.cuantos(), 0);
    }

    /// La clase desconocida NO se decide aqui -- aqui solo se dice si se
    /// conoce. Quien decide es el cargador, con [`OBLIGATORIO`] en la mano.
    #[test]
    fn lo_desconocido_se_reconoce_como_desconocido() {
        assert!(clase_conocida(CLASE_MEMORIA));
        assert!(!clase_conocida(0x7777));
    }
}
