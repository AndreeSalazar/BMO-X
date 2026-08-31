//! **QUE PUEDE PEDIR UN PROCESO QUE NO SEA UN OBJETO.** C4, y la delegacion.
//!
//! [carril]  ROJO      que puede pedir un proceso que no sea un objeto
//!
//! # El agujero, dicho por el propio kernel
//!
//! `syscall/ops.rs` lo llevaba declarado:
//!
//! > *"**Limitacion declarada**: hoy no esta atada a una capability, igual que
//! > `EJECUTAR`. Cualquier tarea de Ring 3 puede llamarla. (...) las dos
//! > operaciones quieren la misma capability el dia que exista."*
//!
//! O sea que **cualquier `.bex` que corriera podia reiniciar la maquina**. No
//! hacia falta un fallo: bastaba con pedirlo.
//!
//! # *** POR QUE ESTO NO ES UNA CAPABILITY, Y AHI ESTABA LO CARO
//!
//! C4 decia que lo que la bloqueaba no era comprobar sino **delegar**:
//!
//! > *"el escritorio lanza hijos todo el rato, asi que la capability tiene que
//! > llegarle a el sin que se la pueda pasar a lo que lanza."*
//!
//! Y con un handle eso no se puede: **un handle se pasa.** Es lo que los hace
//! utiles --`KIND_CONSOLA` viaja del lanzador al hijo y por eso un terminal
//! existe-- y es justo lo que aqui hay que impedir.
//!
//! ```text
//!    una CAPABILITY   la tienes, y puedes darla       -> se delega
//!    la AUTORIDAD     te la dio quien te creo         -> NO se delega
//! ```
//!
//! *** **Asi que no se resolvio la delegacion: se quito.** La autoridad no viaja
//! de padre a hijo por ningun camino, porque no hay ninguna operacion que la
//! mueva. Se fija al crear el proceso y **solo la puede fijar Ring 0**.
//!
//! # Y entonces quien la tiene
//!
//! ```text
//!    el escritorio         lo arranca el KERNEL (core/desktop.rs)   SI
//!    `run` del shell 0     lo teclea el dueno en Ring 0             SI
//!    un hijo de Ring 3     lo lanza otro proceso                    NO
//! ```
//!
//! *** **Los dos primeros tienen algo que el tercero no puede fingir: el que
//! pidio el lanzamiento estaba en Ring 0.** Un `.bex` no puede llegar ahi -- esa
//! es la frontera entera del sistema-- asi que no puede darse a si mismo lo que
//! esto concede.
//!
//! Y se comprobo antes de escribir una linea: **solo el director usa
//! `OP_EJECUTAR` y `OP_REINICIAR`** en todo Ring 3. Ninguna app las llama, asi
//! que cerrar esta puerta no le quita nada a nadie que la usara.
//!
//! # [!] LO QUE ESTO NO ES
//!
//! **No es una jerarquia de privilegios y no debe convertirse en una.** Son dos
//! bits para dos operaciones que no tienen objeto sobre el que colgar un handle.
//! Todo lo demas en BMO-X sigue siendo capabilities, y **eso es lo correcto**:
//! una autoridad ambiental no se puede acotar, y por eso hay exactamente dos.
//!
//! > El dia que aparezca una tercera, la pregunta que hay que hacerse primero es
//! > si esa operacion **de verdad no tiene objeto** -- o si es que todavia no se
//! > ha encontrado cual es.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::ring0::obj::cap::MAX_PROCS;

/// **Reiniciar la maquina.**
pub const REINICIAR: u64 = 1 << 0;
/// **Lanzar otro programa.**
pub const LANZAR: u64 = 1 << 1;

/// Lo que se le da a un proceso que arranco Ring 0.
pub const DE_SISTEMA: u64 = REINICIAR | LANZAR;

/// Lo que se le da a un proceso lanzado desde Ring 3. **Nada, y va con nombre**
/// para que el sitio que lo pasa diga lo que hace en vez de escribir un `0`.
pub const NINGUNA: u64 = 0;

/// Un `AtomicU64` por proceso. Sin cerrojo: cada pid tiene el suyo y nadie
/// escribe el de otro.
#[allow(clippy::declare_interior_mutable_const)]
const CERO: AtomicU64 = AtomicU64::new(0);
static AUTORIDAD: [AtomicU64; MAX_PROCS] = [CERO; MAX_PROCS];

/// **Fija la autoridad de un proceso recien creado.**
///
/// [!] Se llama UNA vez, al crear, y desde Ring 0. No hay `conceder_mas`: si
/// existiera, el camino para escalar seria llamarla, y todo lo de arriba se
/// vendria abajo. **La unica forma de tener autoridad es que te la dieran al
/// nacer.**
pub fn fijar(pid: u32, bits: u64) {
    if let Some(s) = AUTORIDAD.get(pid as usize) {
        s.store(bits, Ordering::Release);
    }
}

/// **Tiene este proceso esta autoridad?**
///
/// Un pid fuera de rango contesta que **no**. Es la respuesta segura: la otra
/// convierte un indice malo en un permiso.
pub fn tiene(pid: u32, bit: u64) -> bool {
    match AUTORIDAD.get(pid as usize) {
        Some(s) => s.load(Ordering::Acquire) & bit != 0,
        None => false,
    }
}

/// Al morir un proceso. **Sin esto, el pid reusado hereda la autoridad del
/// muerto** -- que es la unica forma que quedaba de colarse, y es de las que no
/// se ven hasta que la maquina lleva horas encendida.
pub fn olvidar(pid: u32) {
    if let Some(s) = AUTORIDAD.get(pid as usize) {
        s.store(0, Ordering::Release);
    }
}

/// Para el panel: que autoridad tiene cada uno, sin interpretarla.
pub fn bits(pid: u32) -> u64 {
    match AUTORIDAD.get(pid as usize) {
        Some(s) => s.load(Ordering::Acquire),
        None => 0,
    }
}
