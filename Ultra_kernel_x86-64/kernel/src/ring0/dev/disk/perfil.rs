//! **La foto de lo que el disco contesto, empaquetada para salir por la
//!
//! [carril]  VERDE     empaqueta lo que el disco contesto
//! puerta.**
//!
//! [eje]     CORRECCION -- se llama una vez en el arranque
//! [exige]   R-DISCO6 (el medio se pregunta), R-USB8 (una foto necesita saber
//!           de cuando es), L7 (aqui no se decide nada)
//!
//! # Que hace este fichero, y sobre todo que NO hace
//!
//! Recibe el sector del IDENTIFY una vez, se lo da a las cuatro generaciones
//! que viven fuera del kernel, y **guarda el resultado ya empaquetado**. Nada
//! mas.
//!
//! No interpreta una sola palabra: eso es `bmo-identify`. No decide nada: eso
//! es `bmo-disco-juicio`, que vive en `platform/shared/` porque alli se puede
//! probar con `cargo test`. Este fichero es **el pegamento**, y por eso es
//! corto -- si crece, es que alguien esta decidiendo aqui.
//!
//! ## ** Por que se guarda una FOTO y no se relee el disco al preguntar
//!
//! Dos razones, y las dos son de correccion, no de velocidad:
//!
//!   1. Un `OP_INFO` llega con el **CR3 del programa que pregunta**. Hablarle
//!      al disco desde ahi es el mismo `#PF` que el USB ya pago.
//!   2. ** El driver tiene **una sola ranura de comando** y con dueno. Mandar un
//!      IDENTIFY para contestar a `info` seria **robarle la ranura a quien
//!      estuviera leyendo un fichero**. Una pregunta no puede desalojar a un
//!      trabajo.
//!
//! [!] Y una foto no envejece sola, asi que aqui aplica lo que R-USB8 dejo
//! escrito para el USB. La diferencia es que **esta foto describe algo que no
//! cambia**: un disco no deja de ser un SSD mientras esta encendido. Lo unico
//! que podria cambiar --que el IDENTIFY fallara y la foto fuera de otro disco--
//! lo cierra `hay_foto`, que es falso hasta que un IDENTIFY sale bien.

use bmo_disco_juicio::Veredicto;
use bmo_identify::{Identify, LoQueDiceElDisco, Medio};

// -- EL CONTRATO, en su copia de Ring 0 --------------------------------------
//
// El kernel no enlaza `bmo-abi`: mantiene su propia copia de los numeros, y el
// guardian de `build.ps1` exige que las tres --kernel, ABI y userland-- digan lo
// mismo, mas la cuarta de `bmo.h`. Espejo de
// `bmo_abi::syscalls::surface::informe`, y el empaquetado se documenta alli.
//
// ** Un bit descuadrado aqui no falla: pinta el disco equivocado. Por eso lo
// barre el guardian y no la revision a ojo.

pub const DISCO_MEDIO_CRUDO_MASK: u64 = 0xFFFF;
pub const DISCO_MEDIO_CLASE_SHIFT: u64 = 16;
pub const DISCO_MEDIO_NO_CONTESTA: u64 = 0;
pub const DISCO_MEDIO_NO_ROTA: u64 = 1;
pub const DISCO_MEDIO_ROTA: u64 = 2;
pub const DISCO_MEDIO_RESERVADO: u64 = 3;
pub const DISCO_MEDIO_RPM_SHIFT: u64 = 32;
pub const DISCO_MEDIO_RPM_MASK: u64 = 0xFFFF;

pub const DISCO_ENLACE_NEGOCIADA_SHIFT: u64 = 4;
pub const DISCO_ENLACE_NEGOCIADA_MASK: u64 = 0x7;
pub const DISCO_ENLACE_NCQ: u64 = 1 << 8;
pub const DISCO_ENLACE_COLA_SHIFT: u64 = 16;
pub const DISCO_ENLACE_COLA_MASK: u64 = 0xFF;
pub const DISCO_ENLACE_USADAS_SHIFT: u64 = 24;
pub const DISCO_ENLACE_USADAS_MASK: u64 = 0xFF;
pub const DISCO_ENLACE_OCIOSAS_SHIFT: u64 = 32;
pub const DISCO_ENLACE_OCIOSAS_MASK: u64 = 0xFF;

pub const DISCO_GEO_EXP_MASK: u64 = 0xF;
pub const DISCO_GEO_106_VALIDA: u64 = 1 << 4;
pub const DISCO_GEO_DESPL_SHIFT: u64 = 8;
pub const DISCO_GEO_DESPL_MASK: u64 = 0x3FFF;
pub const DISCO_GEO_209_VALIDA: u64 = 1 << 22;
pub const DISCO_GEO_TRIM: u64 = 1 << 23;

pub const DISCO_JUICIO_HAY_PERFIL: u64 = 1 << 0;
pub const DISCO_JUICIO_SOLIDO: u64 = 1 << 1;
pub const DISCO_JUICIO_SOLO_BARRERA: u64 = 1 << 2;
pub const DISCO_JUICIO_TRIM: u64 = 1 << 3;
pub const DISCO_JUICIO_MEDIDO: u64 = 1 << 4;
pub const DISCO_JUICIO_SOLIDO_SIN_TRIM: u64 = 1 << 5;
pub const DISCO_JUICIO_DESALINEADO: u64 = 1 << 6;
pub const DISCO_JUICIO_ENLACE_BAJO: u64 = 1 << 7;
pub const DISCO_JUICIO_OCIOSAS_SHIFT: u64 = 8;
pub const DISCO_JUICIO_OCIOSAS_MASK: u64 = 0xFF;
pub const DISCO_JUICIO_FRONTERA_SHIFT: u64 = 16;
pub const DISCO_JUICIO_FRONTERA_MASK: u64 = 0xFFFF_FFFF;

/// Ranuras de comando que el driver usa de verdad.
///
/// ** El HBA declara 32 y `mod.rs` usa **la 0, siempre** -- con su motivo
/// escrito alli: un comando en vuelo es estado global del puerto y el
/// temporizador expropia. Vive aqui como constante con nombre para que la resta
/// `32 - 1` salga por la puerta y **se vea sin leer codigo**. El dia que el
/// driver encole, este numero cambia y el informe lo dice solo.
pub const RANURAS_EN_USO: u8 = 1;

/// Lo que contesto el disco. `None` hasta que un IDENTIFY sale bien.
static mut FOTO: Option<LoQueDiceElDisco> = None;

/// Guarda la foto a partir del sector crudo del IDENTIFY.
///
/// Se le pasa el buffer entero y no palabras sueltas a proposito: quien decide
/// que palabras importan es el padre, y este fichero no puede saberlo sin
/// empezar a interpretar.
pub fn tomar_foto(sector: &[u8]) {
    let Some(id) = Identify::nuevo(sector) else {
        // Un IDENTIFY corto no es un IDENTIFY con menos datos: es una lectura
        // que fallo. Se deja sin foto, que es lo que hace que todo lo de abajo
        // conteste en su forma conservadora.
        return;
    };
    unsafe { FOTO = Some(LoQueDiceElDisco::leer(&id)) };
}

/// Hay una foto valida?
pub fn hay_foto() -> bool {
    unsafe { FOTO.is_some() }
}

fn veredicto() -> Option<Veredicto> {
    let dice = unsafe { FOTO }?;
    Some(Veredicto::del_sistema(
        dice,
        super::model(),
        super::total_sectors(),
        RANURAS_EN_USO,
    ))
}

/// El medio, para decirlo en el arranque: una frase y su cifra.
///
/// ** Las cuatro frases son distintas **incluida la de "no contesta"**, que es la
/// que no se puede confundir con las otras tres. Un disco mudo y un HDD no son
/// el mismo caso, y el arranque es donde hay que verlo.
pub fn medio_en_palabras() -> (&'static str, u64) {
    match unsafe { FOTO }.map(|d| d.medio) {
        None => ("el IDENTIFY no dejo foto del medio", 0),
        Some(Medio::NoRota) => ("medio de ESTADO SOLIDO (palabra 217)", 1),
        Some(Medio::Rota { rpm }) => ("medio ROTACIONAL, rpm", rpm as u64),
        Some(Medio::NoContesta) => ("el disco NO DICE si gira (217 = 0)", 0),
        Some(Medio::Reservado { crudo }) => {
            ("la palabra 217 trae un valor RESERVADO", crudo as u64)
        }
    }
}

/// **Declara este disco que sabe recortar?** La palabra 169, leida.
///
/// ** Sin foto contesta `false`, y esa es la respuesta conservadora correcta:
/// mandar un `DATA SET MANAGEMENT` "a ver si suena" a un aparato que no lo
/// anuncio devuelve un error de task file que se lee como un driver roto.
pub fn trim_soportado() -> bool {
    unsafe { FOTO }.map(|d| d.trim.soportado).unwrap_or(false)
}

/// Cuantos bloques de payload admite el disco en UNA orden (palabra 105).
///
/// El minimo es 1 y lo garantiza ACS-3; quien lo traduce es `bmo-identify`, no
/// este fichero. Sin foto se contesta ese mismo 1: recortar despacio es
/// correcto, recortar de mas no.
pub fn trim_bloques_max() -> u16 {
    unsafe { FOTO }.map(|d| d.trim.bloques_max).unwrap_or(1)
}

/// `INFO_DISCO_MEDIO`: la palabra 217 cruda, su clase y las RPM.
pub fn medio() -> u64 {
    let Some(dice) = (unsafe { FOTO }) else { return 0 };
    let (clase, rpm, crudo) = match dice.medio {
        Medio::NoContesta => (DISCO_MEDIO_NO_CONTESTA, 0, 0),
        Medio::NoRota => (DISCO_MEDIO_NO_ROTA, 0, 1),
        Medio::Rota { rpm } => (DISCO_MEDIO_ROTA, rpm as u64, rpm as u64),
        Medio::Reservado { crudo } => (DISCO_MEDIO_RESERVADO, 0, crudo as u64),
    };
    (crudo & DISCO_MEDIO_CRUDO_MASK)
        | (clase << DISCO_MEDIO_CLASE_SHIFT)
        | ((rpm & DISCO_MEDIO_RPM_MASK) << DISCO_MEDIO_RPM_SHIFT)
}

/// `INFO_DISCO_ENLACE`: soportado, negociado, NCQ, cola y ranuras.
pub fn enlace() -> u64 {
    let Some(dice) = (unsafe { FOTO }) else { return 0 };
    let usadas = RANURAS_EN_USO;
    let ociosas = dice.cola.profundidad.saturating_sub(usadas);

    (dice.enlace.soportadas as u64 & 0x7)
        | ((dice.enlace.negociada as u64 & DISCO_ENLACE_NEGOCIADA_MASK)
            << DISCO_ENLACE_NEGOCIADA_SHIFT)
        | if dice.cola.ncq { DISCO_ENLACE_NCQ } else { 0 }
        | ((dice.cola.profundidad as u64 & DISCO_ENLACE_COLA_MASK)
            << DISCO_ENLACE_COLA_SHIFT)
        | ((usadas as u64 & DISCO_ENLACE_USADAS_MASK)
            << DISCO_ENLACE_USADAS_SHIFT)
        | ((ociosas as u64 & DISCO_ENLACE_OCIOSAS_MASK)
            << DISCO_ENLACE_OCIOSAS_SHIFT)
}

/// `INFO_DISCO_GEOMETRIA`: el exponente, el desplazamiento y TRIM.
pub fn geometria() -> u64 {
    let Some(dice) = (unsafe { FOTO }) else { return 0 };
    let g = dice.geometria;
    (g.exponente as u64 & DISCO_GEO_EXP_MASK)
        | if g.valida { DISCO_GEO_106_VALIDA } else { 0 }
        | ((g.desplazamiento as u64 & DISCO_GEO_DESPL_MASK)
            << DISCO_GEO_DESPL_SHIFT)
        | if g.desplazamiento_valido { DISCO_GEO_209_VALIDA } else { 0 }
        | if dice.trim.soportado { DISCO_GEO_TRIM } else { 0 }
}

/// `INFO_DISCO_JUICIO`: el veredicto del nieto, empaquetado.
///
/// ** Si no hay foto contesta 0, y eso NO es "todo bien": con el bit
/// `HAY_PERFIL` a cero y la frontera a cero, todo lo que dependa de esto toma su
/// camino conservador. Es la misma forma que `sin declarar` en el perfil de CPU.
pub fn juicio() -> u64 {
    let Some(v) = veredicto() else { return 0 };
    let c = v.contraste;
    let frontera_kib = v.frontera_de_escritura().unwrap_or(0) / 1024;

    (if v.perfil.is_some() { DISCO_JUICIO_HAY_PERFIL } else { 0 })
        | if v.medio_solido_confirmado() { DISCO_JUICIO_SOLIDO } else { 0 }
        | if v.la_barrera_es_lo_unico() { DISCO_JUICIO_SOLO_BARRERA } else { 0 }
        | if v.el_recolector_puede_avisar() { DISCO_JUICIO_TRIM } else { 0 }
        | if v.tiene_rendimiento_medido() { DISCO_JUICIO_MEDIDO } else { 0 }
        | if c.solido_sin_trim { DISCO_JUICIO_SOLIDO_SIN_TRIM } else { 0 }
        | if c.desalineado { DISCO_JUICIO_DESALINEADO } else { 0 }
        | if c.enlace_por_debajo { DISCO_JUICIO_ENLACE_BAJO } else { 0 }
        | ((c.ranuras_ociosas as u64 & DISCO_JUICIO_OCIOSAS_MASK)
            << DISCO_JUICIO_OCIOSAS_SHIFT)
        | ((frontera_kib & DISCO_JUICIO_FRONTERA_MASK)
            << DISCO_JUICIO_FRONTERA_SHIFT)
}
