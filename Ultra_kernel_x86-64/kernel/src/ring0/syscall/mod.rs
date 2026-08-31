//! x86-64 SYSCALL entry and BMO ABI v2 dispatcher (**2** frozen syscalls).
//!
//! [carril]  ROJO      el despachador de los DOS syscalls congelados
//!
//! generacion: padre -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: que objeto hay detras del handle
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
//! ## *** POR QUE SE PARTIO ESTE FICHERO, Y NO ES POR EL NUMERO (2026-08-24)
//!
//! ** Se partio **para poder optimizar ciclos despues**, y el orden importa:
//! `OPTIMIZACION_MAESTRO.md` pone la optimizacion **la ultima**, detras de
//! CORRECTO y MEDIDO. Este camino ya esta medido, y el numero es el que manda
//! sobre todo el sistema:
//!
//! ```text
//!    la puerta pelada          969 ciclos    (792 ticks = 214 ns)
//!    resolver el handle      + 221
//!    ------------------------------------
//!    antes de hacer NADA    ~1.190 ciclos
//!    el trabajo de OP_INFO      36 ciclos
//! ```
//!
//! **El trabajo es el 3% y la burocracia el 97%**, y esto es lo que paga TODA
//! app de Ring 3 en cada operacion. Es el camino mas caliente que tiene BMO-X.
//!
//! *** Y aqui esta el motivo del reparto: **cuando le llegue el turno a limar
//! ciclos, hay que poder leer el camino entero sin bajar por cuarenta y cinco
//! brazos.** Un despachador de 1.363 lineas se optimiza a ciegas; uno de 974
//! con las familias fuera se lee de una sentada, y **se puede medir por
//! familia** en vez de en bloque.
//!
//! [!] Lo que este reparto NO hace, y hay que decirlo: **no ha quitado un solo
//! ciclo.** Los cuerpos son los mismos y el `match` sigue siendo el mismo
//! `match`; lo unico que cambio es donde vive cada cuerpo. Confundir "ahora se
//! lee" con "ahora es rapido" seria exactamente el error que la ley 0 de la
//! optimizacion existe para evitar.
//!
//! ** Y la burocracia NO SE LIMA, SE EVITA: la via rapida son 58 instrucciones
//! y un Zen 3 retira seis por ciclo -- ni contandolas a una por ciclo se llega
//! a 969. Lo que cuesta son **las dos transiciones de privilegio**, y esas se
//! pagan o no se pagan. Cuando llegue el dia, la mejora sera hacer MENOS
//! operaciones, no operaciones mas baratas.
//!
//! ## [!] Y SIGUEN SIENDO **DOS** SYSCALLS. El reparto no toco eso.
//!
//! `NR_INVOKE` y `NR_WAIT`. Lo que se repartio son las **operaciones** --las
//! filas del `match` de `invoke_current_task`-- que no son syscalls: son lo que
//! se pide POR la puerta. La API crece dentro del par (clase de objeto,
//! operacion) y el ABI no se mueve; ese es el congelamiento entero.
//!
//! *** Y el guardian que lo comprueba **se quedo ciego con este reparto**:
//! leia `ops.rs` y `mod.rs` por su nombre, asi que las operaciones que se
//! fueron a `op_*.rs` dejaron de existir para el. Arreglado el mismo dia --
//! ahora lee la carpeta entera. La leccion ya estaba escrita en `build.ps1`
//! de la vez anterior: *"un guardian que solo mira la mitad da una tranquilidad
//! que no ha ganado"*.
//!
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
use crate::ring0::task::autoridad;
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
/// EL RENGLON DE LOS GESTOS sobre ESTRATOS: crear, borrar, renombrar, carpetas.
/// Salio de aqui por L6a, y el corte se eligio porque el brazo dejo de ser
/// "crear un fichero" el dia que la maquina de abajo aprendio cuatro verbos.
mod gesto;
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

// ** AQUI HABIA UNA SEGUNDA COPIA DE `MEM_OP_OFRECER`, Y VALIA OTRA COSA.
//
// Decia `0x02`. El ABI, el userland y la tabla de `ops.rs` dicen **`0x03`**, y
// esta era la que usaba el despacho -- o sea que el numero que comparaba el
// kernel no era el que mandaba Ring 3. Dos fallos mudos por el precio de uno:
//
//   ofrecer (0x03)   no entraba en su brazo: caia al generico, y `operation`
//                    no lo conoce -> `unsupported`. **Prestar no funcionaba.**
//   bytes   (0x02)   es `MEM_OP_BYTES`, y entraba en el brazo de OFRECER, que
//                    lee `r8` como el tid del destinatario. `entregado()`
//                    contestaba 0 SIEMPRE, y no por estar vacio.
//
// El comentario que la defendia decia que vivia aqui *"y no en la tabla porque
// es una operacion sobre un handle de MEMORIA, no sobre la tarea"*. La frase es
// cierta y la conclusion no: `ops.rs` ya guarda `MEM_OP_*`, `ES_NODO_*` y
// `CHANNEL_OP_*`, que tampoco son de la tarea. Lo que hacia falta no era un
// sitio distinto, era **un solo sitio**.
//
// La constante vive ahora en `ops.rs` como todas, y el guardian de `build.ps1`
// la barre contra el ABI: esto no puede volver a pasar en silencio.


/// **Las tres operaciones que MANDAN sobre la maquina**: nucleos, sello de
/// ESTRATOS y administracion del disco. Ver su cabecera: la medida del estado
/// compartido de este despachador dio CERO, y eso cambio el diagnostico.
mod op_maquina;
/// **Tomar y soltar un aparato exclusivo**: entrada, pantalla y audio. Es la
/// puerta de los LIDERES -- ver `docs/identidad/LIDERES.md`.
mod op_aparato;
/// **Abrir algo y recibir un handle**: directorio, fichero, consola, el propio
/// paquete. Las seis donde el handle ES el permiso.
mod op_abrir;
/// **La consola**: escribir y leer. Sale porque es la unica pareja que habla
/// con una pantalla, y la mas caliente del sistema.
mod op_consola;
/// **Contar lo que el kernel sabe**: CABINA, `info`, klog y la autopsia. Las
/// ocho que preguntan y NO cambian nada.
mod op_contar;

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
        TASK_OP_CONSOLE_WRITE => op_consola::console_write(arg0, arg1),
        TASK_OP_CONSOLE_READ => op_consola::console_read(arg0, arg1),
        TASK_OP_DIR_ABRIR => op_abrir::dir_abrir(arg0, arg1),
        TASK_OP_ARCHIVO_ABRIR => op_abrir::archivo_abrir(arg0, arg1),
        TASK_OP_MI_PAQUETE => op_abrir::mi_paquete(arg0, arg1),
        TASK_OP_ARCHIVO_ASINC => op_abrir::archivo_asinc(arg0, arg1),
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
        TASK_OP_ARCHIVO_CREAR => op_abrir::archivo_crear(arg0, arg1),
        TASK_OP_CONSOLA_CREAR => op_abrir::consola_crear(arg0, arg1),
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
        // **Solo BUSCA** lo concedido al lanzar. Ver `obj/tarea.rs`.
        TASK_OP_HIJO => match crate::ring0::obj::tarea::buscar(scheduler::current_pid(), arg0) {
            Some(handle) => BmoStatus::ok_value(handle),
            None => cap_err((cap::ERROR_PERMISSION_DENIED, cap::FLAG_NEEDS_CAP)),
        },
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
        TASK_OP_INPUT_CLAIM => op_aparato::input_claim(arg0, arg1),
        TASK_OP_FRAMEBUFFER_CLAIM => op_aparato::framebuffer_claim(arg0, arg1),
        TASK_OP_ENTRADA_SOLTAR => op_aparato::entrada_soltar(arg0, arg1),
        TASK_OP_PANTALLA_SOLTAR => op_aparato::pantalla_soltar(arg0, arg1),
        TASK_OP_CABINA_INFO => op_contar::cabina_info(arg0, arg1),
        TASK_OP_CABINA_TEXTO => op_contar::cabina_texto(arg0, arg1),
        TASK_OP_AUDIO_CLAIM => op_aparato::audio_claim(arg0, arg1),
        TASK_OP_AUDIO_RELEASE => op_aparato::audio_release(arg0, arg1),
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
        TASK_OP_INFO => op_contar::info(arg0, arg1),
        TASK_OP_INFO_TEXTO => op_contar::info_texto(arg0, arg1),
        TASK_OP_KLOG_INFO => op_contar::klog_info(arg0, arg1),
        TASK_OP_KLOG_TEXTO => op_contar::klog_texto(arg0, arg1),
        TASK_OP_AUTOPSIA_INFO => op_contar::autopsia_info(arg0, arg1),
        TASK_OP_AUTOPSIA_TEXTO => op_contar::autopsia_texto(arg0, arg1),
        TASK_OP_AUDIO_CENSO => op_aparato::audio_censo(arg0, arg1),
        TASK_OP_APARATO_TOMAR => op_aparato::aparato_tomar(arg0, arg1),
        TASK_OP_APARATO_SOLTAR => op_aparato::aparato_soltar(arg0, arg1),
        TASK_OP_LATIDO_TOMAR => op_aparato::latido_tomar(arg0, arg1),
        // ** Las tres que MANDAN sobre la maquina viven en `op_maquina.rs`.
        // No se fueron por tamano: se fueron porque contestan la misma
        // pregunta, y porque medir el estado compartido de este despachador
        // dio CERO -- ver la cabecera de ese fichero.
        TASK_OP_SMP_DESPERTAR => op_maquina::smp_despertar(arg0, arg1),
        TASK_OP_ESTRATOS_SELLAR => op_maquina::estratos_sellar(arg0, arg1),
        TASK_OP_DISCO => op_maquina::disco(arg0, arg1),
        // ** LOS DOS QUE LE DEVUELVEN AL DUENO SU MAQUINA (2026-08-24).
        //
        // `net rx` y `placa` existian SOLO en el shell de Ring 0, y al shell de
        // Ring 0 no se vuelve. Un camino que solo existe alli es un camino que
        // el dueno de su propia maquina no puede tomar.
        TASK_OP_RED => op_maquina::red(arg0, arg1),
        TASK_OP_PLACA => op_contar::placa(arg0, arg1),
        // ** CREAR UN FICHERO. La primera operacion del sistema que escribe
        // CONTENIDO en el almacen: `sellar` commitea sin datos y el recorte le
        // habla al aparato.
        //
        // El nombre sale del renglon de RUTA --el mismo que usan `EJECUTAR` y
        // los dos de archivo-- y el contenido del suyo, que vive aqui abajo.
        // Inventar un segundo mecanismo para el nombre habria sido tener dos
        // sitios donde se pierde un byte.
        // ** EL RENGLON DE LOS GESTOS: crear, borrar, renombrar, carpetas.
        //
        // Cuatro verbos y una sola maquina debajo, asi que una sola operacion
        // arriba. El despacho vive en `gesto.rs` -- aqui solo se le pasa quien
        // lo pide, que es lo unico que este lado sabe.
        TASK_OP_ES_GESTO => {
            BmoStatus::ok_value(gesto::servir(scheduler::current_pid(), arg0, arg1))
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
                // -- ** El ARBOL: los niveles por los que YA se ha pasado --
                //
                // No leen nada nuevo del disco. Cada nivel se quedo con su
                // listado al pasar por el (`fsys/estratos/nivel.rs`), asi que
                // estas tres contestan de memoria -- que es la condicion para
                // que un panel de arbol se pueda repintar al mover el raton.
                ES_NODO_NIVEL_HIJOS => cursor::nivel_hijos(arg1 as usize),
                ES_NODO_NIVEL_HIJO_TIPO => {
                    cursor::nivel_hijo_tipo((arg1 >> 32) as usize, (arg1 & 0xFFFF_FFFF) as usize)
                }
                ES_NODO_NIVEL_ELEGIDO => cursor::nivel_elegido(arg1 as usize),
                // La UNICA del cursor que toca el disco, y por eso se pide a
                // mano: se manda despues de escribir, no en cada repintado.
                ES_NODO_RECARGAR => cursor::recargar() as u64,
                // -- La HISTORIA. `RELEER` es la unica que lee del disco.
                ES_HIST_RELEER => {
                    crate::ring0::fsys::estratos::historia::releer() as u64
                }
                ES_HIST_CUANTAS => crate::ring0::fsys::estratos::historia::cuantas(),
                ES_HIST_RECORTADA => crate::ring0::fsys::estratos::historia::recortada(),
                ES_HIST_CUANDO => {
                    crate::ring0::fsys::estratos::historia::cuando(arg1 as usize)
                }
                ES_HIST_QUIEN => crate::ring0::fsys::estratos::historia::quien(arg1 as usize),
                ES_HIST_CON_NOMBRE => {
                    crate::ring0::fsys::estratos::historia::con_nombre(arg1 as usize)
                }
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
                // Los bits bajos llevan DOS numeros aqui: `(nivel << 16) | i`.
                // Caben de sobra --como mucho 16 niveles y 64 hijos-- y evitan
                // una tercera puerta para el mismo mecanismo.
                ES_TXT_HIST_NOMBRE => {
                    crate::ring0::fsys::estratos::historia::nombre(i, arg1 as usize)
                }
                ES_TXT_NIVEL_HIJO => {
                    cursor::nivel_child_name((i >> 16) & 0xFFFF, i & 0xFFFF, arg1 as usize)
                }
                _ => cursor::child_name(i, arg1 as usize),
            })
        }
        TASK_OP_REINICIAR => {
            let pid = scheduler::current_pid();
            // *** C4: HASTA HOY ESTO LO PODIA PEDIR CUALQUIERA.
            //
            // No hacia falta un fallo -- bastaba con llamar. Ahora hace falta la
            // AUTORIDAD, que solo se fija al nacer y solo desde Ring 0: ver
            // `task/autoridad.rs`.
            if !autoridad::tiene(pid, autoridad::REINICIAR) {
                crate::ring0::cabina::warn(
                    "ring3",
                    "un proceso SIN autoridad pidio reiniciar la maquina",
                    pid as u64,
                );
                return BmoStatus::err_with_flags(
                    cap::ERROR_PERMISSION_DENIED,
                    cap::FLAG_NEEDS_CAP,
                );
            }
            crate::ring0::cabina::warn(
                "ring3",
                "reinicio pedido por un proceso CON autoridad",
                pid as u64,
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
            // ** La misma puerta que REINICIAR, y por eso la misma llave: el
            // kernel ya lo tenia escrito -- *"las dos operaciones quieren la
            // misma capability el dia que exista"*.
            if !autoridad::tiene(pid, autoridad::LANZAR) {
                crate::ring0::cabina::warn(
                    "ring3",
                    "un proceso SIN autoridad quiso lanzar otro programa",
                    pid as u64,
                );
                return BmoStatus::err_with_flags(
                    cap::ERROR_PERMISSION_DENIED,
                    cap::FLAG_NEEDS_CAP,
                );
            }
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
            // *** NINGUNA. Aqui esta la delegacion, cerrada: un proceso de Ring 3
            // lanza, y lo que lanza NO hereda lo que el tiene. La autoridad no
            // viaja porque no hay ninguna operacion que la mueva.
            let informe = crate::ring0::task::launch::ruta(
                ruta_tomar(pid),
                autoridad::NINGUNA,
            );
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
                    // ** El handle sobre el hijo: la autoridad de cerrarlo
                    // se concede una vez, aqui, a quien lanzo. Ver `obj/tarea.rs`.
                    crate::ring0::obj::tarea::conceder(pid, tid);
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

// -- ** EL RENGLON DEL CONTENIDO, hermano del de la ruta --------------------
//
// Mismo mecanismo y por el mismo motivo --la superficie congelada no acepta
// punteros-- con UNA diferencia que importa: aqui la cuenta es explicita. La
// ruta se corta en el primer cero porque en una ruta un cero no puede aparecer;
// en un fichero **si puede**, y cortarlo ahi seria entregar la mitad.
//
// Y va atado al pid como el de la ruta: dos procesos acumulando a la vez se
// mezclarian el contenido, que aqui significa **escribir en el disco un fichero
// con trozos de otro**.
const DATOS_MAX: usize = 96;
static mut DATOS_BUF: [u8; DATOS_MAX] = [0; DATOS_MAX];
static mut DATOS_N: usize = 0;
static mut DATOS_PID: u32 = u32::MAX;

/// Vacia el renglon del proceso. Se manda ANTES de acumular.
pub(super) fn datos_limpiar(pid: u32) {
    unsafe {
        DATOS_PID = pid;
        DATOS_N = 0;
    }
}

/// Mete `cuantos` de los ocho bytes de `palabra`. Devuelve los que lleva.
pub(super) fn datos_meter(pid: u32, palabra: u64, cuantos: u64) -> usize {
    unsafe {
        if DATOS_PID != pid {
            DATOS_PID = pid;
            DATOS_N = 0;
        }
        let buf = &mut *core::ptr::addr_of_mut!(DATOS_BUF);
        let n = (cuantos as usize).min(8);
        for k in 0..n {
            if DATOS_N >= DATOS_MAX {
                break;
            }
            buf[DATOS_N] = ((palabra >> (k * 8)) & 0xFF) as u8;
            DATOS_N += 1;
        }
        DATOS_N
    }
}

/// Lo acumulado, y **vacia el renglon**: igual que `ruta_tomar`, para que un
/// segundo fichero no herede el contenido del primero.
pub(super) fn datos_tomar(pid: u32) -> &'static [u8] {
    unsafe {
        if DATOS_PID != pid {
            return &[];
        }
        let n = DATOS_N;
        DATOS_N = 0;
        // ** El slice se pide EXPLICITO. Tomar `&(*addr_of!(ARR))[..n]` crea
        // una referencia a la desreferencia de un puntero crudo, y el kernel
        // compila con esa lint en DENY: es la misma forma que ya documenta
        // `AhciDisk::identity` en el driver del disco.
        core::slice::from_raw_parts(core::ptr::addr_of!(DATOS_BUF) as *const u8, n)
    }
}

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
pub(super) fn ruta_tomar(pid: u32) -> &'static str {
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
                // *** EL TAMANO DE **ESTE** BLOQUE, no la suma de los del
                // proceso. Ver `memory::bytes_de_bloque`: comparar contra el
                // total dejaba leer 4 KiB fuera del bloque que el handle
                // autoriza, y si esa VA no estaba mapeada el que fallaba era el
                // KERNEL -- una app sin privilegios tumbando la maquina con dos
                // numeros. Corregido el 2026-08-24.
                let Some(tam) = crate::ring0::obj::memory::bytes_de_bloque(pid, base) else {
                    return BmoStatus::err(1);
                };
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
                // *** Y SI EL BLOQUE NO SE RECONOCE, SE PARA (2026-08-24).
                //
                // ** Aqui habia `None => base + desde`: si `fisica_de` no
                // reconocia el rango, se caia a la **VA de Ring 3** y el kernel
                // escribia por ahi. Funcionaba, y era el ultimo sitio del
                // sistema donde Ring 0 tocaba una direccion de usuario.
                //
                // *** Y desde el arreglo de los limites de la misma manana ese
                // camino ES INALCANZABLE: `bytes_de_bloque` ya garantizo que el
                // rango cae dentro de ESTE bloque, que es justo lo que
                // `fisica_de` comprueba. O sea que el respaldo no protegia de
                // nada -- sostenia un caso que no puede ocurrir.
                //
                // [!] Quitarlo no es cosmetico: es lo que permite encender
                // CR4.SMAP. Un respaldo que nunca se toma sigue siendo una
                // linea que el CPU podria ejecutar.
                let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, base + desde, cuantos)
                else {
                    return BmoStatus::err(1);
                };
                let destino = crate::ring0::mm::phys_to_virt(fisica);
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
                // *** EL TAMANO DE **ESTE** BLOQUE, no la suma de los del
                // proceso. Ver `memory::bytes_de_bloque`: comparar contra el
                // total dejaba leer 4 KiB fuera del bloque que el handle
                // autoriza, y si esa VA no estaba mapeada el que fallaba era el
                // KERNEL -- una app sin privilegios tumbando la maquina con dos
                // numeros. Corregido el 2026-08-24.
                let Some(tam) = crate::ring0::obj::memory::bytes_de_bloque(pid, base) else {
                    return BmoStatus::err(1);
                };
                let desde = frame.r10;
                let cuantos = frame.r8;
                if desde.checked_add(cuantos).map_or(true, |fin| fin > tam) {
                    return BmoStatus::err(1);
                }
                // ** POR EL ESPEJO, igual que el de leer. Este era el caso
                // peor de los dos: no tenia camino de physmap **en absoluto**,
                // siempre dereferenciaba la VA del proceso. Mismo `fisica_de`,
                // misma comprobacion, y el kernel deja de tocar Ring 3.
                let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, base + desde, cuantos)
                else {
                    return BmoStatus::err(1);
                };
                let n = unsafe {
                    crate::ring0::obj::file::write_from(
                        resolved.object,
                        crate::ring0::mm::phys_to_virt(fisica) as *const u8,
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
            // ** UN HIJO QUE ESTE PROCESO LANZO. Tener el handle ES el
            // permiso -- ver `obj/tarea.rs`.
            cap::KIND_TAREA => {
                match crate::ring0::obj::tarea::operation(resolved.object, frame.rsi, frame.rdx) {
                    Some(v) => BmoStatus::ok_value(v),
                    None => unsupported(),
                }
            }
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
            cap::KIND_MMIO => {
                match crate::ring0::obj::mmio::operation(resolved.object, frame.rsi) {
                    Some(v) => BmoStatus::ok_value(v),
                    None => unsupported(),
                }
            }
            cap::KIND_LATIDO => {
                match crate::ring0::obj::latido::operation(frame.rsi) {
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
        // *** S3 DEL SUELO: DORMIR HASTA QUE LATE EL HARDWARE.
        //
        // `frame.rsi` es el testigo que el llamante vio (`LATIDO_OP_CUENTA`), y
        // `wait_current_checked` lo compara **bajo el cerrojo del
        // planificador**, que es el mismo que toma `on_timer` para subirlo. Por
        // eso un latido no se puede colar entre la comparacion y el bloqueo:
        // no es una carrera que se gane casi siempre, es una que no existe.
        //
        // ** Y esto es la pieza que faltaba para bajar un driver a Ring 3. La
        // fuente es el reloj porque es la unica que no pide un aparato -- ver
        // la decision 3 de `docs/plan/PLAN_SUELO_RING3.md`. El dia que la
        // fuente sea la IRQ de una tarjeta, este brazo no cambia: cambia quien
        // llama a `tic()`.
        if r.kind == cap::KIND_LATIDO {
            let visto = frame.rsi;
            let seq = scheduler::wait_current_checked(
                crate::ring0::obj::latido::LLAVE,
                deadline,
                visto,
                crate::ring0::obj::latido::cuenta,
            );
            return BmoStatus::ok_value(seq);
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
