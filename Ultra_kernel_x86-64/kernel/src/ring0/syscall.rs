//! x86-64 SYSCALL entry and BMO ABI v2 dispatcher (3 frozen syscalls).
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
//! The surface is frozen at `INVOKE`, `CHANNEL_KICK`, `WAIT`. Everything
//! else is a capability operation resolved through `cap::resolve` -- new
//! functionality adds operations and handle kinds, never syscalls.

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
const NR_INVOKE: u32 = 0x00;
const NR_CHANNEL_KICK: u32 = 0x01;
const NR_WAIT: u32 = 0x02;
const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;
const TASK_OP_GET_PID: u64 = 0x01;
const TASK_OP_GET_TID: u64 = 0x02;
const TASK_OP_YIELD: u64 = 0x03;
const TASK_OP_EXIT: u64 = 0x04;
const TASK_OP_CHANNEL_OPEN: u64 = 0x05;
const TASK_OP_CONSOLE_WRITE: u64 = 0x06;
/// Crea un endpoint atendido por este proceso. arg0 = estuario por el que se
/// le entregaran las llamadas. Devuelve el handle del endpoint.
const TASK_OP_ENDPOINT_CREATE: u64 = 0x07;
/// Pide el derecho a LLAMAR al endpoint `arg0`. Devuelve el handle de cliente.
///
/// Puerta de descubrimiento provisional, con el mismo aviso que lleva
/// `TASK_OP_CONSOLE_WRITE`: hoy cualquier proceso puede pedir cualquier
/// endpoint por su indice, y eso NO es disciplina de capabilities. Existe para
/// arrancar, y muere cuando haya un servicio de nombres que entregue el handle
/// a quien deba tenerlo. Se dice aqui para que nadie lo confunda con el
/// diseno final.
const TASK_OP_ENDPOINT_CONNECT: u64 = 0x08;
/// Reclamar la pantalla. Devuelve un handle `KIND_FRAMEBUFFER` y, con el, el
/// framebuffer mapeado en el espacio del proceso. Ver `ring0/fb.rs`: a partir
/// de aqui el kernel deja de dibujar y el proceso escribe pixeles con `mov`.
const TASK_OP_FRAMEBUFFER_CLAIM: u64 = 0x09;
/// Soltar la pantalla siendo su dueno y **seguir vivo**. Pareja de
/// `FRAMEBUFFER_CLAIM`.
///
/// `0x1D` elegido tras listar los opcodes ORDENADOS, que es la regla desde que
/// `MEMORIA_PEDIR` se puso en `0x12` --ya ocupado por `REINICIAR`-- y pedir
/// memoria habria reiniciado la maquina.
const TASK_OP_PANTALLA_SOLTAR: u64 = 0x1D;
/// Soltar la ENTRADA siendo su dueno y seguir vivo. Pareja de `INPUT_CLAIM`.
///
/// Va con `PANTALLA_SOLTAR` porque el caso de uso es el mismo y **separarlas fue
/// el bug**: prestar la pantalla sin la entrada dejo a `ray.bex` pintando sin
/// poder leer su propio ESC, y a la maquina sin teclado.
const TASK_OP_ENTRADA_SOLTAR: u64 = 0x1E;
/// Reclamar el raton. Devuelve un handle `KIND_INPUT`: el kernel lee el HID,
/// Ring 3 decide que hace con las coordenadas. Ver `ring0/input.rs`.
const TASK_OP_INPUT_CLAIM: u64 = 0x0A;
/// Acumula 8 bytes de ruta (LE, el cero corta) en el renglon del proceso.
///
/// Mismo formato que `TASK_OP_CONSOLE_WRITE`, y por la misma razon: los
/// argumentos van en registros y aqui no hay `copy_from_user`. Pasar un puntero
/// de Ring 3 obligaria al kernel a traducirlo contra el espacio del llamante y
/// a validar que el rango entero es suyo -- infraestructura que no existe todavia
/// y que no se va a improvisar en el camino de lanzar un programa. Ocho bytes
/// por llamada es feo y es seguro; lo segundo importa mas.
const TASK_OP_RUTA: u64 = 0x0B;
/// Lanza lo que se haya acumulado con `TASK_OP_RUTA` y vacia el renglon.
/// Devuelve el tid admitido. Ver `ring0/lanzar.rs` -- el gate de firma es el
/// mismo que el del shell, no una copia.
const TASK_OP_EJECUTAR: u64 = 0x0C;
/// Crea una consola y devuelve su handle de LECTURA. Quien la crea es el
/// terminal: la consola es suya y la drena a su ritmo. Ver `ring0/consola.rs`.
const TASK_OP_CONSOLA_CREAR: u64 = 0x0D;
/// Abre un directorio del volumen de datos y devuelve su handle. La ruta se
/// acumula antes con `TASK_OP_RUTA` -- el MISMO renglon que usa `EJECUTAR`, que
/// es lo que hace que no haga falta un segundo mecanismo para lo mismo.
const TASK_OP_DIR_ABRIR: u64 = 0x0E;
/// LEE de la consola asignada a este proceso. Devuelve `(n << 56) | bytes`.
///
/// La pareja de `TASK_OP_CONSOLE_WRITE`: el hijo escribe por una y escucha por
/// la otra, sobre el MISMO objeto. Es lo que permite un `ACCEPT` en un proceso
/// que no tiene --ni debe tener-- la capability del teclado: el terminal que lo
/// lanzo le pasa lo que se teclea.
const TASK_OP_CONSOLE_READ: u64 = 0x0F;
/// Abre un archivo del volumen de datos para LEER. La ruta se acumula antes
/// con `TASK_OP_RUTA` -- el MISMO renglon que `EJECUTAR` y que `DIR_ABRIR`.
/// Ver `ring0/archivo.rs`.
const TASK_OP_ARCHIVO_ABRIR: u64 = 0x10;
/// Pedir un bloque de memoria. Espejo de `bmo_abi::...::TASK_OP_MEMORIA_PEDIR`
/// -- el drift guard del build comprueba que los dos digan lo mismo.
const TASK_OP_MEMORIA_PEDIR: u64 = 0x15;
/// Igual, pero para ESCRIBIR. Son dos operaciones y no un argumento de modo
/// porque abrir para escribir puede fallar por motivos que abrir para leer no
/// tiene --volumen de solo lectura, nombre que no es 8.3-- y mezclarlas
/// obligaria a devolver errores que no aplican a la mitad de las llamadas.
const TASK_OP_ARCHIVO_CREAR: u64 = 0x11;
/// Reinicia la maquina. No vuelve.
///
/// El reinicio de tres pasos (`0xCF9` -> 8042 -> triple fault) ya existia y solo
/// lo tenia el shell del kernel: la caja de Ring 3 contestaba "no lo conozco" a
/// `reboot`, y la unica salida era el boton. Reiniciar es tocar puertos de E/S,
/// que Ring 3 no puede --ni debe-- hacer; por eso es una operacion y no un
/// permiso ambiental.
///
/// **Limitacion declarada**: hoy no esta atada a una capability, igual que
/// `EJECUTAR`. Cualquier tarea de Ring 3 puede llamarla. Se apunta en CABINA
/// antes de reiniciar para que nunca sea silenciosa, y las dos operaciones
/// quieren la misma capability el dia que exista.
const TASK_OP_REINICIAR: u64 = 0x12;
/// Un dato numerico del sistema (`arg0` = campo) y uno de texto (`arg0` =
/// campo, `arg1` = trozo de 8 bytes). Ver `ring0/core/informe.rs`: leer cuanta
/// RAM hay no es un privilegio, es una pregunta.
const TASK_OP_INFO: u64 = 0x13;
const TASK_OP_INFO_TEXTO: u64 = 0x14;
/// El log del kernel, LEIDO desde Ring 3. `KLOG_INFO` cuantas hay
/// (`arg0` = 0 disponibles, 1 total), `KLOG_TEXTO` ocho bytes de una linea
/// (`arg0` = linea, **0 es la mas reciente**; `arg1` = trozo).
///
/// Mismo criterio que `INFO`: contesta texto y no concede nada. Ver
/// `ring0/core/klog.rs` -- existe porque desde que el escritorio es el arranque,
/// el panel del kernel no se pinta y el log no lo podia leer nadie.
const TASK_OP_KLOG_INFO: u64 = 0x16;
const TASK_OP_KLOG_TEXTO: u64 = 0x17;
/// **La primera operacion de la superficie que ESCRIBE EN EL DISCO.** Cierra
/// una transaccion vacia en ESTRATOS y devuelve la generacion nueva, o 0.
/// Ver `ring0/fsys/estratos.rs::sellar`.
const TASK_OP_ESTRATOS_SELLAR: u64 = 0x18;
/// El CURSOR de ESTRATOS: `arg0` la pregunta, `arg1` su argumento. Y los
/// nombres, de ocho en ocho.
///
/// Dos operaciones y no diez. `INFO_ES_*` ya contestaba *como esta* el almacen;
/// esto contesta **que hay dentro**, que es lo que la ventana de Datos no podia
/// ensenar porque `raiz`, `nodo`, `entradas` y `entrada` eran funciones de
/// Ring 0 sin puerta. Mismo criterio que `INFO` y que el klog: contesta y no
/// concede -- aqui no hay una sola operacion que escriba.
const TASK_OP_ES_NODO: u64 = 0x19;
const TASK_OP_ES_TEXTO: u64 = 0x1A;
/// Despertar los otros nucleos. Espejo de `bmo_abi::...::TASK_OP_SMP_DESPERTAR`.
const TASK_OP_SMP_DESPERTAR: u64 = 0x1B;
/// Tomar lo que otro proceso me haya ofrecido. Espejo de `...::TASK_OP_TOMAR`.
const TASK_OP_TOMAR: u64 = 0x1C;
/// Ofrecer un trozo del bloque propio. Es una operacion sobre `KIND_MEMORIA`.
const MEM_OP_OFRECER: u64 = 0x03;
/// Las preguntas del cursor. Espejo de `bmo_abi::...::ES_NODO_*`.
const ES_NODO_RAIZ: u64 = 0x00;
const ES_NODO_HIJOS: u64 = 0x01;
const ES_NODO_TRUNCADO: u64 = 0x02;
const ES_NODO_HONDO: u64 = 0x03;
const ES_NODO_TIPO: u64 = 0x04;
const ES_NODO_HIJO_TIPO: u64 = 0x05;
const ES_NODO_ENTRAR: u64 = 0x06;
const ES_NODO_SUBIR: u64 = 0x07;
const ES_NODO_HIJO_BYTES: u64 = 0x08;
const ES_NODO_HIJO_ATRIBUTOS: u64 = 0x09;
const ES_NODO_HIJO_FIRMADO: u64 = 0x0A;
const ES_NODO_VERIFICAR: u64 = 0x0B;
/// Que texto pide `ES_TEXTO`, en los bits altos de `arg0`. Espejo de
/// `bmo_abi::...::ES_TXT_*`.
const ES_TXT_RUTA: u64 = 1;
const CHANNEL_OP_GET_SEQ: u64 = 0x01;
const CHANNEL_OP_GET_INDEX: u64 = 0x02;
const ERROR_INVALID_ARGUMENT: u32 = 7;
const ERROR_UNSUPPORTED: u32 = 10;

#[repr(C)]
struct BmoStatus {
    code: u32,
    flags: u32,
    value: u64,
}

impl BmoStatus {
    const fn ok_value(value: u64) -> Self { Self { code: 0, flags: 0, value } }
    const fn err(code: u32) -> Self { Self { code, flags: 0, value: 0 } }
    const fn err_with_flags(code: u32, flags: u32) -> Self { Self { code, flags, value: 0 } }
}

const _: () = assert!(core::mem::size_of::<BmoStatus>() == 16);

const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_SFMASK: u32 = 0xC000_0084;
const RFLAGS_TF: u64 = 1 << 8;
const RFLAGS_IF: u64 = 1 << 9;
const RFLAGS_DF: u64 = 1 << 10;
const RFLAGS_NT: u64 = 1 << 14;
const RFLAGS_AC: u64 = 1 << 18;
const KERNEL_CS: u64 = 0x08;
// Legacy STAR layout; kept armed although the exit path is iretq-only.
const SYSRET_SELECTOR_BASE: u64 = 0x10;

#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() -> ! {
    naked_asm!(
        "swapgs",
        "mov gs:[0x08], rsp",          // stash user RSP
        "mov rsp, gs:[0x00]",          // per-CPU syscall stack
        // Trap tail: ss, rsp, rflags, cs, rip (SYSCALL contract values).
        "push 0x1B",                   // user SS
        "push qword ptr gs:[0x08]",    // user RSP
        "push r11",                    // user RFLAGS
        "push 0x23",                   // user CS
        "push rcx",                    // user RIP
        // 15 GPRs (push order; pops restore the reverse).
        "push rax", "push rcx", "push rdx", "push rbx", "push rbp",
        "push rsi", "push rdi", "push r8", "push r9", "push r10",
        "push r11", "push r12", "push r13", "push r14", "push r15",
        "mov rbp, rsp",
        "sub rsp, {reserva}",
        "and rsp, -64",                // XSAVE exige 64 bytes de alineacion
        // La cabecera a cero ANTES del xsave64: ver el prologo del timer. En
        // corto: `XSAVE` no escribe los 48 bytes reservados de la cabecera y
        // `XRSTOR` da #GP(0) si no son cero. El area se talla sobre la pila,
        // asi que sin esto hereda la basura de lo que hubiera debajo.
        //
        // Incluye el XSTATE_BV de +512: `XSAVE` CONSERVA los bits que caen
        // fuera de `RFBM = EDX:EAX AND XCR0`, asi que la basura de ahi
        // sobrevive al guardado. Ver el prologo del timer.
        "mov qword ptr [rsp+{bv}], 0",
        "mov qword ptr [rsp+{cero}], 0",
        "mov qword ptr [rsp+{cero}+8], 0",
        "mov qword ptr [rsp+{cero}+16], 0",
        "mov qword ptr [rsp+{cero}+24], 0",
        "mov qword ptr [rsp+{cero}+32], 0",
        "mov qword ptr [rsp+{cero}+40], 0",
        "mov qword ptr [rsp+{cero}+48], 0",
        "mov [rsp+{area}], rbp",       // back-pointer to the GPR block
        "mov qword ptr [rsp+{firma}], {magia}", // sello del contexto
        // RFBM = -1: guarda lo que XCR0 tenga habilitado, sea lo que sea.
        // rax y rdx ya estan salvados en el bloque de GPR de arriba.
        "mov eax, -1", "mov edx, -1",
        "xsave64 [rsp]",
        "mov gs:[0x10], rsp",          // publish this context
        "cld",
        "mov rdi, rbp",
        "call {dispatch}",
        // Shared trap epilogue: rax = xsave-base of the context to run.
        "mov rsp, rax",
        "cmp qword ptr [rsp+{firma}], {magia}",
        "jne 3f",
        // La CABECERA, antes de borrar el sello: asi el informe la lee intacta
        // y puede decir de quien era el contexto. rax/rdx se pueden pisar aqui
        // -- los recuperan los pops de abajo.
        "mov rdx, qword ptr [rsp+{bv}]",
        "and rdx, qword ptr [rip+{no_xcr0}]",
        "jnz 8f",
        "mov rax, qword ptr [rsp+{cero}]",
        "or rax, qword ptr [rsp+{cero}+8]",
        "or rax, qword ptr [rsp+{cero}+16]",
        "or rax, qword ptr [rsp+{cero}+24]",
        "or rax, qword ptr [rsp+{cero}+32]",
        "or rax, qword ptr [rsp+{cero}+40]",
        "or rax, qword ptr [rsp+{cero}+48]",
        "jnz 8f",
        // UN SOLO USO: al restaurarlo se borra el sello. Un contexto que ya
        // se consumio no puede volver a pasar por bueno -- si alguien lo
        // intenta, se planta con nombre en vez de reventar en el xrstor.
        "mov qword ptr [rsp+{firma}], 0",
        "mov eax, -1", "mov edx, -1",
        "xrstor64 [rsp]",
        "mov rsp, [rsp+{area}]",
        "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
        "pop r10", "pop r9", "pop r8", "pop rdi", "pop rsi",
        "pop rbp", "pop rbx", "pop rdx", "pop rcx", "pop rax",
        "cmp qword ptr [rsp+8], 0x08", // returning to kernel CS?
        "je 1f",
        "cmp qword ptr [rsp+8], 0x23", // ...o a usuario. Cualquier otra cosa
        "jne 4f",                      // no es un contexto, es basura.
        "swapgs",
        "1: iretq",
        "3: mov rdi, {m_sello}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        "4: mov rdi, {m_cs}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        "8: mov rdi, {m_cab}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        dispatch = sym dispatch,
        podrido = sym crate::ring0::plat::faults::contexto_podrido,
        no_xcr0 = sym crate::ring0::plat::trap::XSAVE_NO_XCR0,
        area = const crate::ring0::plat::trap::XSAVE_AREA,
        firma = const crate::ring0::plat::trap::SELLO_FIRMA,
        magia = const crate::ring0::plat::trap::SELLO_MAGIA,
        bv = const crate::ring0::plat::trap::XSAVE_BV,
        cero = const crate::ring0::plat::trap::XSAVE_CERO_DESDE,
        m_sello = const crate::ring0::plat::faults::PODRIDO_SELLO,
        m_cs = const crate::ring0::plat::faults::PODRIDO_CS,
        m_cab = const crate::ring0::plat::faults::PODRIDO_CABECERA,
        reserva = const crate::ring0::plat::trap::XSAVE_RESERVA,
    );
}

unsafe fn wrmsr(msr: u32, value: u64) {
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") value as u32,
        in("edx") (value >> 32) as u64,
        options(nostack),
    );
}

pub fn init() {
    let star = (SYSRET_SELECTOR_BASE << 48) | (KERNEL_CS << 32);
    unsafe {
        wrmsr(MSR_STAR, star);
        wrmsr(MSR_LSTAR, syscall_entry as *const () as u64);
        // Do not let hostile user flags trigger #DB/#AC before the entry stub
        // has switched away from the user stack. Interrupts stay masked for
        // the whole dispatch; the iretq restores the user IF.
        wrmsr(
            MSR_SFMASK,
            RFLAGS_TF | RFLAGS_IF | RFLAGS_DF | RFLAGS_NT | RFLAGS_AC,
        );
    }
}

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
            match crate::ring0::obj::consola::output_of(pid) {
                Some(idx) => {
                    // Desempaquetar aqui: el anillo guarda bytes, no palabras.
                    // El cero corta, igual que en la consola del kernel.
                    let w = arg0.to_le_bytes();
                    let n = w.iter().position(|&b| b == 0).unwrap_or(8);
                    crate::ring0::obj::consola::write(idx, &w[..n]);
                }
                None => crate::ring0::uconsole::write_packed(arg0),
            }
            BmoStatus::ok_value(0)
        }
        TASK_OP_CONSOLE_READ => {
            let _ = arg0;
            let pid = scheduler::current_pid();
            match crate::ring0::obj::consola::output_of(pid) {
                Some(idx) => BmoStatus::ok_value(crate::ring0::obj::consola::read_entry(idx)),
                // Sin consola asignada no hay de donde leer. Cero = "nada", no
                // error: un programa que sondea no debe morir por preguntar.
                None => BmoStatus::ok_value(0),
            }
        }
        TASK_OP_DIR_ABRIR => {
            let _ = arg0;
            let pid = scheduler::current_pid();
            let ruta = ruta_tomar(pid);
            match crate::ring0::obj::directorio::open(pid, ruta) {
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
            match crate::ring0::obj::archivo::open(pid, ruta) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        TASK_OP_ARCHIVO_CREAR => {
            let _ = arg0;
            let pid = scheduler::current_pid();
            let ruta = ruta_tomar(pid);
            match crate::ring0::obj::archivo::create(pid, ruta) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        TASK_OP_CONSOLA_CREAR => {
            let _ = arg0;
            match crate::ring0::obj::consola::create(scheduler::current_pid()) {
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
        // * Pedir memoria. Mismo comentario de CR3 que el framebuffer: durante
        // el syscall sigue cargado el espacio del llamante, que es justo donde
        // hay que mapear.
        TASK_OP_MEMORIA_PEDIR => {
            match crate::ring0::obj::memoria::request(
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
            BmoStatus::ok_value(crate::ring0::core::informe::campo(arg0))
        }
        TASK_OP_INFO_TEXTO => {
            BmoStatus::ok_value(crate::ring0::core::informe::texto(arg0, arg1))
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
        TASK_OP_SMP_DESPERTAR => {
            use crate::ring0::plat::smp::{self, obra};
            let cuantos = if arg0 > u32::MAX as u64 { u32::MAX } else { arg0 as u32 };
            match arg1 {
                // Desactivar: los obreros vuelven a `hlt` y ahi se quedan.
                1 => {
                    obra::parar();
                    crate::ring0::core::phase::dashboard_log("[smp] obreros PARADOS");
                    BmoStatus::ok_value(0)
                }
                // La prueba. Devuelve la aceleracion x100 --`842` son 8,42x--
                // porque por la puerta solo cabe un numero y una fraccion no
                // se puede mandar entera. El detalle en crudo va a CABINA.
                2 => {
                    let (alive, _) = smp::alive();
                    let (uno, todos, partes) = obra::prueba(alive);
                    crate::ring0::cabina::info("smp", "ticks con UN nucleo", uno);
                    crate::ring0::cabina::info("smp", "ticks con todos", todos);
                    crate::ring0::cabina::info("smp", "partes que corrieron", partes as u64);
                    crate::ring0::core::phase::dashboard_log("[smp] prueba de reparto hecha");
                    if todos > 0 && partes > 0 {
                        BmoStatus::ok_value(uno.saturating_mul(100) / todos)
                    } else {
                        BmoStatus::ok_value(0)
                    }
                }
                _ => {
                    let (alive, esperados) = smp::despertar(cuantos, |_| {});
                    BmoStatus::ok_value(((alive as u64) << 32) | esperados as u64)
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
            match crate::ring0::fsys::estratos::sellar() {
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
                ES_NODO_VERIFICAR => cursor::verificar(arg1 as usize),
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
                ES_TXT_RUTA => cursor::nombre_nivel(i, arg1 as usize),
                _ => cursor::hijo_nombre(i, arg1 as usize),
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
            let informe = crate::ring0::task::lanzar::ruta(ruta_tomar(pid));
            match informe.res {
                Ok(tid) => {
                    if let (Some(idx), Some(hijo)) = (consola_idx, informe.pid) {
                        crate::ring0::obj::consola::assign_output(hijo, idx);
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
                match crate::ring0::obj::consola::operation(resolved.object, frame.rsi, frame.rdx) {
                    Some(v) => BmoStatus::ok_value(v),
                    None => unsupported(),
                }
            }
            // Preguntar que hay en el disco. Solo LEER: crear y borrar seran
            // operaciones aparte con su propio derecho, no un efecto lateral
            // de tener el directorio abierto.
            cap::KIND_DIRECTORIO => {
                match crate::ring0::obj::directorio::operation(resolved.object, frame.rsi, frame.rdx) {
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
                        if frame.rsi == crate::ring0::obj::directorio::DIR_OP_CERRAR {
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
                if frame.rsi == crate::ring0::obj::archivo::ARCH_OP_LEER_EN =>
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
                let tam = crate::ring0::obj::memoria::handed_over_by(pid);
                let desde = frame.r10;
                let cuantos = frame.r8;
                // La unica comprobacion, y cabe en una linea porque el rango lo
                // dimos nosotros. Un desbordamiento en la suma tambien cae aqui.
                if desde.checked_add(cuantos).map_or(true, |fin| fin > tam) {
                    return BmoStatus::err(1);
                }
                let n = unsafe {
                    crate::ring0::obj::archivo::read_into(
                        resolved.object,
                        (base + desde) as *mut u8,
                        cuantos as usize,
                    )
                };
                BmoStatus::ok_value(n as u64)
            }
            cap::KIND_ARCHIVO => {
                match crate::ring0::obj::archivo::operation(resolved.object, frame.rsi, frame.rdx) {
                    Some(v) => {
                        // Lo mismo que el directorio, y aqui llevaba desde el
                        // principio: `ARCH_OP_CERRAR` soltaba la ranura de las
                        // 16 y dejaba el handle vivo. El compositor cierra
                        // bien, asi que no se notaba -- se notaria a los 64
                        // archivos de una sesion.
                        if frame.rsi == crate::ring0::obj::archivo::ARCH_OP_CERRAR {
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
                let entregado = crate::ring0::obj::memoria::handed_over_by(pid);
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
                match crate::ring0::obj::memoria::operation(
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
            _ => unsupported(),
        },
        Err(err) => cap_err(err),
    }
}

/// `CHANNEL_KICK(capability, published_sequence)` -- notify the consumer.
/// Services the estuary with the per-kick budget and wakes its waiters.
fn channel_kick(frame: &TrapFrame) -> BmoStatus {
    let pid = scheduler::current_pid();
    match cap::resolve(pid, frame.rdi, cap::RIGHT_WRITE) {
        Ok(resolved) if resolved.kind == cap::KIND_CHANNEL => {
            let processed = channel::service(resolved.object as usize);
            BmoStatus::ok_value(processed as u64)
        }
        Ok(_) => unsupported(),
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
    // Igual que el timer: donde tallo su area este trap y para quien. Un
    // SYSCALL de Ring 3 aterriza en la pila que le haya puesto el planificador,
    // asi que si esa rampa apuntara donde no debe, esto lo ensena.
    crate::ring0::plat::trap::registrar_publicacion(
        crate::ring0::task::percpu::trap_rsp(),
        scheduler::current_tid(),
    );
    let status = match frame.rax as u32 {
        NR_INVOKE => invoke(frame),
        NR_CHANNEL_KICK => channel_kick(frame),
        NR_WAIT => wait(frame),
        _ => unsupported(),
    };
    frame.rax = (status.code as u64) | ((status.flags as u64) << 32);
    frame.rdx = status.value;
    percpu::trap_rsp()
}
