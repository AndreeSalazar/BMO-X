//! **QUIEN me lanzo** -- para poder ofrecerle mi superficie.
//!
//! ## Que problema resuelve
//!
//! Una app dibuja en su memoria y **se la ofrece al DIRECTOR** (ver
//! `<bmo/superficie.h>`). Para ofrecerla hay que nombrar al destinatario, y un
//! programa no tiene forma de nombrarlo: el kernel habla en tids, y el tid del
//! compositor no aparece por ninguna parte del espacio del hijo.
//!
//! La alternativa era un registro de nombres --*"quien es el compositor"*-- y
//! eso es exactamente lo que este sistema no hace: un nombre global es una
//! autoridad ambiental, y el que lo lea puede pedirle cosas a alguien que nunca
//! se las ofrecio. **Aqui la respuesta es local y concreta**: no *"quien manda"*
//! sino *"quien me lanzo a MI"*.
//!
//! ## ** Y por eso esto no es una jerarquia de procesos
//!
//! No hay `getppid`, ni grupos, ni herencia de nada. Se guarda **un pid** y solo
//! sirve para una cosa: traducirlo a tid y poder ofrecerle memoria. Un hijo no
//! gana ni un derecho sobre su padre ni al reves -- el padre ya podia ofrecerle
//! memoria (tiene su tid, se lo devolvio `EJECUTAR`) y esto solo hace que la
//! flecha valga en los dos sentidos.
//!
//! ## Por que un modulo propio y no dos columnas en `paquete.rs`
//!
//! Porque `paquete::recordar` **se rinde cuando la ruta no cabe** en `RUTA_MAX`,
//! y con razon: media ruta abre otro fichero. Si el padre viviera en esa misma
//! tabla, un programa con la ruta larga se quedaria ademas sin poder pintar en
//! una ventana, y el sintoma --*"esta app no compone y las demas si"*-- no
//! llevaria a la causa ni de lejos. Dos datos con motivos distintos para
//! faltar son dos tablas.

use crate::ring0::cabina;

/// Cuantos procesos pueden recordar a su padre a la vez. El mismo numero que
/// `paquete::MAX_VIVOS`, y por el mismo motivo: ocho ranuras de tarea y margen.
const MAX_VIVOS: usize = 16;

static mut HIJOS: [u32; MAX_VIVOS] = [0; MAX_VIVOS];
static mut PADRES: [u32; MAX_VIVOS] = [0; MAX_VIVOS];

/// Apunta que a `hijo` lo lanzo `padre`.
///
/// Lo llama el brazo de `EJECUTAR` --y **no** `lanzar.rs**, aunque ahi este el
/// hermano `paquete::recordar`. El motivo es que `lanzar::ruta` lo comparten el
/// syscall y el shell del kernel: llamando a `scheduler::current_pid()` desde
/// dentro, un `run` tecleado en el puerto serie le pondria de padre **a la
/// tarea que estuviera corriendo en ese instante** -- tipicamente el compositor,
/// que no lanzo nada. Un dato falso es peor que ninguno, porque este contesta
/// igual de rapido.
pub fn recordar(hijo: u32, padre: u32) {
    if hijo == 0 || padre == 0 || hijo == padre {
        return;
    }
    unsafe {
        let libre = (0..MAX_VIVOS).find(|&i| HIJOS[i] == 0 || HIJOS[i] == hijo);
        let Some(i) = libre else {
            cabina::warn("familia", "sin ranura para recordar quien lanzo a", hijo as u64);
            return;
        };
        HIJOS[i] = hijo;
        PADRES[i] = padre;
    }
}

/// Quien lanzo a `pid`, si se recuerda. `None` para los que arranco el kernel.
pub fn padre_de(pid: u32) -> Option<u32> {
    unsafe {
        for i in 0..MAX_VIVOS {
            if HIJOS[i] == pid && PADRES[i] != 0 {
                return Some(PADRES[i]);
            }
        }
        None
    }
}

/// Suelta la ranura. Lo llama `cap::revoke_all` con el resto de lo del proceso.
///
/// Se limpian **las dos puntas**: la fila de este pid como hijo, y las filas en
/// las que aparece como padre. Sin lo segundo, un pid reutilizado heredaria los
/// hijos del muerto y `MI_PADRE` mandaria una superficie **a un proceso que no
/// la pidio** -- que ademas la tomaria sin quejarse, porque el prestamo es
/// generico y no sabe que hay pixeles dentro.
pub fn process_died(pid: u32) {
    unsafe {
        for i in 0..MAX_VIVOS {
            if HIJOS[i] == pid || PADRES[i] == pid {
                HIJOS[i] = 0;
                PADRES[i] = 0;
            }
        }
    }
}

/// Cuantas ranuras estan ocupadas. Para la autopsia y para las pruebas.
pub fn vivos() -> usize {
    unsafe { (0..MAX_VIVOS).filter(|&i| HIJOS[i] != 0).count() }
}
