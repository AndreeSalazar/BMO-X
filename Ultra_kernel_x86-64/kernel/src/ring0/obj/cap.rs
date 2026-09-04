//! Ring 0 Capability Engine (F3).
//!
//! [carril]  ROJO      el motor de capabilities: quien puede que
//!
//! generacion: hijo -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: que significa la operacion que autoriza
//!
//! Per-process capability tables backing the 3-syscall BMO ABI v2 surface.
//! Every kernel object a Ring 3 task can name is reached through a
//! `BmoHandle`-encoded capability: `INVOKE`/`CHANNEL_KICK`/`WAIT` resolve
//! the handle here, validate the generation (anti-UAF by construction) and
//! the rights bitset, and only then dispatch to the object.
//!
//! This is a minimal no-alloc mirror of the canonical `bmo-abi` types
//! (`BmoHandle` layout, `BmoCapSet` bits, `HandleKind` codes). Keeping the
//! values here avoids linking the alloc-using ABI crate into Ring 0;
//! build.ps1 rejects values that drift from bmo-abi.
//!
//! Handle layout (must match `bmo_abi::fundamentals::handle::opaque`):
//! ```text
//!   bit 63        : tag        (0 = recurso, 1 = canal/cola)
//!   bits 62..56   : kind       (7 bits)
//!   bits 55..40   : generation (16 bits -- invalida UAF)
//!   bits 39..0    : index      (40 bits -- slot en la tabla del proceso)
//! ```

use crate::ring0::plat::spin::SpinLock;

pub const MAX_PROCS: usize = 16;
pub const SLOTS_PER_PROC: usize = 64;

// HandleKind codes (mirror of bmo-abi handle/kind.rs).
/// La pantalla. Espejo de `bmo_abi::HandleKind::Framebuffer`.
pub const KIND_FRAMEBUFFER: u8 = 0x0F;
/// **El sonido.** El derecho a hacer ruido. Espejo de
/// `bmo_abi::HandleKind::AudioEngine`.
///
/// Es exclusivo como la pantalla y por el mismo motivo: dos duenos escribiendo
/// en el mismo aparato no es mezclar, es ruido -- y mezclar es trabajo de Ring
/// 3, igual que componer ventanas. Ver `ring0/obj/audio.rs`.
///
/// Va antes que el driver **a proposito**: escribir el motor primero y
/// preguntarse despues quien tiene derecho a usarlo es como se acaba con un
/// sistema en el que cualquier programa pita encima de cualquier otro.
pub const KIND_AUDIO: u8 = 0x10;
/// El raton. Espejo de `bmo_abi::HandleKind::InputDevice`.
pub const KIND_INPUT: u8 = 0x20;
/// La SALIDA de un programa. Cierra la ultima asimetria: la pantalla y la
/// entrada eran capabilities y la consola era un global fijo, asi que un
/// terminal de Ring 3 no podia leer lo que imprimia su propio hijo. Ver
/// `ring0/consola.rs`.
pub const KIND_CONSOLE: u8 = 0x30;
/// Un directorio abierto. Preguntar QUE HAY en el disco. Ver
/// `ring0/directorio.rs`: aqui una ruta abierta no es un nombre que cualquiera
/// pueda escribir, es un handle que a alguien le concedieron.
pub const KIND_DIRECTORIO: u8 = 0x40;
/// Un archivo abierto del volumen de datos. Es el hermano de
/// `KIND_DIRECTORIO`: aquel deja PREGUNTAR que hay, este deja LEER y ESCRIBIR
/// lo que hay dentro. Ver `ring0/archivo.rs`.
///
/// Lectura y escritura NO son dos kinds: son dos modos del mismo objeto, y el
/// modo se fija AL ABRIR. Un handle abierto para leer no escribe aunque se le
/// pida -- no por una comprobacion de permisos, sino porque en ese modo el
/// objeto no tiene donde escribir.
pub const KIND_ARCHIVO: u8 = 0x41;
/// Un bloque de memoria que el proceso PIDIO. Ver `obj::memory`: es memoria
/// entregada entera, no un asignador.
pub const KIND_MEMORIA: u8 = 0x50;
/// Memoria PRESTADA por otro proceso. Ver `obj::loan`.
///
/// No es `KIND_MEMORIA` aunque las dos sean memoria, y la diferencia es la que
/// evita el peor fallo posible: un bloque es **del proceso** y esto es
/// **prestado**. Al morir, uno se libera y el otro **solo se desmapea** -- si se
/// liberara, el kernel entregaria a un tercero la memoria de quien la presto.
pub const KIND_PRESTADO: u8 = 0x51;
pub const KIND_CHANNEL: u8 = 0x60;
/// Endpoint RPC: el derecho a llamar (cliente) o a atender (servidor).
pub const KIND_ENDPOINT: u8 = 0x70;
/// Derecho EFIMERO a responder UNA llamada concreta. Se consume al usarlo.
pub const KIND_REPLY: u8 = 0x71;

/// **UN HIJO QUE YO LANCE.** El objeto es su TID.
///
/// Se concede en `TASK_OP_EJECUTAR`, a quien lanzo -- y por eso cerrar un
/// proceso ajeno no es una autoridad del DIRECTOR: es tener su handle. Los tid
/// no se reciclan (`next_tid` solo sube), asi que un handle viejo nunca acaba
/// nombrando a otro proceso.
/// **UNA VENTANA DE MMIO DE UN APARATO.** El suelo de Ring 3, pieza S1.
///
/// # Por que 0x74 y no un numero cualquiera
///
/// Porque `HANDLE_KIND_MASK` son **7 bits**: un `kind` por encima de `0x7F` no
/// cabe en el handle y se codificaria truncado. Y 0x74 esta libre en las dos
/// tablas --esta y la de `bmo-abi`-- que es lo minimo que se le puede pedir a un
/// numero que vive en dos sitios.
///
/// # Lo que este kind concede HOY, y lo que no
///
/// ```text
///    RIGHT_READ    si    -- la pagina se mapea SOLO LECTURA
///    RIGHT_WRITE   no    -- escribir en un aparato es otra decision, y va
///                           despues de que leer este probado en metal
/// ```
///
/// *** El `object` de la capability es la **direccion virtual** donde quedo, no
/// la fisica. Un proceso no necesita la fisica para leer sus registros, y darsela
/// seria regalar un dato que solo sirve para construir un DMA.
pub const KIND_MMIO: u8 = 0x74;

/// **EL LATIDO DEL HARDWARE.** El suelo de Ring 3, pieza S3.
///
/// El derecho a que `WAIT` despierte **cuando late el reloj**, en vez de cuando
/// se acaba un plazo. Ver `obj/latido.rs`.
///
/// ** No es exclusivo, y esa es la diferencia con la pantalla, el audio y la
/// ventana de un aparato: el reloj no se gasta. Y no concede autoridad -- un
/// proceso ya podia dormir con `WAIT(0, _, timeout)`; lo que cambia es la
/// PRECISION del despertar, no el permiso.
///
/// `0x75` esta libre en esta tabla y en la de `bmo-abi`, que es el peaje 2.
pub const KIND_LATIDO: u8 = 0x75;

/// **[X] ERA `0x80`, Y ESO LO HACIA IMPOSIBLE DE RESOLVER** (2026-08-26).
///
/// `HANDLE_KIND_MASK` son **siete bits**. Con `0x80`:
///
/// ```text
///    encode:   (0x80 & 0x7F) << 56   ->  el campo kind del handle vale 0
///    resolve:  slot.kind (0x80) != kind (0)  ->  ERROR_INVALID_HANDLE
/// ```
///
/// *** No fallaba a veces: **fallaba SIEMPRE.** Todo handle `KIND_TAREA` --el
/// hijo que un proceso lanza, o sea el paso 3 de `PLAN_DIRECTOR.md`-- se
/// rechazaba como invalido en cuanto alguien lo usaba, y el mensaje decia
/// *"handle invalido"*, que manda a mirar al que llama.
///
/// ** Y es exactamente la clase de fallo que este arbol ya tiene fichada: **un
/// numero que no cabe en su campo compila.** El `#GP(0x18)` del 16-08 fue el
/// mismo error con otra forma, y la cabecera de `HANDLE_KIND_SHIFT` lo predijo
/// veinte lineas mas arriba: *"un desplazamiento mal puesto COMPILA, devuelve
/// handle invalido, y se lee como un permiso denegado"*.
///
/// [!] Cambiar este numero **no rompe compatibilidad con nada**: no habia un
/// solo handle de este tipo que funcionara. `0x55` esta libre en esta tabla y en
/// la de `bmo-abi`.
pub const KIND_TAREA: u8 = 0x55;

// -- *** EL PORTICO QUE IMPIDE QUE VUELVA A PASAR --------------------------
//
// Una comprobacion en tiempo de COMPILACION, y no una prueba: no hay donde
// correr una prueba en Ring 0, y este fallo no se ve ejecutando -- se ve cuando
// alguien usa el handle, semanas despues, con un mensaje que manda a otro sitio.
//
// ** El coste de anadir una fila aqui al declarar un `KIND_` nuevo es cinco
// segundos. El de olvidarla ya esta medido: una familia entera de capabilities
// que nunca resolvio.
const _: () = {
    assert!(KIND_FRAMEBUFFER as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_AUDIO as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_INPUT as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_CONSOLE as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_DIRECTORIO as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_ARCHIVO as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_MEMORIA as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_PRESTADO as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_CHANNEL as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_ENDPOINT as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_REPLY as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_MMIO as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_LATIDO as u64 <= HANDLE_KIND_MASK);
    assert!(KIND_TAREA as u64 <= HANDLE_KIND_MASK);
};

// Rights bits (mirror of bmo-abi BmoCap ids: bit N = capability N).
pub const RIGHT_READ: u64 = 1 << 0;
pub const RIGHT_WRITE: u64 = 1 << 1;
pub const RIGHT_WAIT: u64 = 1 << 6;

// Error codes (mirror of bmo-abi status/error.rs).
pub const ERROR_INVALID_HANDLE: u32 = 2;
pub const ERROR_PERMISSION_DENIED: u32 = 3;
/// BmoStatus flag raised alongside `ERROR_PERMISSION_DENIED` when the
/// failure is specifically a missing capability (bmo-abi `NEEDS_CAP`).
pub const FLAG_NEEDS_CAP: u32 = 1 << 4;

#[derive(Clone, Copy)]
struct CapSlot {
    kind: u8,
    live: bool,
    /// Monotonic per-slot counter, never reset: a freed-and-reused slot
    /// issues a new generation, so every stale handle fails to resolve
    /// (anti-UAF by construction). Starts at 1; 0 never resolves.
    generation: u16,
    /// BmoCapSet bits: which operations this capability permits.
    rights: u64,
    /// Kernel object reference (e.g. estuary index for KIND_CHANNEL).
    object: u64,
}

impl CapSlot {
    const FREE: Self = Self { kind: 0, live: false, generation: 0, rights: 0, object: 0 };
}

/// A successfully resolved capability.
#[derive(Clone, Copy)]
pub struct Resolved {
    pub kind: u8,
    pub rights: u64,
    pub object: u64,
}

struct CapTables {
    slots: [[CapSlot; SLOTS_PER_PROC]; MAX_PROCS],
}

static CAP_LOCK: SpinLock = SpinLock::new("cap");
static mut TABLES: CapTables = CapTables {
    slots: [[CapSlot::FREE; SLOTS_PER_PROC]; MAX_PROCS],
};

fn tables() -> &'static mut CapTables {
    unsafe { &mut *core::ptr::addr_of_mut!(TABLES) }
}

/// Active-queue kinds carry tag bit 63 (mirror of HandleKind::tag()).
// -- ** EL FORMATO DEL HANDLE, CON NOMBRE -------------------------------
//
// Estos seis numeros vivian a pelo dentro de `encode` y `resolve`, **y otra vez
// dentro de `platform/abi/.../handle/opaque.rs`**. El comentario de arriba de
// este fichero ni lo disimulaba: dice *"mirror of bmo-abi handle/kind.rs"*.
//
// ** ES LA FORMA EXACTA DEL `#GP(0x18)` DEL 16-08 --el mismo numero en dos
// ficheros que no se hablan-- y aqui habria salido peor: un desplazamiento mal
// puesto COMPILA, devuelve "handle invalido", y se lee como un permiso
// denegado. El otro al menos mataba la maquina.
//
// Con nombre, el guardian de `build.ps1` exige que los dos lados digan lo
// mismo, igual que ya hace con las 49 operaciones y los 63 campos de `OP_INFO`.
// **Un formato es un CONTRATO; escribirlo dos veces es tener dos.**
pub const HANDLE_TAG_SHIFT: u64 = 63;
pub const HANDLE_KIND_SHIFT: u64 = 56;
pub const HANDLE_KIND_MASK: u64 = 0x7F;
pub const HANDLE_GEN_SHIFT: u64 = 40;
pub const HANDLE_GEN_MASK: u64 = 0xFFFF;
pub const HANDLE_INDEX_MASK: u64 = 0x000000FF_FFFFFFFF;

const fn kind_tag(kind: u8) -> u64 {
    match kind {
        KIND_CHANNEL => 1,
        _ => 0,
    }
}

const fn encode(kind: u8, generation: u16, index: u64) -> u64 {
    (kind_tag(kind) << HANDLE_TAG_SHIFT)
        | (((kind as u64) & HANDLE_KIND_MASK) << HANDLE_KIND_SHIFT)
        | (((generation as u64) & HANDLE_GEN_MASK) << HANDLE_GEN_SHIFT)
        | (index & HANDLE_INDEX_MASK)
}

/// Grant a capability to `pid`. Returns the encoded handle, or `None` if
/// the process table is full / pid out of range.
pub fn grant(pid: u32, kind: u8, rights: u64, object: u64) -> Option<u64> {
    let pid = pid as usize;
    if pid >= MAX_PROCS {
        return None;
    }
    let _g = CAP_LOCK.lock();
    let table = &mut tables().slots[pid];
    let index = table.iter().position(|s| !s.live)?;
    // Advance the slot's monotonic generation; skip 0 on wrap so a zeroed
    // or forged handle can never resolve.
    let generation = {
        let g = table[index].generation.wrapping_add(1);
        if g == 0 { 1 } else { g }
    };
    table[index] = CapSlot { kind, live: true, generation, rights, object };
    Some(encode(kind, generation, index as u64))
}

/// Resolve `handle` for `pid`, requiring every bit in `required_rights`.
///
/// Errors mirror bmo-abi: `ERROR_INVALID_HANDLE` for a stale/forged
/// handle, `ERROR_PERMISSION_DENIED` (+`FLAG_NEEDS_CAP`) for missing
/// rights.
pub fn resolve(pid: u32, handle: u64, required_rights: u64) -> Result<Resolved, (u32, u32)> {
    let pid = pid as usize;
    if pid >= MAX_PROCS {
        return Err((ERROR_INVALID_HANDLE, 0));
    }
    let kind = ((handle >> HANDLE_KIND_SHIFT) & HANDLE_KIND_MASK) as u8;
    let generation = ((handle >> HANDLE_GEN_SHIFT) & HANDLE_GEN_MASK) as u16;
    let index = (handle & HANDLE_INDEX_MASK) as usize;
    if index >= SLOTS_PER_PROC || generation == 0 {
        return Err((ERROR_INVALID_HANDLE, 0));
    }
    let _g = CAP_LOCK.lock();
    let slot = tables().slots[pid][index];
    if !slot.live
        || slot.generation != generation
        || slot.kind != kind
        || kind_tag(kind) != (handle >> HANDLE_TAG_SHIFT)
    {
        return Err((ERROR_INVALID_HANDLE, 0));
    }
    if slot.rights & required_rights != required_rights {
        return Err((ERROR_PERMISSION_DENIED, FLAG_NEEDS_CAP));
    }
    Ok(Resolved { kind: slot.kind, rights: slot.rights, object: slot.object })
}

/// Find the pid's existing capability for `(kind, object)`. Used by
/// `TASK_OP_CHANNEL_OPEN` so userland can discover its seeded estuary
/// handles without any out-of-band contract.
pub fn find(pid: u32, kind: u8, object: u64) -> Option<u64> {
    let pid = pid as usize;
    if pid >= MAX_PROCS {
        return None;
    }
    let _g = CAP_LOCK.lock();
    let table = &tables().slots[pid];
    for (index, slot) in table.iter().enumerate() {
        if slot.live && slot.kind == kind && slot.object == object {
            return Some(encode(slot.kind, slot.generation, index as u64));
        }
    }
    None
}

/// Revoke a capability. The generation bump invalidates every copy of the
/// old handle by construction.
pub fn revoke(pid: u32, handle: u64) -> bool {
    let pid = pid as usize;
    if pid >= MAX_PROCS {
        return false;
    }
    let generation = ((handle >> 40) & 0xFFFF) as u16;
    let index = (handle & 0x000000FF_FFFF_FFFF) as usize;
    if index >= SLOTS_PER_PROC || generation == 0 {
        return false;
    }
    let _g = CAP_LOCK.lock();
    let slot = &mut tables().slots[pid][index];
    if !slot.live || slot.generation != generation {
        return false;
    }
    // The generation stays (monotonic); only liveness and rights go.
    slot.live = false;
    slot.rights = 0;
    slot.kind = 0;
    slot.object = 0;
    true
}

/// Drop every capability owned by `pid` (process exit). Generations are
/// preserved so recycled slots keep invalidating stale handles.
/// Revoca todas las capabilities de `pid`.
///
/// Antes de soltar los handles se cierran sus endpoints: si el proceso era
/// servidor, todo el que estuviera esperando su respuesta tiene que despertar
/// con `ERROR_ENDPOINT_DEAD`. Sin esto, matar a un servidor deja a sus
/// clientes bloqueados para siempre -- el fallo que hace inservible cualquier
/// IPC bloqueante.
pub fn revoke_all(pid: u32) {
    // ** EL TESTIGO. Diecisiete estaciones en un orden portante es la forma
    // exacta que produce "uno toca lo que otro ya libero", y la pantalla azul
    // no sabia decir en cual iba. Ver `core::desmontaje`; los numeros de aqui
    // abajo son SU tabla de nombres, asi que mover una linea es mover las dos.
    crate::ring0::core::desmontaje::entra(1, pid);
    // *** LA AUTORIDAD SE OLVIDA LA PRIMERA, y por el mismo motivo que el
    // contador de peticiones dos parrafos mas abajo: **un pid reutilizado
    // heredaria la del muerto.**
    //
    // Y esa es la unica forma que quedaba de colarse: morir el escritorio, que
    // su hueco lo coja un `.bex` cualquiera, y que ese nazca pudiendo reiniciar
    // la maquina. No se ve hasta que la maquina lleva horas encendida.
    crate::ring0::task::autoridad::olvidar(pid);
    crate::ring0::core::desmontaje::entra(2, pid);
    crate::ring0::obj::endpoint::process_died(pid);
    // Si tenia la pantalla, el kernel la recupera aqui. Corre en TODAS las
    // salidas --EXIT voluntario y muerte por fault-- asi que un compositor que
    // revienta no deja la maquina ciega.
    crate::ring0::core::desmontaje::entra(3, pid);
    crate::ring0::obj::fb::process_died(pid);
    crate::ring0::core::desmontaje::entra(4, pid);
    crate::ring0::obj::input::process_died(pid);
    // ** Y la ventana de un aparato, por el mismo motivo exacto. Sin esta linea,
    // un driver de Ring 3 que reventara dejaria su aparato marcado como ocupado
    // **para siempre**, y volver a pedirlo pediria reiniciar la maquina.
    //
    // Es `R-APP6` --*muere sin llevarse a nadie*-- y aqui el "nadie" es el
    // proximo que lo pida. Ver `obj/mmio.rs`.
    crate::ring0::core::desmontaje::entra(5, pid);
    crate::ring0::obj::mmio::process_died(pid);
    // Y el sonido, que ademas hay que CALLAR: un proceso que muere en mitad de
    // un tono deja el bit del altavoz puesto y no queda nadie vivo a quien
    // pedirle que lo quite. Un pitido continuo que solo para reiniciando es la
    // maquina de rehen, igual que el teclado secuestrado, con otro aparato.
    crate::ring0::core::desmontaje::entra(6, pid);
    crate::ring0::obj::audio::process_died(pid);
    // ** Y el bufer que le hubiera prestado al tubo de audio. Sin esto, el
    // aparato seguiria leyendo por DMA marcos de un proceso que ya no existe --
    // que es peor que un fallo: es un ruido que no para y que no tiene dueno a
    // quien pedirle que pare.
    crate::ring0::core::desmontaje::entra(7, pid);
    crate::ring0::dev::usb::audio::soltar(pid);
    // Sus bloques de memoria no hay que desmapearlos --el espacio entero se
    // destruye--, pero SI hay que soltar el contador de peticiones: sin esto un
    // pid reutilizado heredaria las del muerto y no podria pedir nada.
    // * ANTES que la memoria, y el orden NO es indiferente: el reflejo se
    // desmapea del espacio del muerto, y ese espacio tiene que existir todavia.
    // Va primero tambien porque `unmap_page` devuelve el marco sin liberarlo --
    // son del compositor-- y liberarlos aqui seria entregarle el escritorio a
    // otro proceso.
    // ** EL CR3 ES EL DEL QUE MUERE, NO EL ACTIVO.
    //
    // Leia `read_cr3()`, que solo es el correcto cuando el que muere es el que
    // llama -- y hasta el paso 3 del DIRECTOR ese era el unico caso. Con
    // `TAREA_OP_CERRAR` el que llama es el PADRE, asi que `read_cr3()` seria el
    // espacio del padre y `undo` desmapearia paginas del que cierra en vez de
    // las del cerrado. Compila, y se lleva por delante al DIRECTOR.
    crate::ring0::core::desmontaje::entra(8, pid);
    let aspace = crate::ring0::task::scheduler::cr3_de_pid(pid)
        .unwrap_or_else(crate::ring0::mm::vmm::read_cr3);
    crate::ring0::core::desmontaje::entra(9, pid);
    crate::ring0::obj::loan::process_died(pid, aspace);
    crate::ring0::core::desmontaje::entra(10, pid);
    crate::ring0::obj::memory::process_died(pid);
    // Si era el LECTOR de una consola, se libera; si solo escribia en ella, su
    // salida vuelve al panel del kernel. Ver `ring0/consola.rs`.
    crate::ring0::core::desmontaje::entra(11, pid);
    crate::ring0::obj::console::process_died(pid);
    crate::ring0::core::desmontaje::entra(12, pid);
    crate::ring0::obj::directory::process_died(pid);
    // Un archivo de ESCRITURA a medias se descarta: lo acumulado no llega al
    // disco. Guardarlo seria inventar un archivo que su autor nunca dio por
    // terminado, y medio fichero de movimientos es peor que ninguno.
    crate::ring0::core::desmontaje::entra(13, pid);
    crate::ring0::obj::file::process_died(pid);
    // Y de donde salio. Sin esto un pid reutilizado heredaria la ruta del
    // muerto, y `MI_PAQUETE` le entregaria **la imagen de otro programa** -- que
    // ademas cargaria y leeria perfectamente, porque es un `.bex` valido.
    crate::ring0::core::desmontaje::entra(14, pid);
    crate::ring0::task::package::process_died(pid);
    // Y quien lo lanzo. Las dos puntas: sin limpiar las filas donde este pid era
    // el PADRE, un pid reutilizado heredaria los hijos del muerto y `MI_PADRE`
    // mandaria una superficie a un proceso que no la pidio -- que ademas la
    // tomaria sin quejarse, porque el prestamo no sabe que lleva pixeles dentro.
    crate::ring0::core::desmontaje::entra(15, pid);
    crate::ring0::task::family::process_died(pid);
    crate::ring0::core::desmontaje::entra(16, pid);
    let r = revoke_all_slots(pid);
    crate::ring0::core::desmontaje::sale();
    r
}

/// Cuantas capabilities siguen VIVAS a nombre de `pid`.
///
/// **Despues de `revoke_all` tiene que ser CERO.** Y hasta hoy nadie lo
/// comprobaba: la funcion hace su trabajo y el que la llama se fia. Esto es lo
/// que convierte "confio en que revoco" en "revoco, y aqui esta el numero" --
/// el escalon 1 de `docs/plan/PLAN_AUTOCURACION.md`.
pub fn live_count_of(pid: u32) -> u32 {
    let pid = pid as usize;
    if pid >= MAX_PROCS {
        return 0;
    }
    let _g = CAP_LOCK.lock();
    let mut n = 0;
    for slot in &tables().slots[pid] {
        if slot.live {
            n += 1;
        }
    }
    n
}

fn revoke_all_slots(pid: u32) {
    let pid = pid as usize;
    if pid >= MAX_PROCS {
        return;
    }
    let _g = CAP_LOCK.lock();
    for slot in &mut tables().slots[pid] {
        slot.live = false;
        slot.rights = 0;
        slot.kind = 0;
        slot.object = 0;
    }
}

/// Seed the init process: one capability per BMO Channel estuary with
/// full transport rights (READ | WRITE | WAIT).
pub fn seed_init(pid: u32) {
    // * El resultado de `grant` se tiraba. Si la tabla se llena a mitad, el
    // primer proceso del sistema arranca con MENOS canales de los que cree
    // tener, y lo descubre mucho despues: un `INVOKE` sobre un handle que
    // nunca existio, sin nada que lo relacione con este bucle. Una capability
    // que no se concedio es una capability que no esta, y eso se dice aqui.
    let mut fallidas = 0u32;
    for index in 0..boot_context::MAX_CHANNEL_PAGES {
        if grant(
            pid,
            KIND_CHANNEL,
            RIGHT_READ | RIGHT_WRITE | RIGHT_WAIT,
            index as u64,
        )
        .is_none()
        {
            fallidas += 1;
        }
    }
    if fallidas != 0 {
        crate::ring0::cabina::fault("cap", "canales que NO se concedieron a init", fallidas as u64);
    }
}
