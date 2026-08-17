//! x86-64 SYSCALL entry and BMO ABI v2 dispatcher (**2** frozen syscalls).
//!
//! ```text
//!    [eje]     LATENCIA -- pays SIZE (the match arms) to buy cycles
//!    [camino]  P1 la puerta -- 100% of every door
//!    [coste]   87 (C) / 104 (Rust) ticks WITH the meter in; of those, 69-107
//!              were the meter's own two `rdtsc` -- so the real Rust work is
//!              ~20 ticks and was never visible
//!    [fila]    DISPATCH (techo 105, meta 60) -- only in the measuring build
//!    [gen]     PADRE -- knows WHICH door and whether the handle is
//!              `CURRENT_TASK`. It does not know what an object means; that
//!              is the grandchild's job, in `obj/*.rs`
//!    [exige]   R-CPU1, R-CPU2, R-BUS2 (nothing here touches MMIO under a
//!              foreign CR3), R-TIME1
//! ```
//!
//! ** The `meta` of 60 was never going to be reached by tuning the two-arm
//! `match`: most of what it measured was the thermometer. **The meter is now
//! behind `--features metro_puerta` and the default build has no `rdtsc` here**
//! -- verified in the bytes: `dispatch` went from 3 to 1, and the survivor
//! belongs to `WAIT`, which reads the clock for its timeout.
//!
//! [!] Decia "3" hasta el 2026-08-11, y llevaba diciendolo desde que
//! `CHANNEL_KICK` se retiro el 10-08 -- en este mismo fichero, cuarenta lineas
//! mas abajo, donde esta contado por que se fue. La cabecera de un modulo es lo
//! que se lee para saber que hay dentro, y **el numero de puertas de este
//! sistema no es un detalle: es lo que se promete que no va a cambiar**.
//!
//! The entry builds the unified trap frame (see trap.rs): swapgs, switch to
//! the per-CPU syscall stack, synthesize the 5-word trap tail (user SS/RSP/
//! RFLAGS/CS/RIP from the SYSCALL contract), push 15 GPRs, FXSAVE, then call
//! the Rust dispatcher with the frame pointer.
//!
//! Return is via `iretq`, never `sysretq`: one return path for traps and
//! syscalls, no non-canonical-RCX #GP hazard in Ring 0, and -- critically --
//! the dispatcher may answer with a *different* context than the one that
//! entered (YIELD/WAIT/EXIT switch right at the syscall boundary).
//!
//! The surface is frozen at `INVOKE` and `WAIT`. Everything else is a
//! capability operation resolved through `cap::resolve` -- new functionality
//! adds operations and handle kinds, never syscalls.
//!
//! The proof that this works is a number: **the operations went 22 -> 39 while
//! the doors went 3 -> 2.** The system grew and the surface shrank.

use core::arch::{asm, naked_asm};

use crate::ring0::obj::cap;
use crate::ring0::obj::channel;
use crate::ring0::obj::endpoint;
use crate::ring0::task::percpu;
use crate::ring0::task::scheduler;
use crate::ring0::plat::trap::TrapFrame;

// Minimal no-alloc view of the canonical bmo-abi v2 contract. Keeping
// these values here avoids linking the full alloc-using ABI implementation
// into Ring 0; build.ps1 rejects values that drift from bmo-abi.

/// THE OPERATION TABLE: the numbers, and nothing that runs. Every constant in
/// there has a twin in `bmo-abi`, in `bmo.h` and in the userland runtime, and
/// the build guard refuses to link if they disagree.
pub(crate) mod ops;
/// THE DOOR ITSELF: the naked entry stub. The only code in the kernel that runs
/// with no frame, no stack of its own and no compiler help.
mod entry;
pub use entry::init;

// El METRO de la puerta. Vive aparte porque medir es un trabajo distinto de
// despachar, y porque quien busque "cuanto cuesta un syscall" no tiene por que
// leerse el despachador para encontrarlo.
pub mod meter;
// Lo que una puerta TIENE PERMITIDO costar. Va al lado del metro porque uno
// mide y el otro juzga, y separarlos dejaria el numero sin contrato.
pub mod presupuesto;
pub(crate) use ops::*;

/// `MEM_OP_OFRECER` -- lend a memory block to another task. Lives with the
/// dispatch and not in the table because it is an operation on a MEMORY
/// handle, not on the current task.
const MEM_OP_OFRECER: u64 = 0x02;


#[inline]
fn unsupported() -> BmoStatus {
    BmoStatus::err(ERROR_UNSUPPORTED)
}

#[inline]
fn cap_err(err: (u32, u32)) -> BmoStatus {
    BmoStatus::err_with_flags(err.0, err.1)
}

fn invoke_current_task(operation: u64, arg0: u64, arg1: u64) -> BmoStatus {
    match operation {
        TASK_OP_GET_PID => BmoStatus::ok_value(scheduler::current_pid() as u64),
        TASK_OP_GET_TID => BmoStatus::ok_value(scheduler::current_tid() as u64),
        // These switch at the syscall boundary; when (if) this context runs
        // again it resumes here and reports success.
        TASK_OP_YIELD => {
            scheduler::yield_current();
            BmoStatus::ok_value(0)
        }
        TASK_OP_EXIT => {
            let _ = arg0;
            // Capabilities die with the process; every outstanding handle
            // becomes invalid before the final switch (no SCHED/CAP lock
            // nesting: revoke completes first).
            cap::revoke_all(scheduler::current_pid());
            scheduler::exit_current();
            BmoStatus::ok_value(0)
        }
        // Bootstrap console: render up to 8 packed bytes (LE, NUL-stop) to
        // the kernel's on-screen log + serial. This is how the first Ring 3
        // program draws -- the whole point of the CPL3->CPL0 demo. It writes
        // nothing but text and cannot escalate; the caller only ever paints
        // into the kernel-owned console surface.
        // La salida va a la consola ASIGNADA al proceso, si tiene una -- o al
        // panel del kernel si no, exactamente como antes. Lo nuevo rodea a lo
        // viejo en vez de romperlo: los cinco demos embebidos siguen hablando
        // por el panel sin cambiar una linea.
        TASK_OP_CONSOLE_WRITE => {
            let pid = scheduler::current_pid();
            match crate::ring0::obj::console::output_of(pid) {
                Some(idx) => {
                    // Desempaquetar aqui: el anillo guarda bytes, no palabras.
                    // El cero corta, igual que en la consola del kernel.
                    let w = arg0.to_le_bytes();
                    let n = w.iter().position(|&b| b == 0).unwrap_or(8);
                    crate::ring0::obj::console::write(idx, &w[..n]);
                    // ** Y TAMBIEN AL ANILLO DEL KERNEL, que es la caja negra.
                    //
                    // Esto es una bifurcacion de ENTREGA, no de registro: la
                    // consola asignada decide **quien lo lee en vivo**; el
                    // anillo de `uconsole` es lo que el kernel se acuerda de que
                    // dijo cada proceso, y eso no puede depender de a quien se
                    // lo estuviera diciendo.
                    //
                    // Se cobro el 2026-08-14: DOOM murio con `#GP` tras imprimir
                    // VEINTE lineas --hasta `I_Init: Setting up machine state.`--
                    // y su autopsia decia:
                    //
                    //     ultimo    (no escribio nada)
                    //
                    // Falso, y de la peor clase: no callaba, **afirmaba**. Su
                    // salida iba a la consola hija que le creo el escritorio, y
                    // el anillo del kernel no la veia pasar. Un informe que dice
                    // "no dijo nada" manda a mirar donde no es.
                    crate::ring0::uconsole::write_packed(arg0);
                }
                None => crate::ring0::uconsole::write_packed(arg0),
            }
            BmoStatus::ok_value(0)
        }
        TASK_OP_CONSOLE_READ => {
            let _ = arg0;
            let pid = scheduler::current_pid();
            match crate::ring0::obj::console::output_of(pid) {
                Some(idx) => BmoStatus::ok_value(crate::ring0::obj::console::read_entry(idx)),
                // Sin consola asignada no hay de donde leer. Cero = "nada", no
                // error: un programa que sondea no debe morir por preguntar.
                None => BmoStatus::ok_value(0),
            }
        }
        TASK_OP_DIR_ABRIR => {
            let _ = arg0;
            let pid = scheduler::current_pid();
            let ruta = ruta_tomar(pid);
            match crate::ring0::obj::directory::open(pid, ruta) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        // El eslabon que faltaba: el kernel sabia leer y escribir archivos y
        // Ring 3 no tenia con que pedirselo.
        TASK_OP_ARCHIVO_ABRIR => {
            let _ = arg0;
            let pid = scheduler::current_pid();
            let ruta = ruta_tomar(pid);
            match crate::ring0::obj::file::open(pid, ruta) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        TASK_OP_MI_PAQUETE => {
            let pid = scheduler::current_pid();
            // La ruta la sabe el KERNEL, no el programa. Si no la recuerda --los
            // binarios que el propio kernel embebe no vienen de ninguna-- se
            // dice que no, en vez de abrir cualquier cosa.
            let Some(ruta) = crate::ring0::task::package::ruta_de(pid) else {
                return BmoStatus::err(2);
            };
            match crate::ring0::obj::file::open(pid, ruta) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        TASK_OP_ARCHIVO_ASINC => {
            let _ = arg0;
            let pid = scheduler::current_pid();
            let ruta = ruta_tomar(pid);
            match crate::ring0::obj::file::abrir_asinc(pid, ruta) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        TASK_OP_MI_PADRE => {
            let pid = scheduler::current_pid();
            // Se contesta en TID y no en pid: es lo unico que Ring 3 sabe usar,
            // porque `ofrecer` recibe un tid. Y si el padre ya murio, `tid_de`
            // dice `None` y aqui sale un 0 -- la misma respuesta que "no tengo
            // padre", que es tambien la misma decision para quien pregunta.
            let tid = crate::ring0::task::family::padre_de(pid)
                .and_then(scheduler::tid_de)
                .unwrap_or(0);
            BmoStatus::ok_value(tid as u64)
        }
        TASK_OP_ARCHIVO_CREAR => {
            let _ = arg0;
            let pid = scheduler::current_pid();
            let ruta = ruta_tomar(pid);
            match crate::ring0::obj::file::create(pid, ruta) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        TASK_OP_CONSOLA_CREAR => {
            let _ = arg0;
            match crate::ring0::obj::console::create(scheduler::current_pid()) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        // Discover the caller's seeded estuary capability for index arg0.
        // The handle is the process's own; nothing new is granted here.
        TASK_OP_CHANNEL_OPEN => {
            if arg0 >= boot_context::MAX_CHANNEL_PAGES as u64 {
                return BmoStatus::err(ERROR_INVALID_ARGUMENT);
            }
            match cap::find(scheduler::current_pid(), cap::KIND_CHANNEL, arg0) {
                Some(handle) => BmoStatus::ok_value(handle),
                None => cap_err((cap::ERROR_PERMISSION_DENIED, cap::FLAG_NEEDS_CAP)),
            }
        }
        TASK_OP_ENDPOINT_CREATE => {
            match endpoint::create(scheduler::current_pid(), arg0 as usize) {
                Some(handle) => BmoStatus::ok_value(handle),
                None => BmoStatus::err(endpoint::ERROR_BUSY),
            }
        }
        TASK_OP_ENDPOINT_CONNECT => {
            match endpoint::grant_client(arg0 as usize, scheduler::current_pid()) {
                Some(handle) => BmoStatus::ok_value(handle),
                None => BmoStatus::err(endpoint::ERROR_ENDPOINT_DEAD),
            }
        }
        // La pantalla. El espacio de direcciones en el que se mapea es el que
        // esta cargado AHORA: durante un SYSCALL desde Ring 3, CR3 sigue
        // siendo el del llamante -- el cambio de CR3 solo ocurre en un cambio
        // de contexto, y aqui todavia no ha habido ninguno.
        TASK_OP_INPUT_CLAIM => {
            let _ = arg0;
            match crate::ring0::obj::input::claim(scheduler::current_pid()) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        TASK_OP_FRAMEBUFFER_CLAIM => {
            let _ = arg0;
            match crate::ring0::obj::fb::claim(
                scheduler::current_pid(),
                crate::ring0::mm::vmm::read_cr3(),
            ) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        // * SOLTAR la pantalla sin morirse. La pareja que le faltaba a
        // `FRAMEBUFFER_CLAIM`: hasta hoy la unica forma de dejar de ser dueno
        // era terminar, asi que el escritorio no podia prestarla ni queriendo y
        // `ray.bex` se llevaba un "la pantalla ya tiene dueno".
        //
        // El `CR3` es el del llamante, igual que al reclamar -- y aqui importa
        // mas, porque es de donde hay que DESMAPEAR: el proceso sigue vivo y
        // dejarle las paginas seria dejarle escribir en una pantalla que ya no
        // es suya.
        TASK_OP_ENTRADA_SOLTAR => {
            let _ = arg0;
            match crate::ring0::obj::input::release(scheduler::current_pid()) {
                Ok(()) => BmoStatus::ok_value(0),
                Err(code) => BmoStatus::err(code),
            }
        }
        TASK_OP_PANTALLA_SOLTAR => {
            let _ = arg0;
            match crate::ring0::obj::fb::release(
                scheduler::current_pid(),
                crate::ring0::mm::vmm::read_cr3(),
            ) {
                Ok(()) => BmoStatus::ok_value(0),
                Err(code) => BmoStatus::err(code),
            }
        }
        // * EL SONIDO. Sin CR3 y sin mapeos: aqui no se entrega memoria, se
        // entrega el DERECHO -- que es justamente lo que hace que esta pieza se
        // pueda escribir hoy, con el driver de HDA todavia sin existir.
        // * CABINA. `arg0` = campo, `arg1` = que evento (0 = el mas reciente).
        // Un campo que no existe contesta "no soportado" y no 0: un cero seria
        // indistinguible de un evento cuyo valor ES cero.
        TASK_OP_CABINA_INFO => {
            match crate::ring0::cabina::campo(arg0, arg1) {
                Some(v) => BmoStatus::ok_value(v),
                None => unsupported(),
            }
        }
        // `arg0` empaqueta `(evento << 32) | cual`, `arg1` es el trozo de 8 en
        // 8. Los dos indices en un argumento porque la puerta tiene tres y dos
        // ya estan ocupados -- la misma aritmetica que usa la autopsia.
        TASK_OP_CABINA_TEXTO => {
            let evento = arg0 >> 32;
            let cual = arg0 & 0xFFFF_FFFF;
            BmoStatus::ok_value(crate::ring0::cabina::texto(evento, cual, arg1))
        }
        TASK_OP_AUDIO_CLAIM => {
            let _ = arg0;
            match crate::ring0::obj::audio::claim(scheduler::current_pid()) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        TASK_OP_AUDIO_RELEASE => {
            let _ = arg0;
            match crate::ring0::obj::audio::release(scheduler::current_pid()) {
                Ok(()) => BmoStatus::ok_value(0),
                Err(code) => BmoStatus::err(code),
            }
        }
        // * Pedir memoria. Mismo comentario de CR3 que el framebuffer: durante
        // el syscall sigue cargado el espacio del llamante, que es justo donde
        // hay que mapear.
        TASK_OP_MEMORIA_PEDIR => {
            match crate::ring0::obj::memory::request(
                scheduler::current_pid(),
                crate::ring0::mm::vmm::read_cr3(),
                arg0,
            ) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        // * TOMAR lo que otro me ofrecio. El mapeo ocurre AQUI, en el espacio
        // del que llama -- por eso se toma y no se empuja: mapear en el espacio
        // de otro exigiria el `CR3` de un proceso que no esta corriendo, y esa
        // infraestructura no existe. Asi el destino es `read_cr3()` y ya.
        TASK_OP_TOMAR => {
            let pid = scheduler::current_pid();
            match crate::ring0::obj::loan::take(pid, crate::ring0::mm::vmm::read_cr3()) {
                Some(h) => BmoStatus::ok_value(h),
                None => BmoStatus::ok_value(0),
            }
        }
        TASK_OP_RUTA => {
            ruta_push(scheduler::current_pid(), arg0);
            BmoStatus::ok_value(0)
        }
        TASK_OP_INFO => {
            BmoStatus::ok_value(crate::ring0::core::report::campo(arg0))
        }
        TASK_OP_INFO_TEXTO => {
            BmoStatus::ok_value(crate::ring0::core::report::texto(arg0, arg1))
        }
        TASK_OP_KLOG_INFO => {
            use crate::ring0::core::klog;
            BmoStatus::ok_value(match arg0 {
                0 => klog::disponibles(),
                1 => klog::total(),
                _ => 0,
            })
        }
        TASK_OP_KLOG_TEXTO => {
            BmoStatus::ok_value(crate::ring0::core::klog::texto(arg0, arg1))
        }
        // * LA AUTOPSIA. Contesta texto y nada mas, como el klog y como INFO:
        // no concede una capability, no deja escribir, no deja mirar el espacio
        // de nadie. Es la parte "meta" del metakernel puesta en una fila de
        // tabla -- el sistema informa sobre si mismo.
        TASK_OP_AUTOPSIA_INFO => {
            use crate::ring0::core::autopsy;
            BmoStatus::ok_value(match arg0 {
                0 => autopsy::total(),
                1 => autopsy::disponibles(),
                2 => autopsy::renglones(arg1),
                _ => 0,
            })
        }
        TASK_OP_AUTOPSIA_TEXTO => {
            // `arg0` trae los dos indices: informe arriba, fila abajo.
            let informe = arg0 >> 32;
            let fila = arg0 & 0xFFFF_FFFF;
            BmoStatus::ok_value(crate::ring0::core::autopsy::texto(informe, fila, arg1))
        }
        // * Despertar nucleos DESDE Ring 3. Es la unica operacion de esta tabla
        // que cambia el estado del hardware en vez de contestar una pregunta, y
        // por eso conviene decir por que se acepta: no concede nada al llamante
        // --los APs quedan parados y sin tocar el kernel-- y el resultado es un
        // numero. Ver `plat/smp` y `docs/SMP_MAESTRO.md`.
        //
        // El aviso por nucleo se traga aqui: cruzar el borde de Ring 3 once
        // veces para pintar una linea costaria mas que el propio bring-up. Lo
        // que si queda es CABINA, que ya recibe el relato entero desde dentro.
        // `arg0` = cuantos despertar (0 = solo censar, `u32::MAX` = todos).
        // `arg1` = el modo: 0 despertar - 1 PARAR - 2 la prueba de reparto.
        // Devuelve 1 si encontro un aparato de reproduccion. Los NUMEROS van a
        // CABINA: son ocho y por la puerta cabe uno.
        TASK_OP_AUDIO_CENSO => {
            let hubo = unsafe { crate::ring0::dev::usb::audio::censar() };
            BmoStatus::ok_value(hubo as u64)
        }
        TASK_OP_SMP_DESPERTAR => {
            use crate::ring0::plat::smp::{self, crew};
            let cuantos = if arg0 > u32::MAX as u64 { u32::MAX } else { arg0 as u32 };
            match arg1 {
                // Desactivar: los obreros vuelven a `hlt` y ahi se quedan.
                1 => {
                    crew::parar();
                    crate::ring0::core::dashboard::dashboard_log("[smp] obreros PARADOS");
                    BmoStatus::ok_value(0)
                }
                // La prueba. Devuelve la aceleracion x100 --`842` son 8,42x--
                // porque por la puerta solo cabe un numero y una fraccion no
                // se puede mandar entera. El detalle en crudo va a CABINA.
                2 => {
                    let (alive, _) = smp::alive();
                    let (uno, todos, partes) = crew::prueba(alive);
                    crate::ring0::cabina::info("smp", "ticks con UN nucleo", uno);
                    crate::ring0::cabina::info("smp", "ticks con todos", todos);
                    crate::ring0::cabina::info("smp", "partes que corrieron", partes as u64);
                    // * LOS TRES TESTIGOS, siempre, salga bien o mal.
                    //
                    // En metal el 08-08 esto contesto `0.00x` y no habia nada
                    // mas que mirar: "falto una parte" no dice cuantas
                    // llegaron. Estos tres numeros parten el camino en los tres
                    // sitios donde se puede romper -- entrar al bucle, ver la
                    // ronda, terminar la faena-- y la diferencia entre dos
                    // consecutivos senala el tramo culpable.
                    let (entraron, vieron, hechos) = crew::testigos();
                    crate::ring0::cabina::info("smp", "obreros que ENTRARON al bucle", entraron as u64);
                    crate::ring0::cabina::info("smp", "obreros que VIERON la ronda", vieron as u64);
                    crate::ring0::cabina::info("smp", "obreros que TERMINARON", hechos as u64);
                    // ** Y LA MEDIDA, DENUNCIADA POR ELLA MISMA.
                    //
                    // El 08-11 esto dio `37` ticks para 400 millones de vueltas
                    // con los once obreros entrando, viendo y terminando. Los
                    // testigos decian que el reparto iba bien y el numero decia
                    // que no, y **nadie sospecho del reloj**. Ahora lo dice el.
                    crate::ring0::cabina::info("smp", "el hash que dejo la faena", crew::suma_testigo());
                    if !crew::medida_creible(uno) {
                        crate::ring0::cabina::fault(
                            "smp",
                            "esa medida es IMPOSIBLE para las vueltas que son: el cronometro miente, no el reparto",
                            uno,
                        );
                    }
                    if hechos < alive {
                        crate::ring0::cabina::warn(
                            "smp",
                            "faltan obreros por terminar",
                            (alive - hechos) as u64,
                        );
                    }
                    // * Y la otra mitad del resultado, que no es la velocidad.
                    // Doce nucleos calculando a la vez es justo el momento en
                    // que un choque de cerrojo aparece si va a aparecer, y una
                    // aceleracion contada sin mirar esto es media medida.
                    // Ver `plat/spin.rs` y `docs/SMP_MAESTRO.md`.
                    let (choques, pico) = crate::ring0::plat::spin::contention();
                    if choques == 0 {
                        crate::ring0::cabina::info("smp", "cerrojos: ni un choque", 0);
                    } else {
                        crate::ring0::cabina::warn(
                            "smp",
                            "CHOQUES de cerrojo: alguien entro en el kernel",
                            choques as u64,
                        );
                        crate::ring0::cabina::warn(
                            "smp",
                            crate::ring0::plat::spin::worst(),
                            pico as u64,
                        );
                    }
                    crate::ring0::core::dashboard::dashboard_log("[smp] prueba de reparto hecha");
                    if todos > 0 && partes > 0 {
                        BmoStatus::ok_value(uno.saturating_mul(100) / todos)
                    } else {
                        BmoStatus::ok_value(0)
                    }
                }
                _ => {
                    let (alive, esperados) = smp::despertar(cuantos, |_| {});
                    // ** EN QUE ESTA CADA NUCLEO, A CABINA.
                    //
                    // Lo pidio el dueno con estas palabras: *"que el smp asi
                    // natural ayude a verify los cores y hilos: que se estan
                    // usando, y que la cabina con filtros pueda decir que esta
                    // ejecutando"*.
                    //
                    // La tabla ya existia en el shell de Ring 0, y al shell de
                    // Ring 0 se llega cuando el escritorio NO arranca. Desde la
                    // caja del escritorio no habia forma de verla. Ahora va a
                    // CABINA, que es el sitio que se mira desde los dos lados y
                    // el unico que tiene filtros.
                    //
                    // El valor de cada evento es `nucleo * 16 + estado`, que
                    // cabe en un numero y se lee de un vistazo: la decena es el
                    // nucleo y la unidad el estado.
                    let hilos = match (crate::ring0::cpu_vendor::profile::active().nucleos)() {
                        Some(t) => (t.hilos as u32).min(32),
                        None => alive + 1,
                    };
                    // ** CORE o THREAD en el propio mensaje, y en ingles.
                    //
                    // Lo pidio el dueno para la vista y para los FILTROS, y esa
                    // segunda mitad es la que manda: CABINA filtra por texto de
                    // modulo y por gravedad, asi que meter la palabra **dentro
                    // del mensaje** es lo que permite leer de un vistazo cuantos
                    // de los que estan en pie son nucleos de verdad.
                    //
                    // Y hace falta: `12 hilos` no dice si son doce nucleos o
                    // seis con SMT, y de eso depende cuantos obreros pedir --
                    // calculo denso quiere seis, no doce.
                    for id in 0..hilos {
                        let e = smp::estado_de(id);
                        let t = smp::tipo_de(id);
                        // `CORE OBRERO` / `THREAD DORMIDO`: dos palabras, la
                        // primera dice QUE es y la segunda EN QUE esta.
                        let msg: &'static str = match (t, e) {
                            ("CORE", smp::Estado::Maestro) => "CORE   MASTER",
                            ("CORE", smp::Estado::Obrero) => "CORE   worker",
                            ("CORE", smp::Estado::Dormido) => "CORE   asleep",
                            ("CORE", smp::Estado::Ausente) => "CORE   ABSENT",
                            ("CORE", _) => "CORE   -",
                            ("THREAD", smp::Estado::Maestro) => "THREAD MASTER",
                            ("THREAD", smp::Estado::Obrero) => "THREAD worker",
                            ("THREAD", smp::Estado::Dormido) => "THREAD asleep",
                            ("THREAD", smp::Estado::Ausente) => "THREAD ABSENT",
                            ("THREAD", _) => "THREAD -",
                            _ => "?      -",
                        };
                        crate::ring0::cabina::info("smp", msg, id as u64);
                    }
                    // ** Y el coste, que es el numero del ahorro. Hoy es
                    // incomodo a proposito: el que espera GIRA, no duerme.
                    let girando = smp::girando();
                    if girando > 0 {
                        crate::ring0::cabina::warn(
                            "smp",
                            "nucleos GIRANDO en vacio al 100% (con MWAIT serian 0)",
                            girando as u64,
                        );
                    }
                    // ** BIT 63 = LOS OBREROS ESTAN PARADOS.
                    //
                    // Cabe de sobra --`alive` no pasa de 32-- y hace falta
                    // porque el numero solo mentia por omision: `smp stop`
                    // seguido de `smp` contestaba `12 de 12`, que es cierto y se
                    // lee como "el stop no hizo nada". Ring 3 pinta la mitad que
                    // faltaba; el kernel no opina, solo dice el hecho.
                    let parados = if crew::parados() { 1u64 << 63 } else { 0 };
                    BmoStatus::ok_value(parados | ((alive as u64) << 32) | esperados as u64)
                }
            }
        }
        // * Escribe en el disco. Se apunta en CABINA ANTES y DESPUES, pase lo
        // que pase: la primera operacion que cambia el almacen no puede ser
        // silenciosa ni cuando funciona.
        TASK_OP_ESTRATOS_SELLAR => {
            crate::ring0::cabina::info(
                "estratos",
                "sellado pedido por un proceso de Ring 3",
                scheduler::current_pid() as u64,
            );
            match crate::ring0::fsys::estratos::seal() {
                Ok(g) => BmoStatus::ok_value(g),
                Err(e) => {
                    crate::ring0::cabina::warn("estratos", e.name(), 0);
                    BmoStatus::ok_value(0)
                }
            }
        }
        // -- El cursor de ESTRATOS --
        //
        // CONTESTA, no autoriza -- el mismo trato que `INFO` y que el klog.
        // Ninguna de estas preguntas cambia el volumen: no hay aqui una sola
        // operacion que escriba, y por eso no piden capability propia.
        TASK_OP_ES_NODO => {
            use crate::ring0::fsys::estratos::cursor;
            BmoStatus::ok_value(match arg0 {
                ES_NODO_RAIZ => cursor::a_la_raiz() as u64,
                ES_NODO_HIJOS => cursor::hijos(),
                ES_NODO_TRUNCADO => cursor::truncado(),
                ES_NODO_HONDO => cursor::hondo(),
                ES_NODO_TIPO => cursor::tipo(),
                ES_NODO_HIJO_TIPO => cursor::hijo_tipo(arg1 as usize),
                ES_NODO_ENTRAR => cursor::entrar(arg1 as usize) as u64,
                ES_NODO_SUBIR => cursor::subir() as u64,
                ES_NODO_HIJO_BYTES => cursor::hijo_bytes(arg1 as usize),
                ES_NODO_HIJO_ATRIBUTOS => cursor::hijo_atributos(arg1 as usize),
                ES_NODO_HIJO_FIRMADO => cursor::hijo_firmado(arg1 as usize),
                // * La UNICA de la tabla que hace trabajo de verdad: lee el
                // archivo entero y le hace el BLAKE3. Por eso se pide a mano y
                // no se calcula al pintar.
                ES_NODO_VERIFICAR => cursor::verify(arg1 as usize),
                // Una pregunta que no existe se contesta con cero y no con un
                // fallo: quien pregunte de mas se entera igual, y un `unsupported`
                // aqui obligaria al panel a distinguir dos formas de "nada".
                _ => 0,
            })
        }
        // `arg0` lleva DOS cosas: el indice en los 32 bits bajos y que texto se
        // pide en los altos. Se reparte el argumento en vez de anadir otra
        // operacion porque son el mismo mecanismo --sacar un nombre de ocho en
        // ocho-- pidiendo dos cosas distintas.
        TASK_OP_ES_TEXTO => {
            use crate::ring0::fsys::estratos::cursor;
            let i = (arg0 & 0xFFFF_FFFF) as usize;
            BmoStatus::ok_value(match arg0 >> 32 {
                ES_TXT_RUTA => cursor::level_name(i, arg1 as usize),
                _ => cursor::child_name(i, arg1 as usize),
            })
        }
        TASK_OP_REINICIAR => {
            crate::ring0::cabina::warn(
                "ring3",
                "reinicio pedido por un proceso de Ring 3",
                scheduler::current_pid() as u64,
            );
            crate::ring0::plat::reinicio::ahora();
        }
        // `arg1` = handle de la consola que el llamante entrega al hijo, o 0
        // para que el hijo escriba en el panel del kernel como siempre. Ese
        // handle es lo que convierte un lanzador en un TERMINAL: a partir de
        // aqui la salida del hijo aterriza en el anillo de quien lo lanzo.
        TASK_OP_EJECUTAR => {
            let _ = arg0;
            let pid = scheduler::current_pid();
            // Se resuelve ANTES de lanzar: si el handle es basura, mejor no
            // haber creado un proceso que despues habla al vacio.
            let consola_idx = if arg1 == 0 {
                None
            } else {
                match cap::resolve(pid, arg1, cap::RIGHT_READ) {
                    Ok(r) if r.kind == cap::KIND_CONSOLE => Some(r.object as usize),
                    Ok(_) => return BmoStatus::err(cap::ERROR_INVALID_HANDLE),
                    Err((code, flags)) => return cap_err((code, flags)),
                }
            };
            let informe = crate::ring0::task::launch::ruta(ruta_tomar(pid));
            match informe.res {
                Ok(tid) => {
                    if let (Some(idx), Some(hijo)) = (consola_idx, informe.pid) {
                        crate::ring0::obj::console::assign_output(hijo, idx);
                    }
                    // * QUIEN lo lanzo, para que el hijo pueda ofrecerle su
                    // superficie. Se apunta AQUI y no dentro de `lanzar.rs`
                    // --donde vive el hermano `package::recordar`-- porque
                    // `launch::ruta` lo comparten este brazo y el shell del
                    // kernel: mirando `current_pid()` desde dentro, un `run`
                    // tecleado por el puerto serie le pondria de padre a la
                    // tarea que estuviera corriendo, tipicamente el compositor.
                    // Aqui el padre es quien hizo la llamada, sin adivinar.
                    if let Some(hijo) = informe.pid {
                        crate::ring0::task::family::recordar(hijo, pid);
                    }
                    BmoStatus::ok_value(tid as u64)
                }
                Err(f) => {
                    crate::ring0::cabina::warn("lanzar", f.motivo(), pid as u64);
                    BmoStatus::err(f.codigo())
                }
            }
        }
        _ => unsupported(),
    }
}

// -- El renglon de ruta --------------------------------------------------
//
// Una ruta se arma a trozos de 8 bytes y se consume entera en `EJECUTAR`. El
// renglon es UNO y lleva el pid de quien lo esta llenando: si empieza a
// escribir otro proceso, lo que hubiera a medias se descarta en vez de
// mezclarse. Dos procesos lanzando a la vez es un caso que hoy no existe --solo
// el compositor tiene la caja-- y cuando exista, media ruta de cada uno seria un
// fallo mucho peor que un lanzamiento perdido.

const RUTA_MAX: usize = 128;
static mut RUTA_BUF: [u8; RUTA_MAX] = [0; RUTA_MAX];
static mut RUTA_N: usize = 0;
static mut RUTA_PID: u32 = u32::MAX;

fn ruta_push(pid: u32, empaquetado: u64) {
    unsafe {
        if RUTA_PID != pid {
            RUTA_PID = pid;
            RUTA_N = 0;
        }
        let buf = &mut *core::ptr::addr_of_mut!(RUTA_BUF);
        for b in empaquetado.to_le_bytes() {
            if b == 0 {
                break;
            }
            if RUTA_N >= RUTA_MAX {
                break;
            }
            buf[RUTA_N] = b;
            RUTA_N += 1;
        }
    }
}

/// La ruta acumulada, y el renglon queda vacio. Devuelve `""` si el que llama
/// no es el que la escribio -- no se lanza la ruta de otro.
fn ruta_tomar(pid: u32) -> &'static str {
    unsafe {
        if RUTA_PID != pid {
            return "";
        }
        let n = RUTA_N;
        RUTA_N = 0;
        RUTA_PID = u32::MAX;
        let buf = &*core::ptr::addr_of!(RUTA_BUF);
        core::str::from_utf8(&buf[..n]).unwrap_or("")
    }
}

/// Synchronous control operations on a resolved capability.
fn invoke_channel(resolved: cap::Resolved, operation: u64) -> BmoStatus {
    let index = resolved.object as usize;
    match operation {
        CHANNEL_OP_GET_SEQ => BmoStatus::ok_value(channel::complete_seq(index)),
        CHANNEL_OP_GET_INDEX => BmoStatus::ok_value(resolved.object),
        _ => unsupported(),
    }
}

/// `INVOKE(capability, operation, a0..a3)` -- the single synchronous door.
fn invoke(frame: &TrapFrame) -> BmoStatus {
    if frame.rdi == CURRENT_TASK {
        // * `frame.r10` y no `rcx`: en SYSCALL el CPU mete ahi el RIP de
        // retorno. Es el mismo motivo por el que el prologo hace `push rcx`.
        return invoke_current_task(frame.rsi, frame.rdx, frame.r10);
    }
    let pid = scheduler::current_pid();
    // Un endpoint se resuelve con WRITE (llamar), no con READ: son derechos
    // distintos y el cliente solo tiene el de llamar.
    if let Ok(r) = cap::resolve(pid, frame.rdi, cap::RIGHT_WRITE) {
        match r.kind {
            cap::KIND_ENDPOINT => {
                // Argumentos: rdi(cap), rsi(op), rdx, r10, r8.
                //
                // * NO rcx. En `SYSCALL` el CPU mete ahi el RIP de retorno --
                // por eso el prologo hace `push rcx` como RIP de usuario-- y
                // r11 se lleva RFLAGS. Un argumento en rcx no es el dato del
                // cliente: es la direccion a la que va a volver. Por eso la
                // convencion salta a r10, igual que en Linux.
                let res = endpoint::call(
                    r.object as usize,
                    frame.rsi,
                    [frame.rdx, frame.r10, frame.r8],
                );
                return BmoStatus { code: res.code, flags: 0, value: res.value };
            }
            cap::KIND_REPLY => {
                let res = endpoint::reply_to(
                    pid, frame.rdi, r.object, frame.rsi as u32, frame.rdx,
                );
                return BmoStatus { code: res.code, flags: 0, value: res.value };
            }
            // ** AVISAR AL CONSUMIDOR: lo que era el syscall numero 1.
            //
            // Va en el bloque de WRITE y no con las otras operaciones del canal
            // --que se resuelven con READ-- porque **avisar es escribir**: mueve
            // el estuario y despierta a quien espera. Quien solo puede leer la
            // secuencia no puede empujarla, y esa diferencia es la que se habria
            // perdido moviendolo al monton de abajo.
            cap::KIND_CHANNEL if frame.rsi == CHANNEL_OP_KICK => {
                let procesados = channel::service(r.object as usize);
                return BmoStatus::ok_value(procesados as u64);
            }
            _ => {}
        }
    }
    match cap::resolve(pid, frame.rdi, cap::RIGHT_READ) {
        Ok(resolved) => match resolved.kind {
            cap::KIND_CHANNEL => invoke_channel(resolved, frame.rsi),
            // La pantalla solo contesta preguntas: donde esta y que forma
            // tiene. Los pixeles no pasan por aqui -- para eso esta mapeada.
            // El raton solo contesta donde esta y que botones tiene. Dibujar
            // el cursor es una decision de aspecto, y eso no es del kernel.
            cap::KIND_INPUT => match crate::ring0::obj::input::operation(frame.rsi) {
                Some(v) => BmoStatus::ok_value(v),
                None => unsupported(),
            },
            // La salida de los hijos de este proceso. Se drena a su ritmo: el
            // kernel no empuja, el terminal tira.
            cap::KIND_CONSOLE => {
                match crate::ring0::obj::console::operation(resolved.object, frame.rsi, frame.rdx) {
                    Some(v) => BmoStatus::ok_value(v),
                    None => unsupported(),
                }
            }
            // Preguntar que hay en el disco. Solo LEER: crear y borrar seran
            // operaciones aparte con su propio derecho, no un efecto lateral
            // de tener el directorio abierto.
            cap::KIND_DIRECTORIO => {
                match crate::ring0::obj::directory::operation(resolved.object, frame.rsi, frame.rdx) {
                    Some(v) => {
                        // ** CERRAR DEVUELVE DOS RECURSOS, NO UNO.
                        //
                        // La ranura del objeto la suelta el modulo; **el handle
                        // lo tiene que soltar aqui**, que es el unico sitio que
                        // lo conoce -- `operation` recibe el indice del objeto,
                        // no el handle con el que se pidio.
                        //
                        // Sin esto el arreglo de la fuga de directorios quedaba
                        // a medias y de la peor manera: las 8 ranuras de
                        // directorio volvian, y los **64 handles por proceso**
                        // no. O sea, el mismo fallo con el contador ocho veces
                        // mas largo -- el tipo de bug que parece arreglado
                        // porque tarda ocho veces mas en aparecer.
                        if frame.rsi == crate::ring0::obj::directory::DIR_OP_CERRAR {
                            cap::revoke(pid, frame.rdi);
                        }
                        BmoStatus::ok_value(v)
                    }
                    None => unsupported(),
                }
            }
            // Mover los bytes de dentro. El MODO (leer o escribir) se fijo al
            // abrir y no es un argumento aqui: pedirle bytes a un archivo de
            // escritura no es un error de permisos, es una pregunta que ese
            // objeto no responde.
            // * LEER UN BLOQUE ENTERO, y por que se despacha AQUI y no dentro
            // de `archivo`: hace falta resolver una SEGUNDA capability --la del
            // bloque de memoria-- y las capabilities viven en este borde.
            //
            // Y esta es la pieza que hacia falta para `fopen`. `ARCH_OP_LEER`
            // da siete bytes por llamada: un WAD de 4 MB serian seiscientas mil
            // llamadas. No se arregla validando punteros de Ring 3 --eso es la
            // infraestructura que `informe.rs` dice que no existe--; se arregla
            // **no necesitandola**: el destino es un bloque que concedio el
            // kernel, asi que comprobar es una resta contra lo que se entrego.
            cap::KIND_ARCHIVO
                if frame.rsi == crate::ring0::obj::file::ARCH_OP_LEER_EN =>
            {
                let pid = scheduler::current_pid();
                // El bloque tiene que ser SUYO y con permiso de escritura: se va
                // a escribir dentro. Que lo diga la capability y no un puntero
                // es la diferencia entera.
                let bloque = match cap::resolve(pid, frame.rdx, cap::RIGHT_WRITE) {
                    Ok(b) if b.kind == cap::KIND_MEMORIA => b,
                    Ok(_) => return unsupported(),
                    Err(err) => return cap_err(err),
                };
                let base = bloque.object;
                let tam = crate::ring0::obj::memory::handed_over_by(pid);
                let desde = frame.r10;
                let cuantos = frame.r8;
                // La unica comprobacion, y cabe en una linea porque el rango lo
                // dimos nosotros. Un desbordamiento en la suma tambien cae aqui.
                if desde.checked_add(cuantos).map_or(true, |fin| fin > tam) {
                    return BmoStatus::err(1);
                }
                // ** Y AQUI SE WRITES POR EL ESPEJO DEL KERNEL, NO POR LA VA
                // DEL PROCESO.
                //
                // Son **la misma memoria**: el bloque se entrego con marcos que
                // el kernel ve tambien por el physmap, y escribir en cualquiera
                // de las dos direcciones escribe en los mismos bytes. La
                // diferencia esta en lo que puede hacer el disco con cada una:
                //
                // | destino | lo que puede hacer el HBA |
                // |---|---|
                // | la VA de Ring 3 | nada: no sabe traducir. Rebota y se copia |
                // | el espejo fisico | **escribir dentro del bloque, directo** |
                //
                // `dev/disk.rs` reconoce el physmap por una RESTA --no preguntando
                // a las tablas de pagina-- asi que un destino de esa ventana se
                // convierte solo en el camino sin rebote. Un lump de DOOM va del
                // plato a su zona de memoria sin pasar por ningun sitio.
                //
                // Si el rango no cae dentro de un bloque conocido, `fisica_de`
                // dice que no y se usa la VA de siempre: correcto y mas lento,
                // que es el orden correcto de las dos cosas.
                let destino = match crate::ring0::obj::memory::fisica_de(pid, base + desde, cuantos)
                {
                    Some(f) => crate::ring0::mm::phys_to_virt(f),
                    None => base + desde,
                };
                let n = unsafe {
                    crate::ring0::obj::file::read_into(
                        resolved.object,
                        destino as *mut u8,
                        cuantos as usize,
                    )
                };
                BmoStatus::ok_value(n as u64)
            }
            // ** ESCRIBIR UN BLOQUE ENTERO -- el espejo del de arriba.
            //
            // La unica diferencia real esta en el derecho que se le pide al
            // bloque: alli `RIGHT_WRITE` porque el kernel escribe DENTRO, aqui
            // `RIGHT_READ` porque solo lo lee. Pedir escritura para leer seria
            // exigir mas autoridad de la que la operacion usa, que es
            // exactamente lo que un sistema de capabilities no debe hacer.
            cap::KIND_ARCHIVO
                if frame.rsi == crate::ring0::obj::file::ARCH_OP_ESCRIBIR_DE =>
            {
                let pid = scheduler::current_pid();
                let bloque = match cap::resolve(pid, frame.rdx, cap::RIGHT_READ) {
                    Ok(b) if b.kind == cap::KIND_MEMORIA => b,
                    Ok(_) => return unsupported(),
                    Err(err) => return cap_err(err),
                };
                let base = bloque.object;
                let tam = crate::ring0::obj::memory::handed_over_by(pid);
                let desde = frame.r10;
                let cuantos = frame.r8;
                if desde.checked_add(cuantos).map_or(true, |fin| fin > tam) {
                    return BmoStatus::err(1);
                }
                let n = unsafe {
                    crate::ring0::obj::file::write_from(
                        resolved.object,
                        (base + desde) as *const u8,
                        cuantos as usize,
                    )
                };
                BmoStatus::ok_value(n as u64)
            }
            cap::KIND_ARCHIVO => {
                match crate::ring0::obj::file::operation(resolved.object, frame.rsi, frame.rdx) {
                    Some(v) => {
                        // Lo mismo que el directorio, y aqui llevaba desde el
                        // principio: `ARCH_OP_CERRAR` soltaba la ranura de las
                        // 16 y dejaba el handle vivo. El compositor cierra
                        // bien, asi que no se notaba -- se notaria a los 64
                        // archivos de una sesion.
                        if frame.rsi == crate::ring0::obj::file::ARCH_OP_CERRAR {
                            cap::revoke(pid, frame.rdi);
                        }
                        BmoStatus::ok_value(v)
                    }
                    None => unsupported(),
                }
            }
            // Un bloque de memoria solo contesta dos cosas: donde esta y
            // cuanto es. Escribir en el no pasa por aqui -- esta MAPEADO, asi
            // que el proceso escribe con un `mov` y el kernel no se entera.
            // Ese es el punto: un syscall por byte seria justo lo contrario
            // de entregar memoria.
            // Lo prestado contesta donde esta y cuanto mide, y **no se escribe
            // por aqui**: esta MAPEADO, asi que el proceso lo toca con un `mov`
            // y el kernel no se entera. Ese es el punto entero de que exista --
            // un syscall por byte seria justo lo contrario de prestar memoria.
            cap::KIND_PRESTADO => {
                match crate::ring0::obj::loan::operation(
                    resolved.object, frame.rsi, scheduler::current_pid(),
                ) {
                    Some(v) => BmoStatus::ok_value(v),
                    None => unsupported(),
                }
            }
            // * OFRECER un trozo del bloque propio. Va aqui y no dentro de
            // `memoria` porque necesita tres argumentos y el espacio de
            // direcciones del que ofrece -- cosas que solo hay en este borde.
            //
            // El bloque ya esta resuelto por SU capability, o sea que es suyo
            // por construccion. La unica comprobacion que queda es que el trozo
            // quepa dentro, y eso es una resta: el rango lo concedio el kernel.
            cap::KIND_MEMORIA if frame.rsi == MEM_OP_OFRECER => {
                let pid = scheduler::current_pid();
                let entregado = crate::ring0::obj::memory::handed_over_by(pid);
                // * El destino llega como TID y no como pid: `ejecutar_en`
                // devuelve un tid, que es lo unico que Ring 3 conoce de un hijo.
                // Traducirlo aqui evita que el userland aprenda un concepto que
                // no usa para nada mas.
                let Some(destino) = scheduler::pid_de(frame.r8 as u32) else {
                    return BmoStatus::ok_value(0);
                };
                let ok = crate::ring0::obj::loan::offer(
                    pid,
                    crate::ring0::mm::vmm::read_cr3(),
                    resolved.object,
                    entregado,
                    frame.rdx,
                    frame.r10,
                    destino,
                );
                BmoStatus::ok_value(ok as u64)
            }
            cap::KIND_MEMORIA => {
                match crate::ring0::obj::memory::operation(
                    resolved.object, frame.rsi, scheduler::current_pid(),
                ) {
                    Some(v) => BmoStatus::ok_value(v),
                    None => unsupported(),
                }
            }
            cap::KIND_FRAMEBUFFER => {
                match crate::ring0::obj::fb::operation(resolved.object, frame.rsi) {
                    Some(v) => BmoStatus::ok_value(v),
                    None => unsupported(),
                }
            }
            // El audio no tiene `object`: la capability no apunta a nada, ES el
            // derecho. Lo que viaja son los argumentos -- frecuencia y duracion.
            cap::KIND_AUDIO => {
                match crate::ring0::obj::audio::operation(frame.rsi, frame.rdx, frame.r10) {
                    Some(v) => BmoStatus::ok_value(v),
                    None => unsupported(),
                }
            }
            _ => unsupported(),
        },
        Err(err) => cap_err(err),
    }
}

/// `WAIT(waitable, observed_sequence, timeout_ns)` -- block until the
/// waitable's sequence moves past `observed_sequence` or the timeout
/// expires (0 = no timeout). `waitable = 0` is a pure timed sleep.
///
/// The observed-sequence compare happens under the scheduler lock, so a
/// kick between the caller's read and this syscall can never be lost.
fn wait(frame: &TrapFrame) -> BmoStatus {
    let timeout_ns = frame.rdx;
    let deadline = if timeout_ns == 0 {
        0
    } else {
        scheduler::rdtsc() + scheduler::ns_to_tsc(timeout_ns)
    };
    if frame.rdi == 0 {
        scheduler::wait_current(0, deadline);
        return BmoStatus::ok_value(0);
    }
    let pid = scheduler::current_pid();
    // El servidor esperando llamadas en su endpoint.
    if let Ok(r) = cap::resolve(pid, frame.rdi, cap::RIGHT_WAIT) {
        if r.kind == cap::KIND_ENDPOINT {
            let res = endpoint::wait_for(r.object as usize, pid, deadline);
            return BmoStatus { code: res.code, flags: 0, value: res.value };
        }
    }
    // ** AQUI NO HAY UN BRAZO PARA `KIND_ARCHIVO`, Y ESO ES UNA DECISION.
    //
    // Se escribio: `wait(handle_de_archivo)` bloqueaba la tarea sobre la clave
    // del disco y la interrupcion la despertaba. Compilaba, y **no esperaba a
    // nada**: traer un trozo (`file::avanzar`) sigue siendo sincrono, asi que
    // cuando la llamada vuelve el dato YA esta. Dormirse despues seria dormirse
    // hasta que otro use el disco.
    //
    // El sitio esta libre y el resto de la cadena existe --el manejador puede
    // llamar a `wake_by_key`, y `wait_current_checked` ya sabe no dormirse si el
    // testigo cambio-- pero le falta la pieza de abajo: que traer el trozo
    // tampoco espere. Mientras eso no exista, este brazo seria una forma cara de
    // volver en el acto.
    match cap::resolve(pid, frame.rdi, cap::RIGHT_WAIT) {
        Ok(resolved) if resolved.kind == cap::KIND_CHANNEL => {
            let index = resolved.object as usize;
            let seq = scheduler::wait_current_checked(
                channel::wait_key(index),
                deadline,
                frame.rsi,
                || channel::complete_seq(index),
            );
            // Advisory: userland re-reads the shared sequence on resume.
            BmoStatus::ok_value(seq)
        }
        Ok(_) => unsupported(),
        Err(err) => cap_err(err),
    }
}

#[unsafe(no_mangle)]
extern "C" fn dispatch(frame: &mut TrapFrame) -> u64 {
    // ** EL METRO. Lo primero y lo ultimo de la funcion a proposito: lo que
    // queda FUERA de estas dos marcas es exactamente el stub de `entry.rs`
    // --pushes, xsave, xrstor, iretq-- y esa resta contra el total que mide
    // `c/coste.bex` desde Ring 3 es lo unico que dice si los ~2600 ciclos estan
    // en el ensamblador o en el Rust. Contestado el 16-08 en el Ryzen: **318
    // aqui dentro, 2345 en el stub**, o sea el 88% fuera. Ver `meter.rs`.
    let __metro = meter::start();
    // Igual que el timer: donde tallo su area este trap y para quien. Un
    // SYSCALL de Ring 3 aterriza en la pila que le haya puesto el planificador,
    // asi que si esa rampa apuntara donde no debe, esto lo ensena.
    crate::ring0::plat::trap::registrar_publicacion(
        crate::ring0::task::percpu::trap_rsp(),
        scheduler::current_tid(),
    );
    // ** DE QUE CLASE ES ESTA PUERTA -- lo que faltaba para saber DONDE se usa.
    //
    // Tres hechos que ya estan en registros --que puerta es, si el handle es
    // `CURRENT_TASK`, y si la operacion es la de consola-- y **ninguna lista**.
    // El coste de cada clase ya estaba medido (~875 / ~1125 / ~2,2 M); lo que no
    // se sabia es cuantas veces se pide cada una, y un coste por vez sin veces
    // por segundo no es un porcentaje.
    //
    // [!] Va DENTRO de la ventana del metro a proposito. Son ~3 ciclos y caen en
    // la fila `DISPATCH`, que es donde vive este codigo. Ponerlo antes de
    // `meter::start` los esconderia en el residuo del stub -- y el residuo es
    // justo el numero que no se puede ensuciar, porque es el unico que dice
    // donde estan los ~570 ciclos todavia sin localizar.
    let clase = match frame.rax as u32 {
        NR_INVOKE if frame.rdi == CURRENT_TASK && frame.rsi == TASK_OP_CONSOLE_WRITE => {
            SYSCALL_CLASS_CONSOLE
        }
        NR_INVOKE if frame.rdi == CURRENT_TASK => SYSCALL_CLASS_TASK,
        NR_INVOKE => SYSCALL_CLASS_HANDLE,
        NR_WAIT => SYSCALL_CLASS_WAIT,
        // La puerta RETIRADA no es ninguna de las cuatro, y no se le inventa una
        // casilla: se deja fuera del histograma a proposito. Lo que falte al
        // sumar las cuatro contra `meter::doors()` es exactamente eso, y esa
        // resta es la comprobacion del instrumento -- si algun dia sale grande,
        // hay trafico que este reparto no esta viendo.
        _ => SYSCALL_CLASS_COUNT,
    };
    meter::count_class(clase as usize);
    let status = match frame.rax as u32 {
        NR_INVOKE => invoke(frame),
        NR_WAIT => wait(frame),
        // ** DOS PUERTAS. El 1 esta RESERVADO y contesta que no existe -- ver
        // `NR_CHANNEL_KICK`. Un binario viejo falla diciendolo, que es lo unico
        // aceptable: reutilizar el numero le haria hacer algo que nadie pidio.
        _ => unsupported(),
    };
    frame.rax = (status.code as u64) | ((status.flags as u64) << 32);
    frame.rdx = status.value;
    let salida = percpu::trap_rsp();
    meter::stop(__metro);
    salida
}
