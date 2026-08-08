//! Ring 3 bootstrap console.
//!
//! The frozen syscall surface exposes no "print" call -- text output is a
//! capability operation. Until a real display-server estuary exists, the
//! first Ring 3 program needs *some* auditable door to prove the CPL3->CPL0
//! path visually. `INVOKE(CURRENT_TASK, CONSOLE_WRITE, packed)` is that door
//! (see `syscall::invoke_current_task`): the kernel receives up to 8 bytes
//! packed little-endian per call and renders them here.
//!
//! Semantics: bytes accumulate into a line buffer; `\n` (or a full buffer)
//! flushes the line to serial and the on-screen kernel-log panel, tagged so
//! it is unmistakably Ring 3 output. There is no way for the caller to reach
//! any memory but this kernel-owned surface -- it hands over bytes by value,
//! never a pointer.
//!
//! Single-core, and syscall dispatch runs with interrupts masked, so the
//! line buffer needs no lock: a CONSOLE_WRITE cannot be preempted mid-flush.
//!
//! # Una linea por proceso
//!
//! Un solo buffer bastaba con un unico programa Ring 3. Con VARIOS (el
//! demo en ensamblador, el de C y el de COBOL corren a la vez), el timer
//! puede cambiar de tarea entre dos CONSOLE_WRITE de la misma linea: los
//! textos se entrelazarian a media palabra y la pantalla seria ilegible.
//! Por eso el buffer esta indexado por PID. La etiqueta tambien: asi el
//! log dice de quien es cada linea sin que el programa tenga que decirlo.

use crate::ring0::dev::console::serial_write_byte;

const LINE_MAX: usize = 96;
/// Procesos Ring 3 con linea propia. Un PID mayor comparte el slot 0, que
/// es degradacion aceptable: se mezcla, pero nunca se pierde texto.
const MAX_PROCS: usize = 8;
const TAG_MAX: usize = 12;

static mut LINE: [[u8; LINE_MAX]; MAX_PROCS] = [[0u8; LINE_MAX]; MAX_PROCS];
static mut LEN: [usize; MAX_PROCS] = [0usize; MAX_PROCS];
/// Etiqueta por proceso, con su longitud. Sin registrar, se usa "ring3".
static mut TAG: [[u8; TAG_MAX]; MAX_PROCS] = [[0u8; TAG_MAX]; MAX_PROCS];
static mut TAG_LEN: [usize; MAX_PROCS] = [0usize; MAX_PROCS];

/// Slot de linea del proceso que esta ejecutando el syscall.
fn slot() -> usize {
    let pid = crate::ring0::task::scheduler::current_pid() as usize;
    if pid < MAX_PROCS { pid } else { 0 }
}

/// Registra con que nombre apareceran las lineas de `pid` en el log.
///
/// Lo llama `proc::admit_payload` al admitir cada programa, para que el
/// kernel log distinga "C>" de "COBOL>" sin que los programas colaboren.
pub fn set_tag(pid: u32, name: &str) {
    let slot = pid as usize;
    if slot >= MAX_PROCS {
        return;
    }
    let bytes = name.as_bytes();
    let n = if bytes.len() > TAG_MAX { TAG_MAX } else { bytes.len() };
    unsafe {
        TAG[slot][..n].copy_from_slice(&bytes[..n]);
        TAG_LEN[slot] = n;
    }
}

// Telemetry for the live dashboard heartbeat: how many CONSOLE_WRITE words
// arrived from CPL3 and how many lines were flushed to the log. Nonzero rx
// = the syscall path from Ring 3 has fired at least once.
static mut RX_WORDS: u64 = 0;
static mut FLUSHED: u64 = 0;

/// `(words_received, lines_flushed)` since boot.
pub fn stats() -> (u64, u64) {
    unsafe { (RX_WORDS, FLUSHED) }
}

/// Lineas que ha escrito CADA proceso. El contador global dice que Ring 3
/// hablo; este dice QUIEN hablo y cuanto -- que es lo que hace falta para
/// mirar una tabla de programas y saber cual hizo su trabajo.
static mut LINES_BY_PID: [u32; MAX_PROCS] = [0; MAX_PROCS];

/// Lineas escritas por `pid` desde el arranque.
pub fn lines_of(pid: u32) -> u32 {
    let slot = pid as usize;
    if slot >= MAX_PROCS { return 0; }
    unsafe { LINES_BY_PID[slot] }
}

// -- * LAS ULTIMAS PALABRAS ----------------------------------------------
//
// Lo que cada proceso dijo justo antes de morir, guardado para DESPUES de que
// muera.
//
// El compositor se moria al arrancar y su manejador de panico decia el archivo
// y la linea exactos... **al log del kernel**, que sigue corriendo. Para cuando
// se miraba la pantalla, ese mensaje ya habia subido y salido, y lo unico que
// quedaba era un shell donde deberia haber un escritorio. Tres arranques
// seguidos con la respuesta delante y nadie pudo leerla.
//
// Un registrador de vuelo que borra la caja negra al aterrizar no es un
// registrador de vuelo. Estas cuatro lineas por proceso sobreviven a su dueno y
// se imprimen cuando hace falta -- que es justo cuando ya no se le puede
// preguntar a el.
const ULTIMAS: usize = 4;
static mut COLA: [[[u8; LINE_MAX]; ULTIMAS]; MAX_PROCS] = [[[0u8; LINE_MAX]; ULTIMAS]; MAX_PROCS];
static mut COLA_LEN: [[usize; ULTIMAS]; MAX_PROCS] = [[0usize; ULTIMAS]; MAX_PROCS];
/// Donde va la siguiente. Es un anillo: se queda con las ULTIMAS, que son las
/// que dicen por que se murio -- las primeras dicen que arranco, y eso ya se vio.
static mut COLA_PUNTA: [usize; MAX_PROCS] = [0; MAX_PROCS];

fn recordar(slot: usize, linea: &[u8]) {
    unsafe {
        let punta = COLA_PUNTA[slot];
        let n = if linea.len() > LINE_MAX { LINE_MAX } else { linea.len() };
        COLA[slot][punta][..n].copy_from_slice(&linea[..n]);
        COLA_LEN[slot][punta] = n;
        COLA_PUNTA[slot] = (punta + 1) % ULTIMAS;
    }
}

/// Las ultimas lineas que dijo `pid`, de la mas vieja a la mas nueva.
///
/// Se entrega por callback y no como slice para no prestar un `static mut`:
/// quien las lee las pinta y se acabo.
pub fn ultimas_palabras(pid: u32, mut pinta: impl FnMut(&str)) {
    let slot = pid as usize;
    if slot >= MAX_PROCS {
        return;
    }
    unsafe {
        let punta = COLA_PUNTA[slot];
        for i in 0..ULTIMAS {
            let idx = (punta + i) % ULTIMAS;
            let n = COLA_LEN[slot][idx];
            if n == 0 {
                continue;
            }
            if let Ok(s) = core::str::from_utf8(&COLA[slot][idx][..n]) {
                pinta(s);
            }
        }
    }
}

/// Dijo algo este proceso alguna vez? Distingue "murio callado" --que ya es un
/// dato-- de "no hay nada guardado".
pub fn hubo_palabras(pid: u32) -> bool {
    let slot = pid as usize;
    if slot >= MAX_PROCS {
        return false;
    }
    unsafe { COLA_LEN[slot].iter().any(|&n| n > 0) }
}

/// Emit up to 8 bytes packed little-endian in `packed`. A zero byte ends the
/// word early (lets a short final chunk be zero-padded by the producer).
pub fn write_packed(packed: u64) {
    unsafe {
        RX_WORDS += 1;
        // La PRIMERA palabra que cruza CPL3->CPL0 por esta puerta: el instante
        // en que el userspace habla. Se graba aqui, en el syscall mismo, no se
        // deduce despues mirando el contador rx.
        if RX_WORDS == 1 {
            crate::ring0::cabina::info("ring3", "primer CONSOLE_WRITE: userspace habla", packed);
        }
    }
    let mut i = 0;
    while i < 8 {
        let b = ((packed >> (i * 8)) & 0xFF) as u8;
        if b == 0 {
            break;
        }
        push(b);
        i += 1;
    }
}

fn push(b: u8) {
    // Everything the caller emits also goes to serial verbatim, so a headless
    // boot still shows the Ring 3 output.
    serial_write_byte(b);
    if b == b'\n' {
        flush();
        return;
    }
    // Non-printable bytes are dropped from the framebuffer line (the FONT16
    // grid is ASCII-only) but were already echoed to serial above.
    if b >= 0x20 && b < 0x7f {
        let s = slot();
        unsafe {
            if LEN[s] < LINE_MAX {
                LINE[s][LEN[s]] = b;
                LEN[s] += 1;
            } else {
                // Overflow: flush what we have and keep going on a fresh line.
                flush();
                LINE[s][0] = b;
                LEN[s] = 1;
            }
        }
    }
}

fn flush() {
    unsafe { FLUSHED += 1 };
    let slot = slot();
    unsafe { LINES_BY_PID[slot] = LINES_BY_PID[slot].wrapping_add(1); }
    let len = unsafe { LEN[slot] };
    // La linea se marca con el nombre del proceso: en el log compartido
    // hay que poder ver de quien es cada renglon.
    let mut tagged = [0u8; TAG_MAX + 2 + LINE_MAX];
    let tag_len = unsafe { TAG_LEN[slot] };
    let head = if tag_len > 0 {
        tagged[..tag_len].copy_from_slice(unsafe { &TAG[slot][..tag_len] });
        tag_len
    } else {
        tagged[..5].copy_from_slice(b"ring3");
        5
    };
    tagged[head] = b'>';
    tagged[head + 1] = b' ';
    let head = head + 2;
    let body = unsafe { &LINE[slot][..len] };
    tagged[head..head + len].copy_from_slice(body);
    // Guardar la linea CRUDA (sin la etiqueta) antes de pintarla: si este
    // proceso se muere, esto es lo unico que quedara de lo que dijo.
    recordar(slot, body);
    if let Ok(s) = core::str::from_utf8(&tagged[..head + len]) {
        // Paint under the KERNEL CR3. This flush runs inside the syscall
        // dispatch of a Ring 3 caller, i.e. under the USER CR3 -- whose
        // address space shares kernel identity only for 0..1 GiB (its PDPT
        // slots 1..3 hold the user image/stack/channels). The GOP
        // framebuffer sits at ~3.5 GiB identity, unmapped there: the first
        // pixel would #PF, and the fault reporter paints too -> recursive
        // #PF on IST1 -> the silent total freeze. Kernel code and stacks are
        // shared in every address space, so the switch is safe.
        let cur = crate::ring0::mm::vmm::read_cr3();
        let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
        if cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(kpml4);
        }
        crate::ring0::core::phase::dashboard_log(s);
        if cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(cur);
        }
    }
    unsafe { LEN[slot] = 0 };
}
