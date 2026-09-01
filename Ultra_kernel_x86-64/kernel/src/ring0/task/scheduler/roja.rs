//! **CARRIL ROJO** -- lo que CAMBIA el estado del planificador.
//!
//! [carril]  ROJO      el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
//!
//! [cuesta]  MAQUINA -- aqui vive el cambio de contexto y `reap`. Un fallo no
//!           mata una tarea: deja la maquina sin nadie a quien darle el CPU, o
//!           con dos duenos del mismo marco.
//!
//! [riesgo]  AJENO -- `reap` desmonta el espacio de un MUERTO, y lo que lee de
//!           su ranura es lo que mas motivos tiene para estar pisado. El
//!           `cr3` de aqui es el que paro la maquina el 30-08.
//!
//! ** Y todo esto corre con `SCHED_LOCK` en la mano y las interrupciones
//! apagadas. Tomar otro cerrojo desde dentro es un abrazo mortal en la ruta que
//! corre 250 veces por segundo -- por eso `emergencia` solo levanta una bandera
//! y el trabajo lo hace el hilo del bus.

use super::verde::{
    TaskState, DEFAULT_QUANTUM_TICKS, MAX_TASKS, QUANTUM_DELANTE,
    TASK_STACK_PAGES, current_state, rdtsc,
};

use crate::ring0::mm::{self, phys};

use crate::ring0::task::percpu;

use crate::ring0::plat::spin::SpinLock;

use crate::ring0::plat::trap;


const _: () = assert!(
    crate::ring0::plat::trap::MIN_TASK_STACK <= (TASK_STACK_PAGES * crate::ring0::mm::PAGE) as usize,
    "la pila de tarea de kernel no cubre un contexto con XSAVE"
);


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


pub(super) struct Scheduler {
    pub(super) tasks: [Task; MAX_TASKS],
    pub(super) current: usize,
    pub(super) next_tid: u32,
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
                // *** EL TESTIGO QUE FALTA PARA CERRAR LO DEL 30-08.
                //
                // ** La guarda de arriba protege UNA pila: la de quien corre
                // `reap`. Y `exit_and_park` deja hilos que se marcan muertos y
                // **siguen sentados en la suya** dando `hlt`. Esa guarda no los
                // puede ver: son `Exited`, no son el actual, y su `rsp` no es
                // el nuestro.
                //
                // La pantalla azul del 30-08 encaja campo por campo --escritura,
                // no-presente, desde el kernel, en pila de hilo del kernel y
                // **de NADIE VIVO**-- pero encajar no es demostrar. Esta linea
                // lo demuestra o lo tumba en un arranque:
                //
                //     si sale `pila liberada de tid=NN base=0xB87000` y despues
                //     la azul dice `rsp=0xFFFF800000B87C50`, es el MISMO marco
                //     y el caso esta cerrado.
                //
                // [!] `cabina` desde aqui es seguro y ya esta probado: `reap`
                // llama a `destroy_address_space`, que llama a `caminable`, que
                // apunta en CABINA. No se toma ningun cerrojo nuevo.
                crate::ring0::cabina::addr(
                    "sched",
                    "pila de hilo liberada, base fisica",
                    stack_phys,
                );
                // *** PASO 0 DEL PLAN: MEDIR, NO ARREGLAR (2026-08-31).
                //
                // ** `reap` decide liberar mirando UNA SOLA COSA: el estado de
                // la tarea, mas la guarda del `rsp` que protege la pila que
                // estamos pisando. Y hay CUATRO punteros mas publicados que
                // pueden estar dentro de este rango:
                //
                // ```text
                //    TSS.RSP0                  donde aterriza un trap de Ring 3
                //    percpu.syscall_stack_top  donde aterriza un SYSCALL
                //    percpu.trap_rsp           el contexto vigente en este CPU
                //    otra tarea .context_rsp   un contexto guardado ajeno
                // ```
                //
                // Ninguno se comprueba. Hoy eso es SEGURO por un invariante que
                // **nadie escribio y nadie vigila**: *"antes de que Ring 3
                // vuelva a entrar siempre hay un cambio que refresca RSP0"*.
                //
                // > Un invariante que no esta escrito no es un invariante:
                // > es una suerte que dura hasta que deja de durar.
                //
                // Esto no lo arregla -- lo MIDE. Si alguna vez grita, el caso de
                // la pantalla azul esta cerrado y con nombre; si no grita nunca,
                // el invariante se cumple y hay que buscar en otro sitio. Las
                // dos respuestas valen, y ninguna cambia la conducta.
                let fin = mm::phys_to_virt(stack_phys) + stack_pages * mm::PAGE;
                let ini = mm::phys_to_virt(stack_phys);
                let dentro = |p: u64| p >= ini && p < fin;
                if dentro(crate::ring0::task::proc::tss_rsp0()) {
                    crate::ring0::cabina::fault(
                        "sched", "se libera la pila que el TSS publica (RSP0)", stack_phys);
                }
                if dentro(percpu::syscall_stack_top()) {
                    crate::ring0::cabina::fault(
                        "sched", "se libera la rampa de SYSCALL publicada", stack_phys);
                }
                if dentro(percpu::trap_rsp()) {
                    crate::ring0::cabina::fault(
                        "sched", "se libera la pila del contexto VIGENTE", stack_phys);
                }
                for (j, o) in self.tasks.iter().enumerate() {
                    if j != i && o.state != TaskState::Empty && dentro(o.context_rsp) {
                        crate::ring0::cabina::fault(
                            "sched", "se libera una pila con el contexto de otra tarea",
                            o.tid as u64);
                    }
                }
                anotar_muerta(self.tasks[i].tid, stack_phys, stack_pages);
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


/* -- ** LA MORGUE: las ultimas pilas de kernel que se liberaron -------------
 *
 * ** Nace de una pantalla azul que se repite y que el dueno sabe provocar:
 * matar el servidor de Ring 3 y volver a entrar. Sale asi, dos arranques
 * seguidos, con el mismo vecindario:
 *
 * ```text
 *    #PF  err=0x02  no-presente ESCRIBIENDO desde el KERNEL
 *    rip=0x0        <- CERO NO ES UNA DIRECCION
 *    cr2=0x8FFFFFFF <- un hueco: NADA se mapea ahi, nunca
 *    rsp=0xFFFF800000B88C50   pila de HILO DEL KERNEL -- de NADIE VIVO
 *    corria tid=05 (Ring 3)
 *    iq: en rsp no hay marco de iretq (cs=0x0000)
 * ```
 *
 * *** Y `de NADIE VIVO` es honesto: `spawn_user` SI guarda la pila de kernel de
 * una tarea de Ring 3 en `stack_phys`, asi que `duenno_de_pila` la habria visto
 * si su duena estuviera viva. No lo esta. Alguien libero esa pila y el kernel
 * siguio corriendo encima.
 *
 * == Por que un registro y no mas razonamiento ==
 *
 * Se han leido `exit_and_park`, `reap`, `schedule_locked` y las dos rutas de
 * muerte, y **todas quedan bien por inspeccion**: `reap` corre al final de
 * `schedule_locked`, que todavia esta en la pila SALIENTE, y la guarda del
 * `rsp` la protege. O sea que el razonamiento dice que esto no puede pasar, y
 * la maquina dice que pasa.
 *
 * > Cuando la lectura y el metal se contradicen, el que se equivoca es la
 * > lectura. Lo que hace falta no es otra teoria: es un NOMBRE.
 *
 * ** Asi que cada pila que se libera deja su ficha, y la pantalla azul la
 * consulta. `de NADIE VIVO` pasa a ser **`fue de tid=NN, liberada en el tick
 * NNNN`** -- y con eso, quien la libero y cuando dejan de ser una hipotesis.
 *
 * [!] Ocho fichas y anillo: esto se escribe con el cerrojo del planificador en
 * la mano y las interrupciones apagadas. Nada que pueda crecer, nada que pueda
 * asignar, nada que pueda tomar otro cerrojo.
 *
 * [!] Y se lee SIN cerrojo desde la pantalla de fallo, a proposito: la maquina
 * ya esta rota y colgarse ahi cambia un volcado legible por un silencio. Una
 * ficha a medias es aceptable para un diagnostico. */
#[derive(Clone, Copy)]
pub(super) struct PilaMuerta {
    pub tid: u32,
    pub base: u64,
    pub paginas: u64,
    pub tick: u64,
}

// ** TREINTA Y DOS Y NO OCHO, y el motivo salio del primer arranque.
//
// Con ocho, "la morgue no lo reconoce" tiene DOS lecturas -- o la pila no la
// libero `reap`, o si la libero y su ficha ya se habia ido por el anillo. Un
// resultado con dos lecturas no cierra nada, y este es EL resultado del que
// cuelga el plan entero (`docs/plan/PLAN_LA_PILA_HUERFANA.md`, seccion 6).
//
// Treinta y dos fichas son 1 KiB de `.bss`. La ambiguedad costaba un arranque.
pub(super) const MORGUE_FICHAS: usize = 32;

pub(super) static mut MORGUE: [PilaMuerta; MORGUE_FICHAS] = [PilaMuerta {
    tid: 0,
    base: 0,
    paginas: 0,
    tick: 0,
}; MORGUE_FICHAS];

pub(super) static mut MORGUE_N: usize = 0;

/// Cuantas pilas se han liberado en total. Se dice en la pantalla azul junto al
/// veredicto: si pasa de `MORGUE_FICHAS`, "no lo reconoce" vuelve a tener dos
/// lecturas y hay que decirlo en vez de callarlo.
pub fn pilas_liberadas() -> u64 {
    unsafe { MORGUE_N as u64 }
}

fn anotar_muerta(tid: u32, base: u64, paginas: u64) {
    unsafe {
        let n = MORGUE_N % MORGUE_FICHAS;
        MORGUE[n] = PilaMuerta {
            tid,
            base,
            paginas,
            tick: crate::ring0::plat::timer::ticks(),
        };
        MORGUE_N = MORGUE_N.wrapping_add(1);
    }
}


pub(super) static SCHED_LOCK: SpinLock = SpinLock::new("sched");

pub(super) static mut SCHEDULER: Scheduler = Scheduler::new();

pub(super) static mut TSC_FREQ: u64 = 0;


pub(super) fn sched() -> &'static mut Scheduler {
    unsafe { &mut *core::ptr::addr_of_mut!(SCHEDULER) }
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


/// Timer trap hook: sweep expired WAIT deadlines, account the quantum, and
/// reschedule when it expires.
pub fn on_timer() {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    let now = rdtsc();
    // *** S3 DEL SUELO: EL LATIDO, Y SE SUBE ANTES DE MIRAR A QUIEN DESPERTAR.
    //
    // El orden no es estilo. `wait_current_checked` compara el testigo **con
    // este mismo cerrojo en la mano**, asi que subirlo primero garantiza que el
    // que estaba a punto de dormirse ve el numero nuevo y no se duerme. Al
    // reves habria una ventana --testigo viejo, tarea ya bloqueada, latido ya
    // servido-- y eso es un aviso perdido: la tarea espera al SIGUIENTE.
    crate::ring0::obj::latido::tic();
    for task in &mut s.tasks {
        if task.state == TaskState::Blocked && task.wait_deadline != 0 && now >= task.wait_deadline {
            task.wait_deadline = 0;
            task.wait_key = 0;
            task.state = TaskState::Ready;
        // ** Y AQUI, EN LA MISMA VUELTA, LOS QUE ESPERAN EL LATIDO.
        //
        // No se llama a `wake_by_key`: eso volveria a tomar `SCHED_LOCK`, que
        // esta funcion ya tiene, y un `SpinLock` no es reentrante. Seria un
        // abrazo mortal en la ruta que corre 250 veces por segundo.
        //
        // [!] Y que esto pueda hacerse desde un manejador de interrupcion no es
        // suerte: `SpinLock::lock` hace `cli`, asi que mientras alguien tiene el
        // cerrojo del planificador **las interrupciones estan apagadas** y el
        // latido no puede llegar en mitad de una seccion critica.
        } else if task.state == TaskState::Blocked
            && task.wait_key == crate::ring0::obj::latido::LLAVE
        {
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


pub fn spawn_kernel(entry: u64, arg: u64, priority: u8) -> Option<u32> {
    // Contiguous: the stack is addressed linearly through the physmap, and
    // `reap` frees it as `stack_phys + p*PAGE` -- both are only sound when
    // the frames really are physical neighbors.
    let stack_base = phys::alloc_frames_contig(TASK_STACK_PAGES)?;
    // *** LA OTRA MITAD DEL TESTIGO. Ver `reap`.
    //
    // ** La cabecera de `reap` lleva escrito desde el 14-08 COMO se cobra una
    // pila liberada de mas:
    //
    //   > "No revienta en el acto: revienta cuando el siguiente
    //   >  `alloc_frames_contig` --UNA PILA NUEVA, un buffer de DMA del AHCI--
    //   >  escribe encima. Ese retraso es justo lo que lo hace dificil de
    //   >  encontrar."
    //
    // *** Y el 2026-08-30 el dueno reprodujo esa frase con las manos: DOOM
    // entero SIN morir, **cerrar Ring 3**, volver a entrar a `d.bex` --que pasa
    // por aqui-- y DOOM otra vez. La azul salio en la segunda.
    //
    // Asi que aqui se apunta la base que se ENTREGA. Con la de `reap` al lado,
    // las dos lineas lo cierran sin necesidad de que la maquina se pare:
    //
    //     sched: pila de hilo liberada, base fisica  0x...B87000
    //     sched: pila NUEVA para un hilo, base       0x...B87000   <- el MISMO
    //
    // ** Si los dos numeros coinciden, la pila se recicla debajo de alguien que
    // seguia encima. Si NO coinciden, la hipotesis se cae aqui y hay que buscar
    // en otro sitio -- que es para lo que sirve un testigo y no una teoria.
    crate::ring0::cabina::addr("sched", "pila NUEVA para un hilo, base", stack_base);
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
            // ** SE DICE EN CABINA, y no es traza de mas: el turno largo no
            // se VE. Un proceso que corre el doble se parece a uno que corre
            // normal en una maquina que va bien, asi que sin esta linea la
            // unica forma de saber si el foco movio algo seria leer el
            // codigo. El dueno vive en el escritorio: lo que no llega a
            // CABINA, para el no ha pasado.
            crate::ring0::cabina::info("sched", "turno largo, esta delante (tid)", tid as u64);
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


