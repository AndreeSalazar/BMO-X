//! **CARRIL VERDE** -- lo que solo MIRA, y los numeros.
//!
//! [cuesta]  NADA -- ni un `mut` que salga de aqui. Estas funciones leen la
//!           tabla y contestan; equivocarse devuelve un numero feo a quien
//!           pregunto, y decide ese quien.
//!
//! [riesgo]  -- ninguno declarado.
//!
//! # *** POR QUE LA LINEA ESTA JUSTO AQUI
//!
//! Porque la pregunta que un carril tiene que contestar es *"que arrastro si
//! toco esto"*, y en un planificador esa linea es exacta: **lo que escribe la
//! tabla puede dejar la maquina sin nadie corriendo; lo que la lee, no.**
//!
//! ** Ejemplo del 2026-08-30: `duenno_de_pila` se anadio para que la pantalla
//! azul dijera de que hilo era la pila. Doce lineas, un bucle y un `if`. Es
//! **verde**, y saberlo es lo que permite escribirla sin miedo un dia que la
//! maquina esta rota. Su vecina de arriba, `schedule_locked`, es roja.
//!
//! [!] Se lee SIN CERROJO, y esta decidido: lo llama la pantalla de fallo, y
//! colgarse ahi convierte un volcado legible en una maquina muda. Un valor a
//! medias es aceptable para un diagnostico; no arrancar, no.

use super::roja::{sched, Task, SCHEDULER, SCHED_LOCK, SWITCH_SNAP, TSC_FREQ};
use crate::ring0::mm;

pub const MAX_TASKS: usize = 64;

pub const DEFAULT_QUANTUM_TICKS: u16 = 4;


/// Lo que dura el turno de la que esta DELANTE. Paso 4 de `PLAN_DIRECTOR.md`.
///
/// ** Y ES QUANTUM Y NO PRIORIDAD, QUE ES LA DECISION ENTERA DEL PASO 4.
///
/// `choose_next` es prioridad ESTRICTA y sin envejecimiento: una tarea de
/// prioridad 1 le gana el turno a las de 0 **siempre que este lista**, y ceder
/// no ayuda porque quien cede sigue listo. Subirle la prioridad a la app de
/// delante le ganaria el turno al DIRECTOR --que esta en 0-- y entonces sus
/// pixeles dejarian de componerse: la ventana de delante seria la primera en
/// dejar de refrescarse. El efecto contrario al que la regla busca.
///
/// El quantum no tiene ese modo de fallo. La rueda sigue dando la vuelta
/// entera y nadie se queda fuera; lo unico que cambia es **cuanto** dura cada
/// parada. La prioridad es un ORDEN --y un orden estricto excluye--; el quantum
/// es un REPARTO.
///
/// ** El foco no decide QUIEN corre. Decide CUANTO.
pub const QUANTUM_DELANTE: u16 = 8;

/// 16 KiB, por el mismo motivo que en `proc.rs`: el contexto con XSAVE ocupa
/// ~3,3 KiB de pila en cada trap, contra los 720 bytes de cuando eran 8 KiB.
pub(super) const TASK_STACK_PAGES: u64 = 4;


#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Empty,
    Ready,
    Running,
    Blocked,
    Exited,
}


pub fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack)); }
    ((high as u64) << 32) | low as u64
}


/// **El reloj para MEDIR, no para mirar la hora.**
///
/// === Por que hacia falta un segundo, y que costo no tenerlo ===
///
/// `rdtsc()` lleva `options(nomem)`, que le promete al compilador que ese bloque
/// **no toca memoria**. Para leer la hora es cierto y es lo que hace que sea
/// barato. Para cronometrar es una mentira con consecuencias:
///
/// ```text
///    t0 = rdtsc();
///    <el trabajo>          <- nada lo ata a las dos lecturas...
///    t1 = rdtsc();         <- ...asi que puede salirse de en medio
/// ```
///
/// Sin `nomem`, el `asm!` es una **barrera para el compilador** y el trabajo se
/// queda donde esta. Y el `lfence` de delante es la otra mitad: `rdtsc` **no es
/// serializante**, asi que el CPU tambien puede adelantarlo por su cuenta.
///
/// ** Esto no es teoria. El 2026-08-11 `smp prueba` contesto `ticks con UN
/// nucleo =37` para un bucle de **400 millones de vueltas**. Treinta y siete.
/// El reparto funcionaba --once obreros entraron, vieron y terminaron-- y lo que
/// estaba roto era **el cronometro**, que es la clase de fallo que hace perder
/// dias buscando en el sitio equivocado.
///
/// Se cobra unos ciclos de mas por lectura, y por eso es una funcion aparte:
/// quien mira la hora sigue usando la barata.
pub fn rdtsc_serial() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        core::arch::asm!(
            "lfence",
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nostack),
        );
    }
    ((high as u64) << 32) | low as u64
}


pub fn tsc_freq() -> u64 {
    unsafe { TSC_FREQ }
}


pub fn ns_to_tsc(ns: u64) -> u64 {
    let hz = unsafe { TSC_FREQ };
    if hz == 0 {
        return 0;
    }
    ((ns as u128 * hz as u128) / 1_000_000_000u128) as u64
}


/// Copy of `SWITCH_SNAP` for the fault reporter.
pub fn switch_snap() -> [u64; 4] {
    unsafe { SWITCH_SNAP }
}


/// Number of switches into user tasks since boot (SWITCH_SNAP ordinal).
pub fn user_switches() -> u64 {
    unsafe { SWITCH_SNAP[3] }
}


/// Lock-free diagnostic read of a task's state by TID. Racy by design --
/// telemetry only. 255 = no live task with that TID (never existed, or
/// exited and was reaped).
pub fn tid_state(tid: u32) -> u8 {
    let s = unsafe { &*core::ptr::addr_of!(SCHEDULER) };
    for t in &s.tasks {
        if t.tid == tid && t.state != TaskState::Empty {
            return t.state as u8;
        }
    }
    255
}


/// El contexto guardado de una tarea (su `xsave_base`), o 0 si no existe.
///
/// Lo necesita Endpoint RPC para escribir el resultado de una llamada **en el
/// frame guardado del llamante**. Un syscall que bloquea no puede calcular su
/// valor de retorno despues de bloquearse: `wait_current_checked` vuelve en el
/// acto y el cambio de contexto se consuma en el epilogo, asi que para cuando
/// hubiera respuesta ese codigo ya se ejecuto. La respuesta se deja donde el
/// epilogo la va a recoger.
/// * Solo devuelve el contexto de una tarea **bloqueada**.
///
/// `context_rsp` es donde quedo guardada la tarea la ultima vez que salio del
/// CPU. Para una tarea que esta CORRIENDO ese valor es viejo: su estado real
/// vive en los registros, no en memoria. Escribir ahi no le llega -- pisa lo
/// que haya ahora en esa direccion de pila, que es de otra cosa.
///
/// Devolver 0 salvo que este `Blocked` convierte ese error en un no-op en vez
/// de en una corrupcion silenciosa de otro contexto.
/// **Quien estaba corriendo**: `(tid, es_user)`. Para la pantalla de fallo.
///
/// # SIN CERROJO, y es deliberado
///
/// Esto lo llama el manejador de faults. Si tomara `SCHED_LOCK` y el fallo
/// hubiera ocurrido **con ese cerrojo en la mano** --que es donde vive
/// `destroy_address_space`, entre otros-- la pantalla de fallo se colgaria
/// girando en un cerrojo que ya nadie va a soltar.
///
/// *** Y eso convierte un volcado legible en una maquina muerta y muda, que es
/// exactamente lo contrario de para lo que existe esa pantalla.
///
/// Leer sin cerrojo puede dar un valor a medias. **Para un diagnostico eso es
/// aceptable y colgarse no**: el mismo criterio que ya usa `context_rsp_of`.
pub fn quien_corre() -> (u32, bool) {
    let s = unsafe { &*core::ptr::addr_of!(SCHEDULER) };
    let t = &s.tasks[s.current];
    (t.tid, t.is_user)
}


/// **De quien es la pila donde se estrello.** `(tid, es_de_usuario)`, o `None`.
///
/// # *** POR QUE NO VALE `quien_corre()` PARA ESTO (2026-08-30)
///
/// La pantalla azul de hoy dijo las dos cosas a la vez:
///
/// ```text
///    rsp=0xFFFF800000B87C50   pila de HILO DEL KERNEL
///    corria tid=05  (Ring 3)
/// ```
///
/// Y no se contradicen: `quien_corre` da **el que el planificador cree que esta
/// corriendo**, y la pila dice **sobre que estaba el CPU de verdad**. Cuando un
/// hilo del kernel revienta, esas dos no tienen por que ser la misma, y hasta
/// hoy la pantalla solo sabia dar la primera.
///
/// *** Y LO PEOR ES QUE LA CABECERA DE `faults.rs` YA PROMETIA ESTO:
///
/// > *"sin saber CUAL hilo, 'un hilo del kernel' no acota nada. **Ahora lo
/// > dice**: hay dos, y el que late cada 4 ms es el del bus."*
///
/// El comentario lo daba por hecho y el codigo imprimia `pila de HILO DEL
/// KERNEL` a secas. Dos pantallas azules --26-08 y 30-08-- se gastaron sin
/// saber cual de los dos hilos era, y las dos tenian el `rsp` delante.
///
/// ** El dato estaba en la tabla desde siempre: cada tarea guarda `stack_phys`
/// y `stack_pages` porque `reap` los necesita para devolver los marcos. Lo
/// unico que faltaba era preguntar al reves -- de la direccion al dueno.
///
/// [!] Sin cerrojo, por lo mismo que `quien_corre`: esto lo llama la pantalla
/// de fallo, y colgarse ahi convierte un volcado legible en una maquina muda.
pub fn duenno_de_pila(rsp: u64) -> Option<(u32, bool)> {
    let s = unsafe { &*core::ptr::addr_of!(SCHEDULER) };
    for t in &s.tasks {
        if t.stack_phys == 0 || t.stack_pages == 0 {
            continue;
        }
        let base = mm::phys_to_virt(t.stack_phys);
        if rsp >= base && rsp < base + t.stack_pages * mm::PAGE {
            return Some((t.tid, t.is_user));
        }
    }
    None
}


pub fn context_rsp_of(tid: u32) -> u64 {
    let s = unsafe { &*core::ptr::addr_of!(SCHEDULER) };
    for t in &s.tasks {
        if t.tid == tid && t.state == TaskState::Blocked {
            return t.context_rsp;
        }
    }
    0
}


/// Spawn a kernel task running `entry(arg)` on its own 8 KiB stack.
/// Returns the new TID.
/// Queda una ranura de tarea libre?
///
/// Es la UNICA respuesta honesta a "cabe otro programa?": las ranuras se
/// reciclan cuando `reap` recoge una tarea que termino, asi que la capacidad
/// es la de AHORA y no la de todo lo que se ha lanzado desde el arranque.
///
/// Nacio de un bug de esa forma exacta: `proc::has_room` miraba la longitud de
/// un registro historico de ocho entries, y como ese registro no baja nunca,
/// tras ocho lanzamientos --cinco de ellos los demos del arranque-- la maquina no
/// admitia un programa mas hasta reiniciar.
pub fn hay_hueco() -> bool {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    s.tasks.iter().any(|t| t.state == TaskState::Empty)
}


/// Cuantas ranuras estan libres, para contarlo en CABINA.
pub fn huecos_libres() -> usize {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    s.tasks.iter().filter(|t| t.state == TaskState::Empty).count()
}


pub fn current_state() -> TaskState {
    let _g = SCHED_LOCK.lock();
    sched().tasks[sched().current].state
}


pub fn current_tid() -> u32 {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    s.tasks[s.current].tid
}


pub fn current_pid() -> u32 {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    s.tasks[s.current].pid
}


/// El espacio de direcciones (`cr3`) del proceso `pid`, si vive.
///
/// * Existe para poder RESCATAR la maquina. Quitarle la pantalla a un proceso
/// no es solo marcarla libre: hay que **desmapear sus paginas de framebuffer**,
/// y para eso hace falta su `cr3`. Sin esto, un programa al que se le retira la
/// pantalla seguiria teniendola mapeada y seguiria escribiendo encima del
/// escritorio -- dos duenos pintando el mismo sitio, que es peor que uno solo
/// pintando mal.
///
/// Se busca por `pid` y no por `tid` porque las capabilities son del PROCESO:
/// `fb::release` y `input::release` hablan en pids.
pub fn cr3_de_pid(pid: u32) -> Option<u64> {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    s.tasks
        .iter()
        .find(|t| t.pid == pid && t.state != TaskState::Empty && t.is_user)
        .map(|t| t.cr3)
}


/// Sigue viva la tarea `tid`?
///
/// Existe para poder comprobar si el ESCRITORIO sigue en pie. Cuando el
/// compositor se muere al arrancar, la maquina se queda en el panel del kernel
/// y hasta ahora no lo decia nadie: habia que deducirlo de que la ventana no
/// salia. Un sistema que sabe algo y no lo cuenta obliga a adivinarlo.
pub fn vive(tid: u32) -> bool {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    s.tasks.iter().any(|t| t.tid == tid && t.state != TaskState::Empty)
}


/// El `pid` de la tarea `tid`, si vive.
///
/// Existe porque Ring 3 solo conoce **tids** --`ejecutar_en` devuelve uno-- y los
/// prestamos de memoria van a un `pid`. Traducirlo aqui evita que el userland
/// tenga que aprender un concepto que no usa para nada mas.
pub fn pid_de(tid: u32) -> Option<u32> {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    s.tasks
        .iter()
        .find(|t| t.tid == tid && t.state != TaskState::Empty)
        .map(|t| t.pid)
}


/// El inverso: el tid de `pid`, si sigue vivo.
///
/// * Existe porque **Ring 3 solo conoce tids**. `EJECUTAR` devuelve un tid, y
/// `MEM_OP_OFRECER` recibe un tid y lo traduce con [`pid_de`]. El kernel, en
/// cambio, apunta parentesco y capabilities en pids -- son del PROCESO. Sin esta
/// traduccion, `TASK_OP_MI_PADRE` tendria que devolver un pid, y un programa que
/// se lo pasara a `ofrecer` estaria nombrando **a otro proceso cualquiera** que
/// resultara tener ese numero de tid. Dos espacios de nombres que se parecen es
/// como se cruzan dos identificadores sin que nada falle al compilar.
///
/// ** Devolver `None` cuando el proceso ya murio es parte del contrato, no un
/// hueco: es lo que convierte esta pregunta en un detector de vida. El DIRECTOR
/// pregunta por el dueno de una superficie cada fotograma y **el cero es la
/// senal de que hay que cerrar la ventana**.
pub fn tid_de(pid: u32) -> Option<u32> {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    s.tasks
        .iter()
        .find(|t| t.pid == pid && t.state != TaskState::Empty && t.state != TaskState::Exited)
        .map(|t| t.tid)
}


pub fn counts() -> (usize, usize) {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    let mut total = 0;
    let mut runnable = 0;
    for task in &s.tasks {
        if task.state != TaskState::Empty {
            total += 1;
        }
        if matches!(task.state, TaskState::Ready | TaskState::Running) {
            runnable += 1;
        }
    }
    (total, runnable)
}


