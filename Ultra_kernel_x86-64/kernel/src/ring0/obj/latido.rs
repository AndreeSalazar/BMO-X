//! `KIND_LATIDO` -- **una interrupcion que despierta a un proceso de Ring 3.**
//!
//! generacion: nieto -- no sabe quien lo llamo ni por que.
//!
//! [cuesta]  TAREA -- si el testigo se compara mal, alguien duerme de mas o
//!           se despierta de menos. No concede nada que no se tuviera: un
//!           proceso ya podia dormir con `WAIT(0, _, timeout)`
//!
//! Pieza **S3** del suelo de `docs/plan/PLAN_SUELO_RING3.md`, y la unica de las
//! tres que el plan daba por *"no existe"*.
//!
//! # *** LO QUE DE VERDAD FALTABA ERA MENOS DE LO QUE EL PLAN SUPUSO
//!
//! La cadena entera estaba escrita, en tres sitios que no se habian juntado --
//! y los tres lo decian por escrito:
//!
//! ```text
//!    scheduler::wait_current_checked   ya bloquea SIN perder avisos
//!    scheduler::wake_by_key            ya despierta a todo el que espera una llave
//!    plat/irq.rs                       "el stub ya esta preparado para eso"
//!    syscall::wait                     "el manejador puede llamar a wake_by_key"
//! ```
//!
//! Lo que faltaba no era el mecanismo: era **un objeto al que un proceso pueda
//! agarrarse**, porque `WAIT` necesita un handle y no habia ninguno que
//! representara *"avisame cuando el hardware lata"*.
//!
//! # ** POR QUE ESTO NO PUEDE BLOQUEAR LA MAQUINA
//!
//! Un manejador de interrupcion que toma un cerrojo que el codigo interrumpido
//! ya tenia es un abrazo mortal, y es el fallo clasico de este mecanismo. Aqui
//! no puede pasar, y no por suerte:
//!
//! > `SpinLock::lock` hace `pushfq` + `cli`. **Mientras alguien tiene el cerrojo
//! > del planificador, las interrupciones estan APAGADAS**, asi que el latido no
//! > puede llegar en mitad de una seccion critica.
//!
//! [!] Y por eso el despertar NO llama a `wake_by_key`: eso volveria a tomar un
//! cerrojo que `on_timer` ya tiene, y un `SpinLock` no es reentrante. Se
//! despierta **dentro del recorrido que `on_timer` ya hace**, con el cerrojo ya
//! en la mano. Una vuelta, un cerrojo.
//!
//! # Las tres decisiones del plan, y cual sortea esta pieza
//!
//! ```text
//!    1. QUIEN puede pedirla    cualquiera: el reloj no concede autoridad
//!    2. la linea ENMASCARADA   no aplica: aqui no se enmascara nada
//!    3. CON QUE se prueba      con el TIMER, que ya late  <- por eso es esta
//! ```
//!
//! *** La 1 se contesta distinto que en `KIND_MMIO` a proposito. Alli el proceso
//! nombra un aparato porque conceder un aparato es conceder poder. **Aqui no se
//! concede nada que no se tuviera**: un proceso ya puede dormir con
//! `WAIT(0, _, timeout)`. Lo unico que cambia es que ahora puede despertar
//! **cuando late el hardware** en vez de cuando se le acaba un plazo -- y esa
//! diferencia es de PRECISION, no de permiso.
//!
//! [!] La decision 2 --que pasa con una linea enmascarada si su driver muere--
//! sigue abierta, y sigue siendo obligatoria **antes de la primera IRQ de un
//! aparato de verdad**. Esta pieza no la contesta porque no la necesita, y
//! decirlo es la diferencia entre sortear un problema y creer que no existe.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::ring0::obj::cap;

/// La llave sobre la que duermen los que esperan el latido.
///
/// Es un numero fijo y no un puntero: `wait_key` solo se compara, nunca se
/// deshace. Vale `0x1A71D0` --"latido" en la unica letra que cabe-- para que un
/// volcado de una tarea bloqueada se lea sin ir a buscar la constante.
pub const LLAVE: u64 = 0x1A71D0;

/// Cuantas veces ha latido el reloj desde el arranque.
///
/// ** Solo sube, y no se reinicia nunca. Un contador que da la vuelta o que se
/// pone a cero convierte *"espera al siguiente"* en *"espera para siempre"*: el
/// que esperaba `visto + 1` no lo ve pasar. A 64 bits y a 250 latidos por
/// segundo, la vuelta llega en dos mil millones de anos.
static CUENTA: AtomicU64 = AtomicU64::new(0);

/// **Un latido.** Lo llama `scheduler::on_timer`, con el cerrojo ya en la mano.
///
/// Sube el contador y **ya esta**: quien despierta es el recorrido que ya hay
/// ahi. Ver la nota del abrazo mortal en la cabecera.
#[inline]
pub fn tic() {
    CUENTA.fetch_add(1, Ordering::Relaxed);
}

/// El contador, para el que quiera mirarlo sin dormirse.
#[inline]
pub fn cuenta() -> u64 {
    CUENTA.load(Ordering::Relaxed)
}

/// **Concede el latido al proceso `pid`.**
///
/// No es exclusivo, y esa es la diferencia con la pantalla, el audio y la
/// ventana de un aparato: el reloj no se gasta. Cien procesos pueden esperarlo a
/// la vez y los cien despiertan.
///
/// Solo `RIGHT_WAIT`: sobre este handle no se lee ni se escribe nada, se espera.
pub fn claim(pid: u32) -> Result<u64, u32> {
    match cap::grant(pid, cap::KIND_LATIDO, cap::RIGHT_WAIT, LLAVE) {
        Some(h) => Ok(h),
        None => Err(cap::ERROR_PERMISSION_DENIED),
    }
}

/// Las operaciones sobre el handle. `None` = esta capability no la conoce.
pub fn operation(op: u64) -> Option<u64> {
    match op {
        crate::ring0::syscall::ops::LATIDO_OP_CUENTA => Some(cuenta()),
        _ => None,
    }
}

/// El dueno murio.
///
/// ** No hay nada que soltar, y se escribe la funcion IGUAL. El peaje 4 de
/// `docs/identidad/LA_COMPATIBILIDAD.md` --*se suelta al morir el dueno*-- se
/// paga aqui de la unica forma honesta cuando no hay nada que devolver: dejando
/// dicho POR QUE no lo hay, para que el que venga no tenga que deducirlo de una
/// ausencia.
///
/// El latido no es exclusivo, no enmascara ninguna linea, y no reserva memoria.
/// La capability la revoca `cap::revoke_all` como cualquier otra, y una tarea
/// muerta no aparece en el recorrido de `on_timer` porque ya no esta `Blocked`.
pub fn process_died(_pid: u32) {}
