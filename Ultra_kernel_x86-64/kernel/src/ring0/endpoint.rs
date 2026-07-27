//! Endpoint RPC: un proceso Ring 3 puede ser SERVIDOR.
//!
//! Diseño en `platform/abi/bmo-abi/src/ENDPOINT_RPC.md`. Es la pieza que
//! faltaba para F4 (drivers en Ring 3) y F5 (compositor): hasta ahora `INVOKE`
//! solo alcanzaba operaciones implementadas **en el kernel**, así que ningún
//! proceso podía atender a otro.
//!
//! ## La superficie no cambia
//!
//! Siguen siendo tres syscalls. Lo nuevo son dos *kinds* de capability y qué
//! significan `INVOKE` y `WAIT` cuando el handle resuelve a uno de ellos.
//!
//! ## Por qué el mensaje viaja por el estuario
//!
//! `WAIT` devuelve un `BmoStatus` de 16 bytes: no caben la operación, sus
//! argumentos y el handle de respuesta. En vez de inventar un segundo formato
//! de mensaje, la llamada se publica en el **anillo de completions** del
//! estuario que el servidor ya tiene abierto — que es exactamente la forma
//! `(opcode, arg0, arg1, arg2)` que ese anillo transporta. `WAIT` devuelve
//! solo el handle de respuesta.
//!
//! Es el reparto que el diseño ya pedía: *el endpoint lleva el control, el
//! estuario lleva los datos*. Y el kernel escribe esa página **por el
//! physmap**, nunca por la vista de usuario: escribir en memoria de un proceso
//! usando la CR3 equivocada es exactamente el fallo que costó la saga del
//! framebuffer.
//!
//! ## Un solo uso, y por qué importa
//!
//! El derecho a responder es una capability efímera (`KIND_REPLY`) que se
//! consume al usarla. Sin eso, un servidor con un bug podría responder dos
//! veces a la misma llamada —despertando a un proceso que ya siguió su
//! camino— y nadie que no fuera el servidor podría ser distinguido de él.

use crate::ring0::{cap, channel, mm, scheduler};

/// Endpoints simultáneos. Ocho es de sobra para el compositor, el servidor de
/// entrada y un par de drivers; subirlo es cambiar este número.
pub const MAX_ENDPOINTS: usize = 8;
/// Llamadas encoladas por endpoint antes de decir que no.
const COLA: usize = 16;

pub const ERROR_ENDPOINT_DEAD: u32 = 20;
pub const ERROR_BUSY: u32 = 21;

#[derive(Clone, Copy)]
struct Llamada {
    ocupada: bool,
    caller_tid: u32,
    op: u64,
    args: [u64; 3],
}

impl Llamada {
    const VACIA: Llamada = Llamada { ocupada: false, caller_tid: 0, op: 0, args: [0; 3] };
}

#[derive(Clone, Copy)]
struct Endpoint {
    vivo: bool,
    servidor_pid: u32,
    /// Estuario donde se le entregan las llamadas al servidor.
    canal: usize,
    cola: [Llamada; COLA],
    cabeza: usize,
    n: usize,
    /// Sube con cada llamada encolada. Es lo que `WAIT` compara para no
    /// dormirse justo después de que llegue trabajo.
    seq: u64,
}

impl Endpoint {
    const VACIO: Endpoint = Endpoint {
        vivo: false, servidor_pid: 0, canal: 0,
        cola: [Llamada::VACIA; COLA], cabeza: 0, n: 0, seq: 0,
    };
}

static mut ENDPOINTS: [Endpoint; MAX_ENDPOINTS] = [Endpoint::VACIO; MAX_ENDPOINTS];

/// Lo que el servidor dejó para un llamante concreto.
#[derive(Clone, Copy)]
struct Respuesta {
    esperando: bool,
    lista: bool,
    /// Sube en cada llamada del mismo tid: una respuesta de una llamada
    /// anterior llega con la generación vieja y se descarta.
    gen: u32,
    code: u32,
    value: u64,
}

impl Respuesta {
    const VACIA: Respuesta = Respuesta { esperando: false, lista: false, gen: 0, code: 0, value: 0 };
}

static mut RESPUESTAS: [Respuesta; scheduler::MAX_TASKS] = [Respuesta::VACIA; scheduler::MAX_TASKS];

fn eps() -> &'static mut [Endpoint; MAX_ENDPOINTS] {
    unsafe { &mut *core::ptr::addr_of_mut!(ENDPOINTS) }
}
fn resp() -> &'static mut [Respuesta; scheduler::MAX_TASKS] {
    unsafe { &mut *core::ptr::addr_of_mut!(RESPUESTAS) }
}

/// Clave de espera del servidor. Como en los estuarios, una dirección estable
/// y única por objeto — aquí, la del propio endpoint dentro del array.
fn clave_endpoint(idx: usize) -> u64 {
    unsafe { core::ptr::addr_of!(ENDPOINTS) as u64 + (idx * core::mem::size_of::<Endpoint>()) as u64 }
}

/// Clave de espera del llamante: su propia ranura de respuesta.
fn clave_respuesta(tid: u32) -> u64 {
    unsafe { core::ptr::addr_of!(RESPUESTAS) as u64 + (tid as u64 * core::mem::size_of::<Respuesta>() as u64) }
}

// ── Crear ───────────────────────────────────────────────────────────────────

/// Crea un endpoint atendido por `pid` y entregado por el estuario `canal`.
/// Devuelve el handle, o `None` si no quedan ranuras.
pub fn crear(pid: u32, canal: usize) -> Option<u64> {
    let tabla = eps();
    for (i, e) in tabla.iter_mut().enumerate() {
        if e.vivo { continue; }
        *e = Endpoint::VACIO;
        e.vivo = true;
        e.servidor_pid = pid;
        e.canal = canal;
        // El servidor puede esperar en él y responder por él.
        return cap::grant(pid, cap::KIND_ENDPOINT, cap::RIGHT_WAIT | cap::RIGHT_READ, i as u64);
    }
    None
}

/// Concede a `pid` el derecho a LLAMAR a un endpoint ya existente.
///
/// El cliente recibe solo `RIGHT_WRITE`: puede llamar, no puede ponerse a
/// esperar en el endpoint de otro ni responder por él. Los derechos viajan en
/// el handle y solo pueden reducirse.
pub fn conceder_cliente(idx: usize, pid: u32) -> Option<u64> {
    let e = &eps()[idx];
    if !e.vivo { return None; }
    cap::grant(pid, cap::KIND_ENDPOINT, cap::RIGHT_WRITE, idx as u64)
}

// ── Llamar (lado cliente) ───────────────────────────────────────────────────

/// Resultado de una llamada, en la forma que `syscall` devuelve.
pub struct Resultado { pub code: u32, pub value: u64 }

/// `INVOKE` sobre un handle de endpoint: encola, despierta al servidor y
/// **bloquea al llamante** hasta que le respondan.
pub fn llamar(idx: usize, op: u64, args: [u64; 3]) -> Resultado {
    let tid = scheduler::current_tid();
    if tid as usize >= scheduler::MAX_TASKS {
        return Resultado { code: ERROR_ENDPOINT_DEAD, value: 0 };
    }
    {
        let e = &mut eps()[idx];
        if !e.vivo { return Resultado { code: ERROR_ENDPOINT_DEAD, value: 0 }; }
        if e.n >= COLA { return Resultado { code: ERROR_BUSY, value: 0 }; }
        let slot = (e.cabeza + e.n) % COLA;
        e.cola[slot] = Llamada { ocupada: true, caller_tid: tid, op, args };
        e.n += 1;
        e.seq = e.seq.wrapping_add(1);
    }

    // La ranura de respuesta se prepara ANTES de despertar a nadie: si el
    // servidor fuera rapidísimo y respondiera antes de que dejáramos esto
    // listo, su respuesta caería en el vacío.
    let r = &mut resp()[tid as usize];
    r.gen = r.gen.wrapping_add(1);
    r.esperando = true;
    r.lista = false;
    r.code = 0;
    r.value = 0;

    scheduler::wake_by_key(clave_endpoint(idx));

    // Dormir hasta que haya respuesta. El chequeo va DENTRO del lock del
    // scheduler (`wait_current_checked`), que es lo que impide perder el
    // despertar si el servidor contesta entre el "no hay nada" y el "me
    // duermo".
    scheduler::wait_current_checked(
        clave_respuesta(tid),
        0,
        0,
        || if resp()[tid as usize].lista { 1 } else { 0 },
    );

    // ★ Lo que se devuelve AQUI es provisional y casi siempre se pisa.
    //
    // Esta línea se ejecuta ANTES de que el servidor conteste: el bloqueo no
    // cambia de contexto en el sitio. `dispatch` escribirá esto en el frame,
    // y cuando el servidor responda, `escribir_en_frame` lo sobrescribirá con
    // el resultado de verdad — que es lo que el epílogo restaura cuando esta
    // tarea vuelve a correr. Si la respuesta YA estaba lista (el servidor
    // ganó la carrera), se devuelve directamente y no hace falta esperar.
    let r = &mut resp()[tid as usize];
    if r.lista {
        let out = Resultado { code: r.code, value: r.value };
        r.esperando = false;
        r.lista = false;
        return out;
    }
    Resultado { code: 0, value: 0 }
}

// ── Esperar (lado servidor) ─────────────────────────────────────────────────

/// `WAIT` sobre un endpoint: entrega la siguiente llamada y devuelve el handle
/// de respuesta. Si no hay ninguna, bloquea al servidor.
/// ★ **No hace bucle, y ese es el contrato.**
///
/// `wait_current_checked` NO cambia de contexto en el sitio: marca la tarea
/// como bloqueada y elige la siguiente, pero el cambio se consuma en el
/// epílogo del trap. O sea que **vuelve**. Un bucle aquí —"me duermo y
/// recompruebo"— nunca llega al epílogo: re-marca la espera una y otra vez
/// dentro de Ring 0 hasta que la máquina se reinicia. Eso es exactamente lo
/// que hizo la primera versión en hardware.
///
/// El contrato es el mismo que el del canal: si no hay llamada, se deja la
/// espera puesta y se devuelve `value = 0`. Quien reintenta es el servidor
/// desde Ring 3, con otro `WAIT` — y para entonces ya lo habrán despertado.
pub fn esperar(idx: usize, servidor_pid: u32, deadline_tsc: u64) -> Resultado {
    let (op, args, caller_tid, canal, gen) = {
        let e = &mut eps()[idx];
        if !e.vivo { return Resultado { code: ERROR_ENDPOINT_DEAD, value: 0 }; }
        if e.n == 0 {
            let observado = e.seq;
            // Dormir hasta que entre una llamada. El chequeo va dentro del
            // lock del scheduler, así que una llamada que llegue entre el
            // "no hay nada" y el "me duermo" no se pierde.
            scheduler::wait_current_checked(
                clave_endpoint(idx),
                deadline_tsc,
                observado,
                || eps()[idx].seq,
            );
            // value = 0 significa "nada todavía, vuelve a preguntar".
            return Resultado { code: 0, value: 0 };
        }
        let slot = e.cabeza;
        let ll = e.cola[slot];
        e.cola[slot] = Llamada::VACIA;
        e.cabeza = (e.cabeza + 1) % COLA;
        e.n -= 1;
        let g = resp()[ll.caller_tid as usize].gen;
        (ll.op, ll.args, ll.caller_tid, e.canal, g)
    };

    // El mensaje, al anillo del estuario del servidor.
    publicar(canal, op, args);

    // El derecho a responder ESTA llamada, y solo ésta.
    let objeto = ((gen as u64) << 48) | ((idx as u64) << 32) | caller_tid as u64;
    match cap::grant(servidor_pid, cap::KIND_REPLY, cap::RIGHT_WRITE, objeto) {
        Some(h) => Resultado { code: 0, value: h },
        None => {
            // Sin ranura de capability no hay forma de responder: se despierta
            // al llamante con el fallo en vez de dejarlo colgado para siempre.
            completar(caller_tid, gen, ERROR_BUSY, 0);
            Resultado { code: ERROR_BUSY, value: 0 }
        }
    }
}

/// Publica la llamada en el anillo de completions del estuario del servidor.
///
/// Se escribe por el **physmap**, no por la vista de usuario: la CR3 activa
/// aquí es la del proceso que hizo el `WAIT`, y depender de eso sería el mismo
/// error que dejó al hola-mundo muriendo al pintar su salida.
fn publicar(canal: usize, op: u64, args: [u64; 3]) {
    let phys = channel::page_phys(canal);
    if phys == 0 { return; }
    let ch = unsafe { &mut *(mm::phys_to_virt(phys) as *mut bmo_channel::Channel) };
    ch.ring0_complete(op, args[0], args[1], args[2]);
}

// ── Responder (lado servidor) ───────────────────────────────────────────────

/// `INVOKE` sobre un handle de respuesta: despierta al llamante y consume el
/// derecho.
pub fn responder(servidor_pid: u32, handle: u64, objeto: u64, code: u32, value: u64) -> Resultado {
    let caller_tid = (objeto & 0xFFFF_FFFF) as u32;
    let gen = (objeto >> 48) as u32;
    completar(caller_tid, gen, code, value);
    // One-shot: responder lo gasta. Que el handle deje de resolver es lo que
    // hace imposible responder dos veces, sin depender de que el servidor se
    // porte bien.
    cap::revoke(servidor_pid, handle);
    Resultado { code: 0, value: 0 }
}

fn completar(caller_tid: u32, gen: u32, code: u32, value: u64) {
    if caller_tid as usize >= scheduler::MAX_TASKS { return; }
    let r = &mut resp()[caller_tid as usize];
    // La generación descarta una respuesta de una llamada que ya terminó: sin
    // esto, un servidor lento podría despertar al llamante en mitad de su
    // llamada SIGUIENTE.
    if !r.esperando || r.gen != gen || r.lista { return; }
    r.code = code;
    r.value = value;
    r.lista = true;
    // Dos caminos, y entre los dos cubren el ciclo entero:
    //
    // - Si el llamante YA se durmió, su resultado va a su frame guardado, que
    //   es de donde el epílogo lo recogerá al despertarlo.
    // - Si todavía no llegó a dormirse (el servidor ganó la carrera),
    //   `escribir_en_frame` no hace nada —la tarea no está `Blocked`— pero
    //   `r.lista` queda puesto y `llamar` lo lee en el acto, antes de volver.
    //
    // Lo que NO puede pasar es escribir en el contexto de una tarea que está
    // corriendo: ahí `context_rsp` es de la última vez que salió del CPU, y
    // esa dirección ya es de otra cosa.
    escribir_en_frame(caller_tid, code, value);
    scheduler::wake_by_key(clave_respuesta(caller_tid));
}

/// Deja el resultado en el frame GUARDADO del llamante.
///
/// ★ Es lo que el diseño llamaba *"copia status al frame del caller"*, y no es
/// un atajo: es la única forma que funciona. Un syscall que bloquea **no puede
/// calcular su valor de retorno después de bloquearse** —
/// `wait_current_checked` vuelve en el acto y el cambio de contexto se consuma
/// en el epílogo—, así que el código que sigue al bloqueo se ejecuta *antes* de
/// que haya respuesta. Escribirla aquí la deja justo donde el epílogo la va a
/// recoger: el `pop rax` / `pop rdx` que restaura la tarea.
///
/// El layout es el de `trap.rs`: el back-pointer al bloque de GPR vive al
/// final del área de XSAVE.
/// Huella de la ÚLTIMA escritura en un frame ajeno: `[tid, ctx, gpr_base]`.
///
/// El reporter de faults la pinta. Si un contexto se corrompe, esto dice si el
/// RPC escribió —y DÓNDE— justo antes. Comparar `ctx` con el `c=` del switch y
/// `gpr_base` con el `b=` responde de una vez si esta ruta es la culpable, en
/// vez de seguir arreglando a ciegas.
static mut ULTIMA_ESCRITURA: [u64; 3] = [0; 3];

/// Lo último que el RPC escribió en el frame de otra tarea.
pub fn ultima_escritura() -> [u64; 3] { unsafe { ULTIMA_ESCRITURA } }

fn escribir_en_frame(tid: u32, code: u32, value: u64) {
    let ctx = scheduler::context_rsp_of(tid);
    if ctx == 0 {
        unsafe { ULTIMA_ESCRITURA = [tid as u64, 0, 0]; }
        return;
    }
    unsafe {
        let gpr_base = ((ctx + crate::ring0::trap::XSAVE_AREA as u64) as *const u64).read_volatile();
        ULTIMA_ESCRITURA = [tid as u64, ctx, gpr_base];
        if gpr_base == 0 { return; }
        // Un back-pointer sano SIEMPRE está por encima de su área y a menos de
        // una pila de distancia. Si no lo está, el que se corrompió fue el
        // propio back-pointer y escribir ahí solo empeoraría las cosas.
        if gpr_base <= ctx || gpr_base - ctx > 64 * 1024 { return; }
        let frame = &mut *(gpr_base as *mut crate::ring0::trap::TrapFrame);
        frame.rax = code as u64;
        frame.rdx = value;
    }
}

// ── Muerte ──────────────────────────────────────────────────────────────────

/// Un proceso se muere: sus endpoints mueren con él y todo el que estuviera
/// esperando respuesta despierta con `ERROR_ENDPOINT_DEAD`.
///
/// Sin esto, matar a un servidor deja a sus clientes bloqueados para siempre —
/// que es justo el fallo que hace inservible un IPC bloqueante.
pub fn proceso_muerto(pid: u32) {
    for i in 0..MAX_ENDPOINTS {
        let (vivo, servidor) = { let e = &eps()[i]; (e.vivo, e.servidor_pid) };
        if !vivo || servidor != pid { continue; }
        loop {
            let ll = {
                let e = &mut eps()[i];
                if e.n == 0 { break; }
                let slot = e.cabeza;
                let ll = e.cola[slot];
                e.cola[slot] = Llamada::VACIA;
                e.cabeza = (e.cabeza + 1) % COLA;
                e.n -= 1;
                ll
            };
            let gen = resp()[ll.caller_tid as usize].gen;
            completar(ll.caller_tid, gen, ERROR_ENDPOINT_DEAD, 0);
        }
        eps()[i].vivo = false;
        crate::ring0::cabina::warn("endpoint", "servidor muerto: endpoint cerrado", i as u64);
    }
}

/// Cuántos endpoints hay vivos (para el informe del shell).
pub fn vivos() -> usize {
    eps().iter().filter(|e| e.vivo).count()
}

/// Llamadas encoladas en un endpoint, para diagnóstico.
pub fn encoladas(idx: usize) -> usize {
    if idx >= MAX_ENDPOINTS { return 0; }
    eps()[idx].n
}
