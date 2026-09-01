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
// ** EL LECTOR VIVE EN `bmo-carga-juicio` DESDE EL 2026-08-31, y aqui solo se
// reexporta. El motivo, entero, esta en aquel fichero: esta crate usa `alloc`
// y el kernel no puede depender de ella, asi que tener el lector aqui obligaba
// a copiarlo a la parte sin `alloc` -- y una tercera copia del mismo formato es
// como nacen las divergencias que el contrato lleva meses tolerando.
//
// Los `pub use` mantienen todas las rutas de antes: nada de fuera se entera.
pub use bmo_carga_juicio::{
    clase_conocida, Requisito, Tabla, CABECERA_LEN, CLASE_AUDIO, CLASE_CPU, CLASE_ENTRADA,
    CLASE_MEMORIA, CLASE_MONTON, CLASE_PANTALLA, CLASE_PROCESOS, CLASE_RECURSOS, MOTIVO_MAX,
    OBLIGATORIO, REQUISITOS_MAGIC, REQUISITO_LEN, UNIDAD_BYTES, UNIDAD_MASCARA, UNIDAD_UNIDADES,
};
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

// Los lectores acotados se fueron con la tabla a `bmo-carga-juicio`.

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
