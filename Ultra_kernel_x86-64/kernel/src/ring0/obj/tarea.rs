//! `KIND_TAREA` -- **un hijo que yo lance**, como capability.
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! ## Que problema resuelve, y por que la respuesta obvia estaba mal
//!
//! El escritorio necesita cerrar una app. La forma facil es una operacion
//! *"matar al proceso N"*, y esa forma es **`root` con otro nombre**: si el
//! DIRECTOR puede cerrar a cualquiera *porque es el DIRECTOR*, la autoridad ha
//! dejado de venir de una capability y ha pasado a venir de un cargo. En el
//! sistema cuya primera clausula dice que la autoridad no se hereda, eso es
//! justo lo que no puede existir.
//!
//! Lo escribio el dueno en `docs/plan/PLAN_DIRECTOR.md`, paso 3, el 2026-08-10:
//!
//! > *"`EJECUTAR` devuelve un handle sobre el hijo, y matar es una operacion
//! > de ese handle. El DIRECTOR cierra lo que EL lanzo porque tiene su handle,
//! > no porque sea especial."*
//!
//! ## Por que no hace falta comprobar el parentesco
//!
//! Porque **la capability ya es la prueba**. Se concede una sola vez, en
//! `TASK_OP_EJECUTAR`, a quien lanzo. Quien no lanzo ese proceso no la tiene, y
//! `TASK_OP_HIJO` --que solo busca lo ya concedido, como `CHANNEL_OPEN`-- no
//! encuentra nada que darle.
//!
//! Anadir ademas una comprobacion de "es tu hijo?" seria tener la misma regla
//! en dos sitios, que es como se acaba con dos reglas que no dicen lo mismo.
//!
//! ## El TID como objeto, y por que aqui sale gratis
//!
//! El objeto de la capability es el TID del hijo. `next_tid` **solo sube**
//! (`wrapping_add(1).max(2)`), asi que un tid no se recicla y un handle viejo no
//! puede acabar nombrando a otro proceso: la tarea que buscaba ya no esta, y se
//! contesta que no esta.
//!
//! Es la propiedad que `loan.rs` tuvo que **ganarse** revocando el handle al
//! soltar --alli la direccion SI se reutiliza-- y que aqui viene con el numero.
//!
//! ## Lo que NO es
//!
//! No es una senal. No hay a quien pedirle que se vaya: una app en ventana
//! puede no tener entrada --hoy ninguna la tiene, ver la casilla 4 de
//! `META-APP_HARD.md`-- asi que esperar a que se entere seria esperar para
//! siempre. Cerrar aqui es lo mismo que hace `EXIT`, pedido desde fuera.

use crate::ring0::obj::cap;
use crate::ring0::task::scheduler;

/// Sigue vivo? Ver `bmo_abi::...::TAREA_OP_VIVE`.
const OP_VIVE: u64 = 0x01;
/// Su TID. Ver `bmo_abi::...::TAREA_OP_TID`.
const OP_TID: u64 = 0x02;
/// Cerrarlo. Ver `bmo_abi::...::TAREA_OP_CERRAR`.
const OP_CERRAR: u64 = 0x03;
/// Ponerlo delante. Ver `bmo_abi::...::TAREA_OP_DELANTE`.
const OP_DELANTE: u64 = 0x04;

/// **Conceder el handle**, en el momento de lanzar. Lo llama el brazo de
/// `TASK_OP_EJECUTAR`, y vive aqui y no alli porque es oficio de este objeto:
/// el despachador despacha, no reparte autoridad.
///
/// Se ignora el fallo a proposito: un lanzamiento que funciono no se convierte
/// en un fallo porque la tabla de capabilities este llena. Quien quiera el
/// handle lo pide con `TASK_OP_HIJO` y descubre alli que no esta.
pub fn conceder(padre_pid: u32, tid: u32) {
    let _ = cap::grant(
        padre_pid,
        cap::KIND_TAREA,
        cap::RIGHT_READ | cap::RIGHT_WRITE,
        tid as u64,
    );
}

/// **Buscar** el handle de un hijo por su tid. No concede: encuentra.
///
/// `None` cuando este proceso no lo lanzo -- y ese es todo el control de
/// acceso que hay, igual que `CHANNEL_OPEN` con el canal ya sembrado.
pub fn buscar(pid: u32, tid: u64) -> Option<u64> {
    cap::find(pid, cap::KIND_TAREA, tid)
}

/// Lo que contesta el handle. `objeto` es el TID del hijo.
///
/// `None` = operacion que este objeto no conoce, y quien llama la convierte en
/// el error de siempre. No se inventa un `0`: un cero aqui seria
/// indistinguible de *"esta muerto"*, y esa es exactamente la confusion que
/// `PRESTADO_OP_DUENO` documenta al otro lado.
pub fn operation(objeto: u64, op: u64, arg: u64) -> Option<u64> {
    let tid = objeto as u32;
    match op {
        OP_TID => Some(tid as u64),
        OP_VIVE => Some(match scheduler::pid_de(tid) {
            Some(_) => 1,
            None => 0,
        }),
        OP_CERRAR => {
            // ** EL ORDEN NO ES INDIFERENTE, y es el mismo que sigue
            // `TASK_OP_EXIT`: primero se revocan las capabilities --que despierta
            // a los que esperaban su respuesta, suelta la pantalla y calla el
            // sonido-- y solo despues se marca la tarea.
            //
            // `revoke_all` toma el cerrojo de capabilities y `terminar` el del
            // planificador; separados, nunca anidan. Al reves, un proceso
            // marcado muerto con sus handles todavia vivos deja endpoints
            // esperando a alguien que ya no va a contestar.
            let Some(pid) = scheduler::pid_de(tid) else {
                // Ya no esta. No es un fallo: cerrar algo cerrado es lo que pasa
                // cuando el usuario pulsa la X de una app que acaba de salir
                // sola, y contestar un error ahi obligaria a todo el mundo a
                // distinguir dos casos que se tratan igual.
                return Some(0);
            };
            cap::revoke_all(pid);
            Some(if scheduler::terminar(tid) { 1 } else { 0 })
        }
        OP_DELANTE => {
            // Quitarselo es dejar a TODOS en el turno normal: no hace falta
            // recordar quien lo tenia, porque delante hay uno y `delante()`
            // ya apaga el de los demas al encender el suyo.
            if arg == 0 {
                scheduler::delante(0);
                return Some(1);
            }
            Some(if scheduler::delante(tid) { 1 } else { 0 })
        }
        _ => None,
    }
}
