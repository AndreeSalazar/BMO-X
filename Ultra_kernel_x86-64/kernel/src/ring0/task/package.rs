//! **De donde salio cada proceso** -- para que pueda leer su propia caja.
//!
//! ## Que problema resuelve
//!
//! Un `.bex` puede llevar sus datos dentro (la seccion `Resources`, ver
//! `bmo_abi::bef::recursos`). Para leerlos, el programa necesita abrir **su
//! propia imagen**, y hasta ahora la unica forma era escribir la ruta a mano:
//!
//! ```c
//! PAQUETE *p = paquete_abrir("apps/doom.bex");   /* y ay si lo mueves */
//! ```
//!
//! Eso esta mal por dos motivos, y el segundo es el que importa:
//!
//! 1. Un programa movido de sitio deja de encontrarse a si mismo.
//! 2. ** **Es pedir por nombre lo que se tiene por derecho.** En un sistema de
//!    capabilities un proceso no adivina identificadores: **se le entrega** lo
//!    que puede tocar. Que un programa abra un fichero *por su ruta* significa
//!    que podria abrir cualquier otro escribiendo otra ruta -- y da igual que
//!    hoy la ruta sea la suya.
//!
//! Asi que el kernel se acuerda de por donde entro cada proceso, y
//! `TASK_OP_MI_PAQUETE` le devuelve un handle de LECTURA sobre eso. El programa
//! no dice **cual**: dice *"el mio"*, y el kernel es quien sabe cual es.
//!
//! ## Por que una ruta y no un handle abierto
//!
//! La alternativa era dejar el fichero abierto desde el lanzamiento y regalarle
//! el handle. No: hay **cuatro** ranuras de archivo por proceso, y un programa
//! que nunca lee su paquete --que son todos los de hoy-- estaria gastando una
//! para siempre. Con la ruta se paga solo cuando se pide.
//!
//! ## El limite, dicho
//!
//! `RUTA_MAX` bytes y `MAX_VIVOS` procesos a la vez. Una ruta mas larga **no se
//! recorta**: no se recuerda, y `MI_PAQUETE` contesta que no hay. Media ruta
//! abriria *otro fichero*, que es peor que ninguno.

use crate::ring0::cabina;

/// Lo que mide la ruta mas larga que se recuerda.
const RUTA_MAX: usize = 64;
/// Cuantos procesos pueden tener paquete a la vez. Ocho ranuras de tarea y
/// margen: si se llena, se dice.
const MAX_VIVOS: usize = 16;

static mut PIDS: [u32; MAX_VIVOS] = [0; MAX_VIVOS];
static mut RUTAS: [[u8; RUTA_MAX]; MAX_VIVOS] = [[0; RUTA_MAX]; MAX_VIVOS];
static mut LARGOS: [u8; MAX_VIVOS] = [0; MAX_VIVOS];

/// Apunta de donde salio `pid`. Lo llama el lanzador tras admitir la imagen.
///
/// Se guarda **despues** de que la admision haya ido bien: recordar la ruta de
/// un proceso que no llego a existir dejaria basura que solo se limpia cuando
/// el pid se reutilice.
pub fn recordar(pid: u32, ruta: &str) {
    if pid == 0 || ruta.is_empty() {
        return;
    }
    if ruta.len() > RUTA_MAX {
        // No se recorta. Media ruta abre OTRO fichero.
        cabina::warn("paquete", "la ruta no cabe: este proceso no vera su caja", ruta.len() as u64);
        return;
    }
    unsafe {
        let libre = (0..MAX_VIVOS).find(|&i| PIDS[i] == 0 || PIDS[i] == pid);
        let Some(i) = libre else {
            cabina::warn("paquete", "sin ranura para recordar de donde salio", pid as u64);
            return;
        };
        PIDS[i] = pid;
        LARGOS[i] = ruta.len() as u8;
        RUTAS[i][..ruta.len()].copy_from_slice(ruta.as_bytes());
    }
}

/// De donde salio `pid`, si se recuerda.
pub fn ruta_de(pid: u32) -> Option<&'static str> {
    unsafe {
        for i in 0..MAX_VIVOS {
            if PIDS[i] == pid && LARGOS[i] > 0 {
                let n = LARGOS[i] as usize;
                let bytes = &*core::ptr::addr_of!(RUTAS[i][..n]);
                return core::str::from_utf8(bytes).ok();
            }
        }
        None
    }
}

/// Suelta la ranura. Lo llama `cap::revoke_all` con el resto de lo del proceso.
///
/// Sin esto un pid reutilizado heredaria la ruta del muerto, y `MI_PAQUETE` le
/// entregaria **la imagen de otro programa** -- que ademas cargaria y leeria
/// perfectamente, porque es un `.bex` valido. La clase de fallo que no revienta.
pub fn process_died(pid: u32) {
    unsafe {
        for i in 0..MAX_VIVOS {
            if PIDS[i] == pid {
                PIDS[i] = 0;
                LARGOS[i] = 0;
            }
        }
    }
}

/// Cuantas ranuras estan ocupadas. Para la autopsia y para las pruebas.
pub fn vivos() -> usize {
    unsafe { (0..MAX_VIVOS).filter(|&i| PIDS[i] != 0).count() }
}
