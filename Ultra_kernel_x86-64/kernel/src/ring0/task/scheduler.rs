//! Fixed-capacity scheduler with real context switching at trap boundaries.
//!
//! Design rule: **a context switch only ever happens at a trap boundary**
//! (timer IRQ or SYSCALL). Voluntary operations from kernel tasks just mark
//! state and park in a `hlt` loop; the next trap commits the switch through
//! the unified frame. SYSCALLs from Ring 3 are themselves trap frames, so
//! YIELD/WAIT/EXIT switch immediately and correctly from the dispatcher.
//!
//! A running context is captured into its task by the trap stub writing
//! `percpu.trap_rsp`; `schedule_locked` stores that into the outgoing task
//! and publishes the next task's `context_rsp` back to `percpu.trap_rsp`,
//! which the trap epilogue restores.

use crate::ring0::mm::{self, phys};
use crate::ring0::task::percpu;
use crate::ring0::plat::spin::SpinLock;
use crate::ring0::plat::trap;

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
const TASK_STACK_PAGES: u64 = 4;

const _: () = assert!(
    crate::ring0::plat::trap::MIN_TASK_STACK <= (TASK_STACK_PAGES * crate::ring0::mm::PAGE) as usize,
    "la pila de tarea de kernel no cubre un contexto con XSAVE"
);

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Empty,
    Ready,
    Running,
    Blocked,
    Exited,
}

#[derive(Clone, Copy)]
pub struct Task {
    pub tid: u32,
    pub pid: u32,
    pub state: TaskState,
    pub priority: u8,
    pub remaining_ticks: u16,
    /// Lo que dura SU turno cuando le toca. `DEFAULT_QUANTUM_TICKS` para todas;
    /// [`QUANTUM_DELANTE`] para la que el usuario tiene delante.
    pub quantum: u16,
    /// fxsave-base of the saved context (see trap.rs); 0 = never ran.
    pub context_rsp: u64,
    pub stack_phys: u64,
    pub stack_pages: u64,
    /// WAIT: channel/waitable identity the task sleeps on (0 = none).
    pub wait_key: u64,
    /// WAIT: absolute TSC deadline (0 = sleeps until `wait_key` fires).
    pub wait_deadline: u64,
    pub is_user: bool,
    pub cr3: u64,
    /// Top of the task's kernel stack (traps from Ring 3 land here via
    /// TSS.RSP0, and the SYSCALL entry reloads it from PerCpu).
    pub kernel_stack_top: u64,
}

impl Task {
    const EMPTY: Self = Self {
        tid: 0,
        pid: 0,
        state: TaskState::Empty,
        priority: 0,
        remaining_ticks: 0,
        quantum: DEFAULT_QUANTUM_TICKS,
        context_rsp: 0,
        stack_phys: 0,
        stack_pages: 0,
        wait_key: 0,
        wait_deadline: 0,
        is_user: false,
        cr3: 0,
        kernel_stack_top: 0,
    };
}

struct Scheduler {
    tasks: [Task; MAX_TASKS],
    current: usize,
    next_tid: u32,
}

impl Scheduler {
    const fn new() -> Self {
        Self { tasks: [Task::EMPTY; MAX_TASKS], current: 0, next_tid: 1 }
    }

    fn choose_next(&self) -> usize {
        let mut best = None;
        let mut best_priority = 0;
        for offset in 1..=MAX_TASKS {
            let index = (self.current + offset) % MAX_TASKS;
            let task = self.tasks[index];
            if task.state == TaskState::Ready && (best.is_none() || task.priority > best_priority) {
                best = Some(index);
                best_priority = task.priority;
            }
        }
        best.unwrap_or(self.current)
    }

    /// Free kernel stacks of exited tasks and recycle their slots.
    /// Never reaps the running task.
    ///
    /// * NI LA PILA QUE ESTAMOS PISANDO. `current_tid` ya es la tarea
    /// ENTRANTE cuando esto corre: la que acaba de hacer EXIT pasa el filtro,
    /// y sus frames volvian al mapa de bits **con RSP todavia dentro de
    /// ellos**. El epilogo aun tiene que ejecutarse ahi --`mov rsp, rax`, el
    /// retorno del `call`-- sobre memoria que ya es de cualquiera. No revienta
    /// en el acto: revienta cuando el siguiente `alloc_frames_contig` (una
    /// pila nueva, un buffer de DMA del AHCI) escribe encima. Ese retraso es
    /// justo lo que lo hace dificil de encontrar.
    ///
    /// La comprobacion es directa: si el RSP de ahora cae dentro de la pila de
    /// esa tarea, no se libera **y la tarea se queda `Exited`**, no se vacia
    /// el hueco. La recoge la siguiente pasada, ya desde otra pila. Un turno
    /// de retraso a cambio de no tirar el suelo.
    fn reap(&mut self) {
        let current_tid = self.tasks[self.current].tid;
        let rsp_ahora: u64;
        unsafe {
            core::arch::asm!("mov {}, rsp", out(reg) rsp_ahora, options(nomem, nostack));
        }
        // Por INDICE y no con `&mut self.tasks`: hay que poder mirar al RESTO de
        // la tabla --para saber si alguien mas comparte este espacio de
        // direcciones-- mientras se recoge una, y las dos cosas a la vez no
        // caben en un solo prestamo.
        for i in 0..self.tasks.len() {
            if self.tasks[i].state != TaskState::Exited || self.tasks[i].tid == current_tid {
                continue;
            }
            let (stack_phys, stack_pages) = (self.tasks[i].stack_phys, self.tasks[i].stack_pages);
            if stack_phys != 0 {
                let base = mm::phys_to_virt(stack_phys);
                let top = base + stack_pages * mm::PAGE;
                if rsp_ahora >= base && rsp_ahora < top {
                    continue; // es el suelo que estamos pisando
                }
                for p in 0..stack_pages {
                    phys::free_frame(stack_phys + p * mm::PAGE);
                }
            }
            // ** Y AQUI SE DEVUELVE EL ESPACIO DE DIRECCIONES ENTERO.
            //
            // Este es el sitio, y no `cap::revoke_all`: aquel corre todavia
            // dentro del syscall del moribundo y **con su CR3 puesto**, o sea
            // que destruir ahi el espacio seria tirar el suelo que se esta
            // pisando. `reap` corre despues del cambio de contexto y ya se salta
            // la tarea en curso, que es la misma garantia que necesita esto.
            //
            // Las hojas que vuelven son las marcadas con `PTE_NUESTRA` --imagen
            // y pila de usuario--; el framebuffer y lo prestado no llevan el bit
            // y no se tocan. Los bloques de `KIND_MEMORIA` tampoco: los devuelve
            // `obj::memory::process_died`, que ademas pregunta si estan
            // prestados antes.
            let (es_user, cr3, tid_muerto) =
                (self.tasks[i].is_user, self.tasks[i].cr3, self.tasks[i].tid);
            if es_user && cr3 != 0 && cr3 != mm::vmm::kernel_pml4() {
                // [!] Un espacio COMPARTIDO no se destruye. Hoy no hay hilos de
                // Ring 3 y esta condicion no se cumple nunca, pero el dia que
                // los haya el fallo seria que el primer hilo en morir se lleva
                // por delante a sus hermanos -- y ese no es un fallo que se
                // encuentre mirando: se encuentra con la maquina ya rota.
                let compartido = self.tasks.iter().enumerate().any(|(j, o)| {
                    j != i && o.state != TaskState::Empty && o.tid != tid_muerto && o.cr3 == cr3
                });
                if !compartido {
                    let (hojas, tablas) = mm::vmm::destroy_address_space(cr3);
                    crate::ring0::cabina::info("mm", "hojas devueltas al reciclar", hojas);
                    crate::ring0::cabina::info("mm", "tablas devueltas al reciclar", tablas);
                }
            }
            self.tasks[i] = Task::EMPTY;
        }
    }
}

static SCHED_LOCK: SpinLock = SpinLock::new("sched");
static mut SCHEDULER: Scheduler = Scheduler::new();
static mut TSC_FREQ: u64 = 0;

fn sched() -> &'static mut Scheduler {
    unsafe { &mut *core::ptr::addr_of_mut!(SCHEDULER) }
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

/// Boot task 0 is the shell/boot context itself; its context is captured on
/// its first trap. `tsc_hz` feeds WAIT deadline conversion.
pub fn init(tsc_hz: u64) {
    let _g = SCHED_LOCK.lock();
    unsafe { TSC_FREQ = tsc_hz };
    let s = sched();
    *s = Scheduler::new();
    s.tasks[0] = Task {
        tid: 1,
        pid: 0,
        state: TaskState::Running,
        priority: 0,
        remaining_ticks: DEFAULT_QUANTUM_TICKS,
        quantum: DEFAULT_QUANTUM_TICKS,
        ..Task::EMPTY
    };
    s.next_tid = 2;
}

/// Hay un contexto saliente que guardar?
///
/// `percpu::trap_rsp()` lo publica el stub de entrada de cada trap. Los stubs
/// de fault (#UD/#GP/#PF) **no publican nada a proposito**: el contexto que se
/// muere no se guarda. Pero entonces el valor que sigue ahi es el del trap
/// ANTERIOR --ya consumido por su epilogo, con la pila por encima libre para
/// que la pise cualquier cosa-- y guardarlo como contexto de nadie es sembrar
/// un `iretq` con basura para dentro de un rato.
///
/// Por eso la decision es un argumento y no una suposicion sobre un global.
#[derive(Clone, Copy, PartialEq)]
enum Saliente {
    /// Venimos de un stub que publico contexto: guardarlo.
    Publicado,
    /// Nadie publico (ruta de fault): la tarea que sale ya esta muerta y su
    /// `context_rsp` no se toca.
    Ninguno,
}

/// Commit a context switch if a better task exists. Must run with the lock
/// held and from a trap boundary only.
///
/// * **UN CONTEXTO SOLO SE GUARDA SI EL CAMBIO SE CONSUMA.** Esto costo tres
/// dias y tres fotos, asi que merece estar escrito entero.
///
/// Antes `context_rsp` se guardaba nada mas entrar, **antes** de saber si iba a
/// haber cambio. Cuando no lo habia --`next == current`, o el destino sin
/// contexto-- el epilogo restauraba ese mismo contexto en el acto y lo
/// **consumia**: `xrstor`, `pop`x15, `iretq`, y la ejecucion seguia por encima
/// de el. A partir de ese instante esa direccion es pila libre. Pero en la
/// tabla seguia anotada como "el contexto de esta tarea".
///
/// Con las tareas de usuario no se notaba: entran siempre por `TSS.RSP0`, o sea
/// por la cima de su pila, asi que su contexto cae siempre en el mismo sitio y
/// una direccion caducada resulta ser la buena por casualidad.
///
/// **La tarea 0 no.** Es el shell, corre en la pila de arranque, y el timer la
/// interrumpe a la profundidad a la que este en ese momento. Su contexto se
/// guarda a una profundidad distinta cada vez. Secuencia mortal:
///
/// 1. El timer interrumpe a la tarea 0 hondo; se publica el area A y se anota.
/// 2. No hay otra tarea lista: `next == current`, no hay cambio. El epilogo
///    restaura A y la tarea 0 sigue -- A queda consumida.
/// 3. La tarea 0 sube por la pila; un trap posterior, mas arriba, extiende su
///    propia area 1256 bytes hacia abajo y **escribe encima de A**.
/// 4. El compositor cede el turno. El planificador elige la tarea 0 y restaura
///    A, que ya es basura.
///
/// El sintoma exacto de la foto: el `xsave64` del vandalo dejo su `FCW`
/// (`0x37F`) justo en el `XSTATE_BV` de A. `XSTATE_BV = 0x37F` enciende bits
/// que `XCR0 = 0x7` no tiene, y eso es `#GP(0)` en `xrstor64` por definicion.
/// Con el compositor cediendo miles de veces por segundo, el paso 4 pasa
/// constantemente; por eso aparecio justo ahora.
fn schedule_locked(s: &mut Scheduler, saliente: Saliente) {
    let outgoing = percpu::trap_rsp();
    if s.tasks[s.current].state == TaskState::Running {
        s.tasks[s.current].state = TaskState::Ready;
    }
    let next = s.choose_next();
    if next == s.current {
        if s.tasks[next].state == TaskState::Ready {
            s.tasks[next].state = TaskState::Running;
        }
        s.reap();
        return;
    }
    let next_rsp = s.tasks[next].context_rsp;
    if next_rsp == 0 {
        // Never switch into a context that does not exist.
        if s.tasks[s.current].state != TaskState::Running {
            s.tasks[s.current].state = TaskState::Running;
        }
        return;
    }
    // -- El cambio VA a ocurrir. Ahora, y solo ahora, se guarda el saliente --
    //
    // A partir de aqui no hay vuelta atras: el epilogo va a restaurar
    // `next_rsp`, asi que el contexto que entro por este trap NO se consume y
    // su direccion sigue siendo valida hasta que esta tarea vuelva a entrar.
    // Guardarlo antes de este punto era anotar como vigente un contexto que un
    // instante despues se restauraba y quedaba caduco.
    if saliente == Saliente::Publicado && outgoing != 0 {
        s.tasks[s.current].context_rsp = outgoing;
        // El dueno, en el propio contexto. El stub ya puso la firma en
        // ensamblador; el tid lo sabe Rust. Con los dos, un epilogo que se
        // encuentre algo raro puede decir DE QUIEN era, no solo que estaba
        // roto.
        crate::ring0::plat::trap::seal(outgoing, s.tasks[s.current].tid);
    }
    s.tasks[next].state = TaskState::Running;
    s.tasks[next].remaining_ticks = s.tasks[next].quantum;
    s.current = next;
    percpu::set_trap_rsp(next_rsp);
    // Address space + Ring 3 trap landing pads follow the selected task.
    // Kernel code keeps running through the switch because every user
    // address space shares the kernel half and the identity map.
    let next_task = s.tasks[next];
    let next_cr3 = if next_task.is_user { next_task.cr3 } else { mm::vmm::kernel_pml4() };
    if next_cr3 != mm::vmm::read_cr3() {
        mm::vmm::switch_to(next_cr3);
    }
    // Las rampas de aterrizaje de Ring 3 (TSS.RSP0 y la pila de SYSCALL)
    // siguen a la tarea SIEMPRE que esta tenga pila propia, no solo cuando es
    // de usuario. Si solo se actualizan para tareas de usuario, al cambiar a
    // una tarea de kernel se quedan apuntando a la pila de la ultima tarea de
    // usuario -- que puede estar ya muerta y liberada. Es un puntero colgando
    // en el TSS: inofensivo mientras nadie entre por ahi, y catastrofico el
    // dia que alguien entre.
    if next_task.kernel_stack_top != 0 {
        crate::ring0::task::proc::set_tss_rsp0(next_task.kernel_stack_top);
        percpu::set_syscall_stack_top(next_task.kernel_stack_top);
    }
    if next_task.is_user {
        // Debug capture for the Ring 3 #GP hunt: the context pointer we just
        // published and what its back-pointer slot reads RIGHT NOW, under
        // the user CR3 that is already loaded. If this reads valid here but
        // zero in the trap epilogue an instant later, the content is being
        // clobbered in that window; if it is already zero here, the task
        // table entry itself is stale/corrupt. Painted by faults.rs.
        unsafe {
            SWITCH_SNAP[0] = next_rsp;
            SWITCH_SNAP[1] = ((next_rsp + crate::ring0::plat::trap::XSAVE_AREA as u64) as *const u64).read_volatile();
            SWITCH_SNAP[2] = mm::vmm::read_cr3();
            SWITCH_SNAP[3] = SWITCH_SNAP[3].wrapping_add(1);
            // El PRIMER cruce a CPL3 de la vida del sistema. Momento historico
            // y, sobre todo, el punto exacto donde antes moriamos: si CABINA lo
            // graba, el iretq a usuario ocurrio de verdad. Una sola vez -- esto
            // corre dentro del IRQ del timer, no puede ser charlatan.
            if SWITCH_SNAP[3] == 1 {
                crate::ring0::cabina::info("sched", "primer switch a CPL3 (userspace corre)", next_task.tid as u64);
            }
        }
    }
    s.reap();
}

/// Debug: last switch into a user task -- `[context_rsp, backptr@switch,
/// cr3@switch, ordinal]`. See the capture in `schedule_locked`.
pub static mut SWITCH_SNAP: [u64; 4] = [0; 4];

/// Copy of `SWITCH_SNAP` for the fault reporter.
pub fn switch_snap() -> [u64; 4] {
    unsafe { SWITCH_SNAP }
}

/// Number of switches into user tasks since boot (SWITCH_SNAP ordinal).
pub fn user_switches() -> u64 {
    unsafe { SWITCH_SNAP[3] }
}

/// Fault isolation: the CURRENT task took a CPU fault it cannot survive.
/// Mark it Exited (never the shell at index 0), commit a switch to the next
/// runnable context, and return the context_rsp the fault stub must restore.
/// Called from the #UD/#GP/#PF stubs when the fault came from CPL3 -- the
/// dying context is NOT saved (it is dead by definition), so unlike the
/// voluntary paths this never touches the outgoing frame.
pub fn kill_current_and_pick() -> u64 {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    if s.current != 0 && s.tasks[s.current].state == TaskState::Running {
        s.tasks[s.current].state = TaskState::Exited;
    }
    schedule_locked(s, Saliente::Ninguno);
    percpu::trap_rsp()
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
pub fn context_rsp_of(tid: u32) -> u64 {
    let s = unsafe { &*core::ptr::addr_of!(SCHEDULER) };
    for t in &s.tasks {
        if t.tid == tid && t.state == TaskState::Blocked {
            return t.context_rsp;
        }
    }
    0
}

/// Timer trap hook: sweep expired WAIT deadlines, account the quantum, and
/// reschedule when it expires.
pub fn on_timer() {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    let now = rdtsc();
    for task in &mut s.tasks {
        if task.state == TaskState::Blocked && task.wait_deadline != 0 && now >= task.wait_deadline {
            task.wait_deadline = 0;
            task.wait_key = 0;
            task.state = TaskState::Ready;
        }
    }
    let current = &mut s.tasks[s.current];
    if current.remaining_ticks > 1 {
        current.remaining_ticks -= 1;
        return;
    }
    current.remaining_ticks = current.quantum;
    schedule_locked(s, Saliente::Publicado);
}

/// Wake every task blocked on `key` (BMO Channel sequence change, F2+).
pub fn wake_by_key(key: u64) {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    for task in &mut s.tasks {
        if task.state == TaskState::Blocked && task.wait_key == key {
            task.wait_key = 0;
            task.wait_deadline = 0;
            task.state = TaskState::Ready;
        }
    }
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

pub fn spawn_kernel(entry: u64, arg: u64, priority: u8) -> Option<u32> {
    // Contiguous: the stack is addressed linearly through the physmap, and
    // `reap` frees it as `stack_phys + p*PAGE` -- both are only sound when
    // the frames really are physical neighbors.
    let stack_base = phys::alloc_frames_contig(TASK_STACK_PAGES)?;
    let stack_top = mm::phys_to_virt(stack_base) + TASK_STACK_PAGES * mm::PAGE;
    let context = unsafe { trap::fabricate(stack_top, entry, arg, false, 0) };

    let _g = SCHED_LOCK.lock();
    let s = sched();
    let index = s.tasks.iter().position(|t| t.state == TaskState::Empty)?;
    let tid = s.next_tid;
    s.next_tid = s.next_tid.wrapping_add(1).max(2);
    s.tasks[index] = Task {
        tid,
        pid: 0,
        state: TaskState::Ready,
        priority: priority.min(31),
        remaining_ticks: DEFAULT_QUANTUM_TICKS,
        quantum: DEFAULT_QUANTUM_TICKS,
        context_rsp: context,
        stack_phys: stack_base,
        stack_pages: TASK_STACK_PAGES,
        wait_key: 0,
        wait_deadline: 0,
        is_user: false,
        cr3: 0,
        kernel_stack_top: stack_top,
    };
    Some(tid)
}

/// Register a Ring 3 task whose initial context was fabricated by the
/// process loader. `kernel_stack` = the trap/syscall landing stack,
/// `cr3` = the user address space. Returns the new TID.
pub fn spawn_user(
    pid: u32,
    context_rsp: u64,
    kernel_stack_phys: u64,
    kernel_stack_pages: u64,
    kernel_stack_top: u64,
    cr3: u64,
    priority: u8,
) -> Option<u32> {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    let index = s.tasks.iter().position(|t| t.state == TaskState::Empty)?;
    let tid = s.next_tid;
    s.next_tid = s.next_tid.wrapping_add(1).max(2);
    s.tasks[index] = Task {
        tid,
        pid,
        state: TaskState::Ready,
        priority: priority.min(31),
        remaining_ticks: DEFAULT_QUANTUM_TICKS,
        quantum: DEFAULT_QUANTUM_TICKS,
        context_rsp,
        stack_phys: kernel_stack_phys,
        stack_pages: kernel_stack_pages,
        wait_key: 0,
        wait_deadline: 0,
        is_user: true,
        cr3,
        kernel_stack_top,
    };
    Some(tid)
}

// -- Voluntary operations ---------------------------------------------
// Each has a trap variant (immediate switch, used by the SYSCALL
// dispatcher) and a mark-only variant (kernel tasks; the next trap
// commits the switch while the task parks in `hlt`).

fn mark_yield(s: &mut Scheduler) {
    if s.tasks[s.current].state == TaskState::Running {
        s.tasks[s.current].state = TaskState::Ready;
    }
}

fn mark_exit(s: &mut Scheduler) {
    // Task 1 is the shell/boot context; it may not exit.
    if s.current != 0 && s.tasks[s.current].state == TaskState::Running {
        s.tasks[s.current].state = TaskState::Exited;
    }
}

fn mark_wait(s: &mut Scheduler, key: u64, deadline: u64) {
    if s.tasks[s.current].state == TaskState::Running {
        s.tasks[s.current].state = TaskState::Blocked;
        s.tasks[s.current].wait_key = key;
        s.tasks[s.current].wait_deadline = deadline;
    }
}

pub fn yield_current() {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    mark_yield(s);
    schedule_locked(s, Saliente::Publicado);
}

pub fn exit_current() {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    let tid = s.tasks[s.current].tid;
    let user = s.tasks[s.current].is_user;
    // Salida VOLUNTARIA (INVOKE EXIT). La distincion con la muerte por fault
    // (faults.rs) importa en la bitacora: una termino su trabajo, la otra la
    // mataron. Antes ambas se veian igual: un contador que bajaba.
    crate::ring0::cabina::info(
        if user { "ring3" } else { "sched" },
        "proceso termino por su cuenta (EXIT)",
        tid as u64,
    );
    mark_exit(s);
    schedule_locked(s, Saliente::Publicado);
}

/// **Cerrar OTRA tarea**, la que lleva ese `tid`. `true` si estaba viva y se
/// marco; `false` si ya no estaba o si no se puede cerrar.
///
/// La llama `obj::tarea::cerrar`, o sea alguien que tiene la capability del
/// hijo. Aqui no se comprueba el parentesco **a proposito**: el permiso ES el
/// handle, y repartir la misma comprobacion entre dos sitios es como se acaba
/// teniendo dos reglas que no dicen lo mismo.
///
/// ** SOLO TAREAS DE RING 3. Un `.bex` se puede cerrar; el hilo del bus USB o
/// el contexto de arranque, no. No es prudencia: una tarea de kernel no tiene
/// espacio propio que destruir ni capabilities que revocar, asi que marcarla
/// `Exited` seria dejar a medias algo que nadie va a recoger.
///
/// ** Y NO se reprograma aqui.** Marcar basta: `choose_next` ya no la elige, y
/// `reap` la recoge en la siguiente pasada con el mismo camino que sigue una
/// que hizo `EXIT` por su cuenta. Un camino menos que mantener.
/// **Poner (o quitar) el turno largo a una tarea.** `true` si se aplico.
///
/// La llama `obj::tarea::operation`, o sea alguien con la capability del
/// hijo. Igual que `terminar`, aqui no se comprueba el parentesco: el permiso
/// ES el handle.
///
/// ** Se apaga el de todos los demas de Ring 3 antes de encender el suyo.**
/// Delante hay UNO. Sin esa vuelta, cada cambio de foco dejaria una app mas
/// con turno largo y en diez minutos lo tendrian todas -- que es la misma
/// forma en que `nice()` dejo de significar nada en otros sistemas, solo que
/// por descuido en vez de por pedirlo.
pub fn delante(tid: u32) -> bool {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    let mut encontrada = false;
    for i in 0..s.tasks.len() {
        if !s.tasks[i].is_user || s.tasks[i].state == TaskState::Empty {
            continue;
        }
        if s.tasks[i].tid == tid {
            s.tasks[i].quantum = QUANTUM_DELANTE;
            encontrada = true;
        } else {
            s.tasks[i].quantum = DEFAULT_QUANTUM_TICKS;
        }
    }
    encontrada
}

pub fn terminar(tid: u32) -> bool {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    for i in 0..s.tasks.len() {
        if s.tasks[i].tid != tid || i == 0 {
            continue;
        }
        if !s.tasks[i].is_user {
            return false;
        }
        if s.tasks[i].state == TaskState::Empty || s.tasks[i].state == TaskState::Exited {
            return false;
        }
        s.tasks[i].state = TaskState::Exited;
        crate::ring0::cabina::info("sched", "cerrado por quien lo lanzo (tid)", tid as u64);
        return true;
    }
    false
}

pub fn wait_current(key: u64, deadline_tsc: u64) {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    mark_wait(s, key, deadline_tsc);
    schedule_locked(s, Saliente::Publicado);
}

/// WAIT with a lost-wakeup guard: `seq()` is sampled *under the scheduler
/// lock* and compared against `observed`. Because `wake_by_key` also takes
/// the lock, a kick can never slip between the compare and the block. If
/// the sequence already moved, returns it immediately without blocking.
pub fn wait_current_checked(
    key: u64,
    deadline_tsc: u64,
    observed: u64,
    seq: impl Fn() -> u64,
) -> u64 {
    let _g = SCHED_LOCK.lock();
    let current = seq();
    if current != observed {
        return current;
    }
    let s = sched();
    mark_wait(s, key, deadline_tsc);
    schedule_locked(s, Saliente::Publicado);
    // Still pre-switch on this stack: the context switch commits at the
    // trap epilogue. The value returned here is what the caller sees when
    // resumed, so it is advisory -- userland re-reads the shared page.
    observed
}

/// Kernel-task parking: mark blocked, then `hlt` until scheduled again.
pub fn park_until(deadline_tsc: u64) {
    {
        let _g = SCHED_LOCK.lock();
        let s = sched();
        mark_wait(s, 0, deadline_tsc);
    }
    while current_state() != TaskState::Running {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// Kernel-task exit: mark exited, then park forever (never resumed).
pub fn exit_and_park() -> ! {
    {
        let _g = SCHED_LOCK.lock();
        let s = sched();
        mark_exit(s);
    }
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
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
