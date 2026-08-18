//! **Lo que se le pide AL DISCO.** Los `DISCO_OP_*` y sus motivos.
//!
//! Es la sexta familia del contrato, y nace con fichero propio por lo que es y
//! no por lo que mide: **es la unica que ACTUA sobre el almacen**. `informe`
//! contesta preguntas, `entrada` cuenta hechos fisicos, `objetos` opera sobre
//! handles que alguien concedio... y esto **le da ordenes al aparato donde vive
//! el trabajo del dueno**.
//!
//! # Por que hay motivos y no un booleano
//!
//! Porque un `0` obligaria a adivinar cual de las cinco puertas dijo que no, y
//! son cinco conversaciones distintas: *"este disco no sabe"* es una propiedad
//! del aparato, *"no esta armado"* es un estado que se puede ganar, *"fuera de
//! la ventana"* es un bug del que llama, y *"el disco fallo"* es hardware.
//!
//! La respuesta viaja empaquetada porque por la puerta cabe **un** numero:
//!
//! ```text
//!   (motivo << 56) | sectores
//!
//!   motivo 0 = HECHO, y entonces `sectores` es lo que se recorto de verdad
//!   motivo > 0 = no se hizo (o se hizo a medias, ver DISCO_TRIM_FALLO)
//! ```

/// **Devolverle al disco la cola libre del volumen ESTRATOS.**
///
/// Sin argumentos: **el rango no lo elige quien llama**. Lo calcula el kernel a
/// partir de `log_head` --el puntero que solo avanza-- y lo comprueba contra la
/// ventana de escritura. Un TRIM con LBA a gusto del llamante seria una orden de
/// borrado apuntable a cualquier sector desde Ring 3, y eso no es una operacion:
/// es un agujero.
pub const DISCO_OP_TRIM_LIBRE: u64 = 0x01;

/// **`FLUSH CACHE` a mano.** Devuelve 1 si el disco confirmo.
///
/// Existe porque este disco declara `SOLO_BARRERA`: no tiene condensadores, asi
/// que la barrera es lo unico que separa "el disco se quedo los bytes" de "los
/// bytes sobrevivirian a un corte". Poder pedirla desde donde se trabaja es lo
/// que hace comprobable esa frase.
pub const DISCO_OP_BARRERA: u64 = 0x02;

// -- Los motivos, en el byte alto de la respuesta ---------------------------

/// Se hizo. `sectores` dice cuantos.
pub const DISCO_TRIM_HECHO: u64 = 0;
/// No hay disco listo.
pub const DISCO_TRIM_SIN_DISCO: u64 = 1;
/// El disco **no declara TRIM** (palabra 169). No se manda a ver si suena.
pub const DISCO_TRIM_NO_SOPORTADO: u64 = 2;
/// El gate de identidad o la ventana de escritura dijeron que no.
pub const DISCO_TRIM_SIN_PERMISO: u64 = 3;
/// No hay volumen ESTRATOS montado, o su cola libre esta vacia.
pub const DISCO_TRIM_SIN_VOLUMEN: u64 = 4;
/// El rango no es representable: cero sectores, o fuera de LBA48.
pub const DISCO_TRIM_RANGO: u64 = 5;
/// El disco rechazo la orden. **`sectores` lleva lo que SI se recorto** antes
/// de romperse: un recorte a medias no se deshace, y callarlo haria que el
/// sistema volviera a mandar lo que ya estaba hecho.
pub const DISCO_TRIM_FALLO: u64 = 6;

/// Desplazamiento del motivo dentro de la respuesta.
pub const DISCO_TRIM_MOTIVO_SHIFT: u64 = 56;
/// Mascara de los sectores.
pub const DISCO_TRIM_SECTORES_MASK: u64 = (1 << 56) - 1;

// -- ** POR QUE FALLO, cuando el motivo es `DISCO_TRIM_FALLO` ---------------
//
// `DISCO_TRIM_FALLO` dice *que el disco no acepto la orden*; estas clases dicen
// **cual de las cinco maneras**, y viajan en `INFO_DISCO_TRIM_FALLO` junto al
// `PxTFD` crudo: `(clase << 32) | tfd`.
//
// === Por que hizo falta, y se pago en metal ===
//
// El primer recorte en el Ryzen (2026-08-17) contesto *"el disco RECHAZO la
// orden"* y ahi se acabo la informacion. El driver distingue las cinco desde
// siempre, pero su `name()` las aplana en una frase y el `tfd` --el registro
// donde el aparato dice por que-- no salia del `enum`.
//
// ** Y las cinco mandan a mirar sitios distintos: `SIN_TIEMPO` acusa al
// presupuesto de espera del driver, `APARATO` acusa al disco, y `PETICION`
// acusa al que armo el payload. Llamarlas a las tres "rechazo" es perder la
// unica pista que hay.

pub const DISCO_FALLO_NINGUNO: u64 = 0;
/// El puerto no estaba preparado.
pub const DISCO_FALLO_NO_LISTO: u64 = 1;
/// El disco no solto BSY/DRQ: no se le pudo ni dar la orden.
pub const DISCO_FALLO_OCUPADO: u64 = 2;
/// **No termino dentro del limite.** No es que dijera que no: es que no
/// contesto -- y el sospechoso es el presupuesto de espera, no el aparato.
pub const DISCO_FALLO_SIN_TIEMPO: u64 = 3;
/// **El disco contesto con error.** El `PxTFD` de los bits bajos dice cual:
/// `0x01` ERR, y en el byte alto el registro de error -- `0x04` ABRT (no
/// conozco esa orden), `0x10` IDNF (ese sector no), `0x40` UNC.
pub const DISCO_FALLO_APARATO: u64 = 4;
/// La peticion era imposible antes de salir: cero bloques, o mas de lo que cabe.
pub const DISCO_FALLO_PETICION: u64 = 5;

/// Desplazamiento de la clase dentro de `INFO_DISCO_TRIM_FALLO`.
pub const DISCO_FALLO_CLASE_SHIFT: u64 = 32;
/// Mascara del `PxTFD` crudo.
pub const DISCO_FALLO_TFD_MASK: u64 = 0xFFFF_FFFF;
