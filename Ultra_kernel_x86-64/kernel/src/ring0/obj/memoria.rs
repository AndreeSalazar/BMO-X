//! **`KIND_MEMORIA`** — pedirle memoria al kernel, y que sea tuya.
//!
//! ═══ El hueco que tapa ═══
//!
//! Hasta ahora un proceso recibía su imagen y 64 KiB de pila, **y no podía
//! pedir más**. Eso bloqueaba dos cosas a la vez y por eso lleva tanto tiempo
//! en la hoja de ruta: cualquier lenguaje con recolector de basura, y cualquier
//! programa que no sepa de antemano cuánto va a necesitar.
//!
//! ═══ ★ Por qué NO es un `malloc` ═══
//!
//! Aquí se entrega **un bloque grande, entero y contiguo**, y se acabó. No hay
//! listas de libres, ni troceado, ni fusión de huecos. Y no es una versión
//! recortada de un asignador: es que **el asignador no es trabajo del kernel**.
//!
//! El caso que lo enseña es DOOM, y por eso es el que se usó para decidir:
//! pide ~8 MiB **una vez** al arrancar y se los administra él con su propio
//! `Z_Zone`. Un `malloc` general en el kernel habría sido escribir un asignador
//! que ese programa no usa, para que encima lo llame a través de un syscall.
//!
//! El reparto queda así, y es el mismo de siempre en este sistema:
//!
//! ```text
//!   el kernel  entrega páginas y dice dónde están
//!   el proceso decide qué hace con ellas
//! ```
//!
//! Un `malloc` de C se escribe **encima** de esto en Ring 3, con la política
//! que quiera cada lenguaje — y COBOL, Ada o un GC futuro pueden traer otra
//! sin pedirle permiso al kernel.
//!
//! ═══ Lo que NO hay, dicho entero ═══
//!
//! - **No se devuelve.** No hay `liberar`. Lo pedido vive hasta que el proceso
//!   muere, y entonces se destruye su espacio de direcciones entero. Para un
//!   programa que pide un bloque al arrancar eso es exactamente lo correcto;
//!   para uno que pida y suelte en un bucle, no — y por eso hay un tope de
//!   peticiones, para que ese caso falle **pronto y diciéndolo** en vez de
//!   comerse la RAM en silencio.
//! - **Contiguo en físico.** Se piden marcos seguidos porque un bloque que el
//!   programa recorre como un array tiene que serlo. Si la RAM está
//!   fragmentada y no hay hueco, se rechaza y **se dice** — entregar memoria a
//!   trozos sin avisar sería peor.
//! - **Sin tocar los tres syscalls.** Esto es una capability más y una
//!   operación más sobre `CURRENT_TASK`, como el framebuffer o la consola.

use crate::ring0::mm::{self, vmm};
use crate::ring0::obj::cap;

/// Cuánto puede pedir un proceso de una vez.
///
/// 64 MiB: ocho veces lo que pide DOOM, y aun así un número que no se puede
/// pedir por accidente. Un tope alto y explícito es mejor que ninguno — sin
/// él, un `pedir(-1)` mal calculado se lleva la máquina entera.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Cuántas veces puede pedir un proceso.
///
/// Cuatro. No hay forma de devolver memoria, así que el número de peticiones
/// ES el número de fugas posibles. Con esto, un programa que pida en un bucle
/// falla a la cuarta vuelta con un motivo, en vez de agotar la RAM y tumbar lo
/// que estuviera corriendo al lado.
pub const MAX_PETICIONES: usize = 4;

/// Dónde empieza el bloque. Espejo de `bmo_abi::…::MEM_OP_BASE`.
pub const MEM_OP_BASE: u64 = 0x01;
/// Cuántos bytes se le han entregado a este proceso.
pub const MEM_OP_BYTES: u64 = 0x02;

pub const ERROR_DEMASIADO: u32 = 0xE001;
pub const ERROR_SIN_RAM: u32 = 0xE002;
pub const ERROR_DEMASIADAS: u32 = 0xE003;
/// No queda ranura en la tabla de contabilidad: ya hay [`MAX_PROCS`] procesos
/// con memoria pedida. **Es un motivo distinto de `ERROR_DEMASIADAS`** y por eso
/// tiene código propio — uno dice "tú has pedido demasiado", el otro "el sistema
/// está lleno", y confundirlos manda a buscar el fallo al programa equivocado.
pub const ERROR_SIN_RANURA: u32 = 0xE004;

/// Cuántos procesos pueden tener memoria pedida **a la vez**.
///
/// ★ Y "a la vez" es la corrección entera. Esto era el tope de *pids* — la
/// tabla se indexaba con `pid as usize` y se rechazaba `pid >= 16`. Pero el pid
/// es un **contador que sólo sube y nunca se reutiliza** (`proc::next_pid`), así
/// que a partir del programa número 16 de un arranque **ningún proceso podía
/// volver a pedir memoria jamás**, y encima con el motivo equivocado:
/// `ERROR_DEMASIADAS`, que quiere decir "has pedido demasiadas veces".
///
/// No era teórico: en la foto del 2026-07-30, `info` decía **17 lanzados** en
/// una sola sesión.
///
/// Es el **patrón 17** otra vez —indexar algo VIVO con un contador HISTÓRICO—,
/// el mismo que dejó la máquina sin poder lanzar nada cuando `has_room()`
/// miraba una bitácora de ocho entradas. Ahora la tabla es de ranuras, se busca
/// por pid y **se libera al morir el proceso**: el tope vuelve a ser lo que
/// dice ser, un límite de recursos concurrentes.
const MAX_PROCS: usize = 16;

/// La contabilidad de un proceso que tiene memoria pedida.
#[derive(Clone, Copy)]
struct Cuenta {
    /// `0` = ranura libre. El pid 0 no existe: `NEXT_PID` empieza en 1.
    pid: u32,
    /// Dónde va el próximo bloque. Empieza en [`vmm::MEMORIA_VA_BASE`] y
    /// avanza; así dos peticiones del mismo proceso no se pisan.
    cursor: u64,
    peticiones: usize,
    /// Bytes entregados. Para el panel: la memoria que un proceso pidió es la
    /// única que el kernel no puede deducir mirando su imagen.
    entregados: u64,
}

const LIBRE: Cuenta = Cuenta { pid: 0, cursor: 0, peticiones: 0, entregados: 0 };

static mut CUENTAS: [Cuenta; MAX_PROCS] = [LIBRE; MAX_PROCS];

/// Total entregado desde el arranque, para `info`. **No baja al morir un
/// proceso**, y es a propósito: es "cuánta memoria ha pedido Ring 3 en esta
/// sesión", no "cuánta hay pedida ahora". Un contador histórico dicho como tal
/// no engaña a nadie; el problema es usarlo como índice.
static mut TOTAL: u64 = 0;

/// La ranura de este proceso, si tiene una.
fn ranura(pid: u32) -> Option<usize> {
    unsafe {
        let t = &*core::ptr::addr_of!(CUENTAS);
        t.iter().position(|c| c.pid == pid && pid != 0)
    }
}

/// La ranura de este proceso, tomando una libre si aún no tiene.
fn ranura_o_nueva(pid: u32) -> Option<usize> {
    if pid == 0 {
        return None;
    }
    if let Some(i) = ranura(pid) {
        return Some(i);
    }
    unsafe {
        let t = &mut *core::ptr::addr_of_mut!(CUENTAS);
        let i = t.iter().position(|c| c.pid == 0)?;
        t[i] = Cuenta { pid, ..LIBRE };
        Some(i)
    }
}

pub fn entregado_por(pid: u32) -> u64 {
    match ranura(pid) {
        Some(i) => unsafe { (*core::ptr::addr_of!(CUENTAS))[i].entregados },
        None => 0,
    }
}

/// Cuántos procesos tienen memoria pedida ahora mismo. Para el panel y para
/// que "no queda ranura" se pueda distinguir de "has pedido demasiadas veces".
pub fn procesos_con_memoria() -> usize {
    unsafe {
        let t = &*core::ptr::addr_of!(CUENTAS);
        t.iter().filter(|c| c.pid != 0).count()
    }
}

pub fn total_entregado() -> u64 {
    unsafe { TOTAL }
}

/// **Pide `bytes` de memoria.** Devuelve el handle de la capability; la
/// dirección se pregunta después con `MEM_OP_BASE`.
///
/// `aspace` es el espacio de direcciones del llamante — durante un syscall
/// desde Ring 3, CR3 **sigue siendo el suyo**: el cambio sólo ocurre en un
/// cambio de contexto, y aquí todavía no ha habido ninguno. Es la misma nota
/// que lleva el framebuffer, y por el mismo motivo.
pub fn pedir(pid: u32, aspace: u64, bytes: u64) -> Result<u64, u32> {
    if bytes == 0 || bytes > MAX_BYTES {
        return Err(ERROR_DEMASIADO);
    }
    // La ranura se toma AQUÍ, después de validar el tamaño: una petición
    // absurda no debe gastar una ranura de la tabla. Sin ranura libre el motivo
    // es otro y se dice con su nombre — antes esto contestaba
    // `ERROR_DEMASIADAS` a un proceso que no había pedido nunca nada.
    let slot = match ranura_o_nueva(pid) {
        Some(s) => s,
        None => {
            crate::ring0::cabina::warn(
                "mem",
                "no queda ranura de contabilidad para otro proceso",
                MAX_PROCS as u64,
            );
            return Err(ERROR_SIN_RANURA);
        }
    };
    unsafe {
        if (*core::ptr::addr_of!(CUENTAS))[slot].peticiones >= MAX_PETICIONES {
            return Err(ERROR_DEMASIADAS);
        }
    }

    let paginas = (bytes + mm::PAGE - 1) / mm::PAGE;
    // Contiguo: ver la cabecera. Un bloque que el programa recorre como un
    // array no puede llegarle a trozos.
    let fisica = match mm::phys::alloc_frames_contig(paginas) {
        Some(f) => f,
        None => {
            crate::ring0::cabina::warn("mem", "sin RAM contigua para la peticion", bytes);
            return Err(ERROR_SIN_RAM);
        }
    };

    let base = unsafe {
        let c = &mut (*core::ptr::addr_of_mut!(CUENTAS))[slot];
        if c.cursor == 0 {
            c.cursor = vmm::MEMORIA_VA_BASE;
        }
        c.cursor
    };

    let mut off = 0u64;
    while off < paginas * mm::PAGE {
        if vmm::map_page(aspace, base + off, fisica + off, true, true).is_err() {
            // Mapeo a medias = páginas sueltas en el espacio del usuario. Se
            // deshace lo hecho: quedarse con la mitad mapeada y sin handle es
            // peor que no tener nada. Mismo criterio que el framebuffer.
            let mut deshacer = 0u64;
            while deshacer < off {
                vmm::unmap_page(aspace, base + deshacer);
                deshacer += mm::PAGE;
            }
            return Err(ERROR_SIN_RAM);
        }
        off += mm::PAGE;
    }

    let handle = match cap::grant(
        pid,
        cap::KIND_MEMORIA,
        cap::RIGHT_READ | cap::RIGHT_WRITE,
        base,
    ) {
        Some(h) => h,
        None => {
            let mut deshacer = 0u64;
            while deshacer < paginas * mm::PAGE {
                vmm::unmap_page(aspace, base + deshacer);
                deshacer += mm::PAGE;
            }
            return Err(cap::ERROR_PERMISSION_DENIED);
        }
    };

    unsafe {
        let c = &mut (*core::ptr::addr_of_mut!(CUENTAS))[slot];
        c.cursor = base + paginas * mm::PAGE;
        c.peticiones += 1;
        c.entregados += paginas * mm::PAGE;
        TOTAL += paginas * mm::PAGE;
    }
    crate::ring0::cabina::info("mem", "bloque entregado a Ring 3", paginas * mm::PAGE);
    Ok(handle)
}

/// El proceso murió: **su ranura vuelve a estar libre.**
///
/// No se desmapea nada — el espacio de direcciones entero se destruye con el
/// proceso, y desmapear páginas de un CR3 que está a punto de morir es trabajo
/// para nadie. Lo que sí hay que soltar es la RANURA, y ahora esa palabra
/// significa algo: mientras la tabla se indexaba por pid, esto sólo ponía tres
/// contadores a cero y la ranura número `pid` seguía siendo suya para siempre.
/// Con `MAX_PROCS` ranuras y pids que sólo suben, eso agotaba la tabla en el
/// programa 16 del arranque.
pub fn proceso_muerto(pid: u32) {
    let Some(slot) = ranura(pid) else { return };
    unsafe {
        (*core::ptr::addr_of_mut!(CUENTAS))[slot] = LIBRE;
    }
}

/// Las operaciones sobre el handle. `base` es la VA con la que se concedió.
pub fn operacion(base: u64, operacion: u64, pid: u32) -> Option<u64> {
    match operacion {
        MEM_OP_BASE => Some(base),
        MEM_OP_BYTES => Some(entregado_por(pid)),
        _ => None,
    }
}
