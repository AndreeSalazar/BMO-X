//! Endpoint RPC: un proceso Ring 3 puede ser SERVIDOR.
//!
//! Diseno en `platform/abi/bmo-abi/src/ENDPOINT_RPC.md`. Es la pieza que
//! faltaba para F4 (drivers en Ring 3) y F5 (compositor): hasta ahora `INVOKE`
//! solo alcanzaba operaciones implementadas **en el kernel**, asi que ningun
//! proceso podia atender a otro.
//!
//! ## La superficie no cambia
//!
//! Siguen siendo dos syscalls. Lo nuevo son dos *kinds* de capability y que
//! significan `INVOKE` y `WAIT` cuando el handle resuelve a uno de ellos.
//!
//! ## Por que el mensaje viaja por el estuario
//!
//! `WAIT` devuelve un `BmoStatus` de 16 bytes: no caben la operacion, sus
//! argumentos y el handle de respuesta. En vez de inventar un segundo formato
//! de mensaje, la llamada se publica en el **anillo de completions** del
//! estuario que el servidor ya tiene abierto -- que es exactamente la forma
//! `(opcode, arg0, arg1, arg2)` que ese anillo transporta. `WAIT` devuelve
//! solo el handle de respuesta.
//!
//! Es el reparto que el diseno ya pedia: *el endpoint lleva el control, el
//! estuario lleva los datos*. Y el kernel escribe esa pagina **por el
//! physmap**, nunca por la vista de usuario: escribir en memoria de un proceso
//! usando la CR3 equivocada es exactamente el fallo que costo la saga del
//! framebuffer.
//!
//! ## Un solo uso, y por que importa
//!
//! El derecho a responder es una capability efimera (`KIND_REPLY`) que se
//! consume al usarla. Sin eso, un servidor con un bug podria responder dos
//! veces a la misma llamada --despertando a un proceso que ya siguio su
//! camino-- y nadie que no fuera el servidor podria ser distinguido de el.

use crate::ring0::mm;
use crate::ring0::obj::{cap, channel};
use crate::ring0::task::scheduler;

/// Endpoints simultaneos. Ocho es de sobra para el compositor, el servidor de
/// entrada y un par de drivers; subirlo es cambiar este numero.
pub const MAX_ENDPOINTS: usize = 8;
/// Llamadas encoladas por endpoint antes de decir que no.
const QUEUE: usize = 16;

pub const ERROR_ENDPOINT_DEAD: u32 = 20;
pub const ERROR_BUSY: u32 = 21;

#[derive(Clone, Copy)]
struct Call {
    ocupada: bool,
    caller_tid: u32,
    op: u64,
    args: [u64; 3],
}

impl Call {
    const EMPTY_ONE: Call = Call { ocupada: false, caller_tid: 0, op: 0, args: [0; 3] };
}

#[derive(Clone, Copy)]
struct Endpoint {
    vivo: bool,
    servidor_pid: u32,
    /// Estuario donde se le entregan las llamadas al servidor.
    canal: usize,
    cola: [Call; QUEUE],
    cabeza: usize,
    n: usize,
    /// Sube con cada llamada encolada. Es lo que `WAIT` compara para no
    /// dormirse justo despues de que llegue trabajo.
    seq: u64,
}

impl Endpoint {
    const EMPTY: Endpoint = Endpoint {
        vivo: false, servidor_pid: 0, canal: 0,
        cola: [Call::EMPTY_ONE; QUEUE], cabeza: 0, n: 0, seq: 0,
    };
}

static mut ENDPOINTS: [Endpoint; MAX_ENDPOINTS] = [Endpoint::EMPTY; MAX_ENDPOINTS];

/// Lo que el servidor dejo para un llamante concreto.
#[derive(Clone, Copy)]
struct Reply {
    esperando: bool,
    lista: bool,
    /// Sube en cada llamada del mismo tid: una respuesta de una llamada
    /// anterior llega con la generacion vieja y se descarta.
    gen: u32,
    code: u32,
    value: u64,
}

impl Reply {
    const EMPTY_ONE: Reply = Reply { esperando: false, lista: false, gen: 0, code: 0, value: 0 };
}

static mut RESPUESTAS: [Reply; scheduler::MAX_TASKS] = [Reply::EMPTY_ONE; scheduler::MAX_TASKS];

fn eps() -> &'static mut [Endpoint; MAX_ENDPOINTS] {
    unsafe { &mut *core::ptr::addr_of_mut!(ENDPOINTS) }
}
fn resp() -> &'static mut [Reply; scheduler::MAX_TASKS] {
    unsafe { &mut *core::ptr::addr_of_mut!(RESPUESTAS) }
}

/// Clave de espera del servidor. Como en los estuarios, una direccion estable
/// y unica por objeto -- aqui, la del propio endpoint dentro del array.
fn endpoint_key(idx: usize) -> u64 {
    unsafe { core::ptr::addr_of!(ENDPOINTS) as u64 + (idx * core::mem::size_of::<Endpoint>()) as u64 }
}

/// Clave de espera del llamante: su propia ranura de respuesta.
fn reply_key(tid: u32) -> u64 {
    unsafe { core::ptr::addr_of!(RESPUESTAS) as u64 + (tid as u64 * core::mem::size_of::<Reply>() as u64) }
}

// -- Crear -------------------------------------------------------------------

/// Crea un endpoint atendido por `pid` y entregado por el estuario `canal`.
/// Devuelve el handle, o `None` si no quedan ranuras.
pub fn create(pid: u32, canal: usize) -> Option<u64> {
    let tabla = eps();
    for (i, e) in tabla.iter_mut().enumerate() {
        if e.vivo { continue; }
        *e = Endpoint::EMPTY;
        e.vivo = true;
        e.servidor_pid = pid;
        e.canal = canal;
        // El servidor puede esperar en el y responder por el.
        return cap::grant(pid, cap::KIND_ENDPOINT, cap::RIGHT_WAIT | cap::RIGHT_READ, i as u64);
    }
    None
}

/// Concede a `pid` el derecho a LLAMAR a un endpoint ya existente.
///
/// El cliente recibe solo `RIGHT_WRITE`: puede llamar, no puede ponerse a
/// esperar en el endpoint de otro ni responder por el. Los derechos viajan en
/// el handle y solo pueden reducirse.
pub fn grant_client(idx: usize, pid: u32) -> Option<u64> {
    let e = &eps()[idx];
    if !e.vivo { return None; }
    cap::grant(pid, cap::KIND_ENDPOINT, cap::RIGHT_WRITE, idx as u64)
}

// -- Llamar (lado cliente) ---------------------------------------------------

/// Resultado de una llamada, en la forma que `syscall` devuelve.
pub struct Outcome { pub code: u32, pub value: u64 }

/// `INVOKE` sobre un handle de endpoint: encola, despierta al servidor y
/// **bloquea al llamante** hasta que le respondan.
pub fn call(idx: usize, op: u64, args: [u64; 3]) -> Outcome {
    let tid = scheduler::current_tid();
    if tid as usize >= scheduler::MAX_TASKS {
        return Outcome { code: ERROR_ENDPOINT_DEAD, value: 0 };
    }
    {
        let e = &mut eps()[idx];
        if !e.vivo { return Outcome { code: ERROR_ENDPOINT_DEAD, value: 0 }; }
        if e.n >= QUEUE { return Outcome { code: ERROR_BUSY, value: 0 }; }
        let slot = (e.cabeza + e.n) % QUEUE;
        e.cola[slot] = Call { ocupada: true, caller_tid: tid, op, args };
        e.n += 1;
        e.seq = e.seq.wrapping_add(1);
    }

    // La ranura de respuesta se prepara ANTES de despertar a nadie: si el
    // servidor fuera rapidisimo y respondiera antes de que dejaramos esto
    // listo, su respuesta caeria en el vacio.
    let r = &mut resp()[tid as usize];
    r.gen = r.gen.wrapping_add(1);
    r.esperando = true;
    r.lista = false;
    r.code = 0;
    r.value = 0;

    scheduler::wake_by_key(endpoint_key(idx));

    // Dormir hasta que haya respuesta. El chequeo va DENTRO del lock del
    // scheduler (`wait_current_checked`), que es lo que impide perder el
    // despertar si el servidor contesta entre el "no hay nada" y el "me
    // duermo".
    scheduler::wait_current_checked(
        reply_key(tid),
        0,
        0,
        || if resp()[tid as usize].lista { 1 } else { 0 },
    );

    // * Lo que se devuelve AQUI es provisional y casi siempre se pisa.
    //
    // Esta linea se ejecuta ANTES de que el servidor conteste: el bloqueo no
    // cambia de contexto en el sitio. `dispatch` escribira esto en el frame,
    // y cuando el servidor responda, `write_into_frame` lo sobrescribira con
    // el resultado de verdad -- que es lo que el epilogo restaura cuando esta
    // tarea vuelve a correr. Si la respuesta YA estaba lista (el servidor
    // gano la carrera), se devuelve directamente y no hace falta esperar.
    let r = &mut resp()[tid as usize];
    if r.lista {
        let out = Outcome { code: r.code, value: r.value };
        r.esperando = false;
        r.lista = false;
        return out;
    }
    Outcome { code: 0, value: 0 }
}

// -- Esperar (lado servidor) -------------------------------------------------

/// `WAIT` sobre un endpoint: entrega la siguiente llamada y devuelve el handle
/// de respuesta. Si no hay ninguna, bloquea al servidor.
/// * **No hace bucle, y ese es el contrato.**
///
/// `wait_current_checked` NO cambia de contexto en el sitio: marca la tarea
/// como bloqueada y elige la siguiente, pero el cambio se consuma en el
/// epilogo del trap. O sea que **vuelve**. Un bucle aqui --"me duermo y
/// recompruebo"-- nunca llega al epilogo: re-marca la espera una y otra vez
/// dentro de Ring 0 hasta que la maquina se reinicia. Eso es exactamente lo
/// que hizo la primera version en hardware.
///
/// El contrato es el mismo que el del canal: si no hay llamada, se deja la
/// espera puesta y se devuelve `value = 0`. Quien reintenta es el servidor
/// desde Ring 3, con otro `WAIT` -- y para entonces ya lo habran despertado.
pub fn wait_for(idx: usize, servidor_pid: u32, deadline_tsc: u64) -> Outcome {
    let (op, args, caller_tid, canal, gen) = {
        let e = &mut eps()[idx];
        if !e.vivo { return Outcome { code: ERROR_ENDPOINT_DEAD, value: 0 }; }
        if e.n == 0 {
            let observado = e.seq;
            // Dormir hasta que entre una llamada. El chequeo va dentro del
            // lock del scheduler, asi que una llamada que llegue entre el
            // "no hay nada" y el "me duermo" no se pierde.
            scheduler::wait_current_checked(
                endpoint_key(idx),
                deadline_tsc,
                observado,
                || eps()[idx].seq,
            );
            // value = 0 significa "nada todavia, vuelve a preguntar".
            return Outcome { code: 0, value: 0 };
        }
        let slot = e.cabeza;
        let ll = e.cola[slot];
        e.cola[slot] = Call::EMPTY_ONE;
        e.cabeza = (e.cabeza + 1) % QUEUE;
        e.n -= 1;
        let g = resp()[ll.caller_tid as usize].gen;
        (ll.op, ll.args, ll.caller_tid, e.canal, g)
    };

    // El mensaje, al anillo del estuario del servidor.
    publish(canal, op, args);

    // El derecho a responder ESTA llamada, y solo esta.
    let objeto = ((gen as u64) << 48) | ((idx as u64) << 32) | caller_tid as u64;
    match cap::grant(servidor_pid, cap::KIND_REPLY, cap::RIGHT_WRITE, objeto) {
        Some(h) => Outcome { code: 0, value: h },
        None => {
            // Sin ranura de capability no hay forma de responder: se despierta
            // al llamante con el fallo en vez de dejarlo colgado para siempre.
            complete(caller_tid, gen, ERROR_BUSY, 0);
            Outcome { code: ERROR_BUSY, value: 0 }
        }
    }
}

/// Publica la llamada en el anillo de completions del estuario del servidor.
///
/// Se escribe por el **physmap**, no por la vista de usuario: la CR3 activa
/// aqui es la del proceso que hizo el `WAIT`, y depender de eso seria el mismo
/// error que dejo al hola-mundo muriendo al pintar su salida.
fn publish(canal: usize, op: u64, args: [u64; 3]) {
    let phys = channel::page_phys(canal);
    if phys == 0 { return; }
    let ch = unsafe { &mut *(mm::phys_to_virt(phys) as *mut bmo_channel::Channel) };
    ch.ring0_complete(op, args[0], args[1], args[2]);
}

// -- Responder (lado servidor) -----------------------------------------------

/// `INVOKE` sobre un handle de respuesta: despierta al llamante y consume el
/// derecho.
pub fn reply_to(servidor_pid: u32, handle: u64, objeto: u64, code: u32, value: u64) -> Outcome {
    let caller_tid = (objeto & 0xFFFF_FFFF) as u32;
    let gen = (objeto >> 48) as u32;
    complete(caller_tid, gen, code, value);
    // One-shot: responder lo gasta. Que el handle deje de resolver es lo que
    // hace imposible responder dos veces, sin depender de que el servidor se
    // porte bien.
    cap::revoke(servidor_pid, handle);
    Outcome { code: 0, value: 0 }
}

fn complete(caller_tid: u32, gen: u32, code: u32, value: u64) {
    if caller_tid as usize >= scheduler::MAX_TASKS { return; }
    let r = &mut resp()[caller_tid as usize];
    // La generacion descarta una respuesta de una llamada que ya termino: sin
    // esto, un servidor lento podria despertar al llamante en mitad de su
    // llamada SIGUIENTE.
    if !r.esperando || r.gen != gen || r.lista { return; }
    r.code = code;
    r.value = value;
    r.lista = true;
    // Dos caminos, y entre los dos cubren el ciclo entero:
    //
    // - Si el llamante YA se durmio, su resultado va a su frame guardado, que
    //   es de donde el epilogo lo recogera al despertarlo.
    // - Si todavia no llego a dormirse (el servidor gano la carrera),
    //   `write_into_frame` no hace nada --la tarea no esta `Blocked`-- pero
    //   `r.lista` queda puesto y `call` lo lee en el acto, antes de volver.
    //
    // Lo que NO puede pasar es escribir en el contexto de una tarea que esta
    // corriendo: ahi `context_rsp` es de la ultima vez que salio del CPU, y
    // esa direccion ya es de otra cosa.
    write_into_frame(caller_tid, code, value);
    scheduler::wake_by_key(reply_key(caller_tid));
}

/// Deja el resultado en el frame GUARDADO del llamante.
///
/// * Es lo que el diseno llamaba *"copia status al frame del caller"*, y no es
/// un atajo: es la unica forma que funciona. Un syscall que bloquea **no puede
/// calcular su valor de retorno despues de bloquearse** --
/// `wait_current_checked` vuelve en el acto y el cambio de contexto se consuma
/// en el epilogo--, asi que el codigo que sigue al bloqueo se ejecuta *antes* de
/// que haya respuesta. Escribirla aqui la deja justo donde el epilogo la va a
/// recoger: el `pop rax` / `pop rdx` que restaura la tarea.
///
/// El layout es el de `trap.rs`: el back-pointer al bloque de GPR vive al
/// final del area de XSAVE.
/// Huella de la ULTIMA escritura en un frame ajeno: `[tid, ctx, gpr_base]`.
///
/// El reporter de faults la pinta. Si un contexto se corrompe, esto dice si el
/// RPC escribio --y DONDE-- justo antes. Comparar `ctx` con el `c=` del switch y
/// `gpr_base` con el `b=` responde de una vez si esta ruta es la culpable, en
/// vez de seguir arreglando a ciegas.
static mut ULTIMA_ESCRITURA: [u64; 3] = [0; 3];

/// Lo ultimo que el RPC escribio en el frame de otra tarea.
pub fn last_write() -> [u64; 3] { unsafe { ULTIMA_ESCRITURA } }

fn write_into_frame(tid: u32, code: u32, value: u64) {
    let ctx = scheduler::context_rsp_of(tid);
    if ctx == 0 {
        unsafe { ULTIMA_ESCRITURA = [tid as u64, 0, 0]; }
        return;
    }
    unsafe {
        let gpr_base = ((ctx + crate::ring0::plat::trap::XSAVE_AREA as u64) as *const u64).read_volatile();
        ULTIMA_ESCRITURA = [tid as u64, ctx, gpr_base];
        if gpr_base == 0 { return; }
        // Un back-pointer sano SIEMPRE esta por encima de su area y a menos de
        // una pila de distancia. Si no lo esta, el que se corrompio fue el
        // propio back-pointer y escribir ahi solo empeoraria las cosas.
        if gpr_base <= ctx || gpr_base - ctx > 64 * 1024 { return; }
        let frame = &mut *(gpr_base as *mut crate::ring0::plat::trap::TrapFrame);
        frame.rax = code as u64;
        frame.rdx = value;
    }
}

// -- Muerte ------------------------------------------------------------------

/// Un proceso se muere: sus endpoints mueren con el y todo el que estuviera
/// esperando respuesta despierta con `ERROR_ENDPOINT_DEAD`.
///
/// Sin esto, matar a un servidor deja a sus clientes bloqueados para siempre --
/// que es justo el fallo que hace inservible un IPC bloqueante.
pub fn process_died(pid: u32) {
    for i in 0..MAX_ENDPOINTS {
        let (vivo, servidor) = { let e = &eps()[i]; (e.vivo, e.servidor_pid) };
        if !vivo || servidor != pid { continue; }
        loop {
            let ll = {
                let e = &mut eps()[i];
                if e.n == 0 { break; }
                let slot = e.cabeza;
                let ll = e.cola[slot];
                e.cola[slot] = Call::EMPTY_ONE;
                e.cabeza = (e.cabeza + 1) % QUEUE;
                e.n -= 1;
                ll
            };
            let gen = resp()[ll.caller_tid as usize].gen;
            complete(ll.caller_tid, gen, ERROR_ENDPOINT_DEAD, 0);
        }
        eps()[i].vivo = false;
        crate::ring0::cabina::warn("endpoint", "servidor muerto: endpoint cerrado", i as u64);
    }
}

/// Cuantos endpoints hay vivos (para el informe del shell).
pub fn alive() -> usize {
    eps().iter().filter(|e| e.vivo).count()
}

/// Llamadas encoladas en un endpoint, para diagnostico.
pub fn queued(idx: usize) -> usize {
    if idx >= MAX_ENDPOINTS { return 0; }
    eps()[idx].n
}
