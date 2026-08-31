//! **THE BLACK BOX** -- the ring, on the disk.
//!
//! [carril]  AMARILLO  que se guarda del anillo, y eso cambia con lo que hace falta saber
//!
//! === Why this is a file of its own ===
//!
//! Because it is the only part of CABINA that survives the machine being
//! switched off, and that changes every constraint. Painting an event costs
//! nothing and can be dropped; writing one has to go through the write gate,
//! respects the named window, and can fail for reasons that have nothing to do
//! with logging.
//!
//! [!] It is also the one that can be asked to run **while the thing it is
//! recording is dying**, which is exactly when the disk is least trustworthy.

use super::*;

// -- La caja negra: el anillo, en el disco -----------------------------------
//
// Hasta aqui CABINA lo ve todo y lo olvida al apagar. Un registrador de vuelo
// que solo existe mientras vuela el avion no sirve para investigar la caida.
//
// El buffer es estatico y no una variable local: 48 eventos formateados son
// varios KiB y la pila del kernel no esta para eso.

pub(crate) const DUMP_MAX: usize = 8 * 1024;
pub(crate) static mut DUMP: [u8; DUMP_MAX] = [0u8; DUMP_MAX];

/// Vuelca la bitacora entera a `CABINA.LOG` en el volumen de datos.
///
/// Devuelve los bytes escritos, o 0 con el motivo ya narrado. Se escribe el
/// anillo en orden CRONOLOGICO (el mas viejo primero), al reves de como se
/// pinta en pantalla: un archivo se lee de arriba abajo.
pub fn dump_to_disk() -> usize {
    let mut n = 0usize;
    {
        // Cabecera: sin ella, un archivo de lineas sueltas no dice de que
        // arranque es ni cuanto se perdio.
        let mut h = Buf::new();
        h.txt("BMO-X CABINA -- bitacora de vuelo\n");
        n = append(n, h.as_str());
        let mut h = Buf::new();
        h.txt("eventos="); h.dec(event_total());
        h.txt(" perdidos="); h.dec(event_lost());
        h.txt(" anillo="); h.dec(EVENT_RING as u64);
        h.txt("\n");
        n = append(n, h.as_str());
        let mut h = Buf::new();
        h.txt("disco="); h.txt(crate::ring0::dev::disk::model());
        h.txt(" serie="); h.txt(crate::ring0::dev::disk::serial());
        h.txt("\n\n");
        n = append(n, h.as_str());
    }

    // Del mas viejo al mas nuevo.
    let have = (event_total() as usize).min(EVENT_RING);
    for i in (0..have).rev() {
        let ev = match event_back(i) { Some(e) => e, None => continue };
        let mut r = Buf::new();
        r.dec_pad(ev.seq, 4);
        r.txt(" t"); r.hex(ev.tick_ns, 5);
        r.txt(" "); r.pad(ev.severity.name(), 5);
        r.txt(" "); r.txt(ev.module_str()); r.txt(": ");
        r.txt(ev.msg_str());
        if ev.value != 0 { r.txt(" ="); r.value_of(&ev); }
        // ** AQUI EL SITIO VA EN TODOS, y no solo en los FAULT.
        //
        // En pantalla se reserva para lo que duele porque hay 80 columnas que
        // repartir. Un fichero no tiene esa limitacion, y **el que lee
        // `CABINA.LOG` no tiene la maquina delante**: esta reconstruyendo lo que
        // paso a partir de esto y nada mas. Ahi el `INFO` de la linea de antes
        // es justo el que dice por donde iba el sistema cuando se torcio.
        let f = ev.fichero_str();
        if !f.is_empty() {
            r.txt("  <"); r.txt(f); r.txt(":"); r.dec(ev.linea as u64); r.txt(">");
        }
        r.txt("\n");
        n = append(n, r.as_str());
    }

    // Sin autoref sobre el puntero crudo: se pide el slice explicitamente, que
    // es lo mismo que el compilador iba a hacer pero dicho en voz alta.
    let data = unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(DUMP) as *const u8, n) };
    match crate::ring0::fsys::fs::create(b"CABINA  LOG", data) {
        Ok(()) => {
            info("fs", "bitacora volcada a CABINA.LOG", n as u64);
            n
        }
        Err(e) => {
            // El motivo REAL, no un "no se pudo". "Ya existe" y "disco lleno"
            // piden cosas distintas de quien lo lee.
            fault("fs", e.name(), n as u64);
            0
        }
    }
}

/// Anade texto al buffer de volcado sin desbordarlo. Devuelve el nuevo final.
pub(crate) fn append(mut n: usize, s: &str) -> usize {
    unsafe {
        let buf = &mut *core::ptr::addr_of_mut!(DUMP);
        for &b in s.as_bytes() {
            if n >= buf.len() { return n; }
            buf[n] = b;
            n += 1;
        }
    }
    n
}

/// Total de eventos grabados desde el arranque (puede exceder el anillo).
pub fn event_total() -> u64 { unsafe { EV_TOTAL } }
/// Eventos perdidos por reentrancia. Deberia ser 0; si no lo es, algo falto
/// durante un fault y la bitacora lo dice en vez de callarlo.
pub fn event_lost() -> u64 { unsafe { EV_LOST } }

// Paleta de estado (aviso por color, como pidio el usuario): verde = bien,
// ambar = atencion, rojo = problema, cyan = info/titulo, gris = neutro.
// Los valores son los mismos que la paleta del panel (core/splash.rs): CABINA
// y el log rodante comparten pantalla, y dos verdes distintos a diez pixeles
// uno del otro se leen como un error de impresion, no como dos capas.
pub(crate) const C_OK: u32    = 0xFF39FF88; // verde neon
pub(crate) const C_WARN: u32  = 0xFFF6C445; // ambar
pub(crate) const C_FAULT: u32 = 0xFFFF3355; // rojo lacado -- SOLO para lo que va mal
pub(crate) const C_INFO: u32  = 0xFF00F0FF; // cian (titulo)
pub(crate) const C_DIM: u32   = 0xFF55647E; // gris azulado
pub(crate) const C_TEXT: u32  = 0xFFE6EDF7; // texto normal
pub(crate) const C_RING3: u32 = 0xFF39FF88; // verde -- userspace
pub(crate) const C_FS: u32    = 0xFF2DE2C5; // jade -- almacenamiento
pub(crate) const C_SEC: u32   = 0xFFC084FC; // violeta -- capabilities

/// Color de una linea de bitacora. La severidad manda (un fallo es rojo venga
/// de donde venga); si es informativa, el color lo pone la CAPA -- asi se lee
/// de un vistazo quien habla sin descifrar el prefijo.
pub(crate) fn ev_color(ev: &Event) -> u32 {
    match ev.severity {
        Severity::Panic | Severity::Fault => C_FAULT,
        Severity::Warning => C_WARN,
        Severity::Trace => C_DIM,
        Severity::Info => match ev.layer {
            Layer::Ring3 => C_RING3,
            Layer::Fs => C_FS,
            Layer::Sec => C_SEC,
            Layer::Lang | Layer::BmoGpu => C_INFO,
            _ => C_TEXT,
        },
    }
}

/// Construye un snapshot desde el estado VIVO del kernel. Aqui `cabina-core`
/// deja de ser estructuras muertas y empieza a respirar.
pub fn snapshot() -> TelemetrySnapshot {
    let mut s = TelemetrySnapshot::zero();
    s.cpu.timer_ticks = crate::ring0::plat::timer::ticks();
    let (_total, free) = crate::ring0::mm::phys::stats();
    s.memory.free_pages = free;
    s.scheduler.context_switches = crate::ring0::task::scheduler::user_switches();
    let (tasks, runnable) = crate::ring0::task::scheduler::counts();
    s.scheduler.processes = tasks as u64;
    s.scheduler.threads = runnable as u64;
    s.uptime_ns = s.cpu.timer_ticks; // proxy hasta tener ns reales del TSC
    s
}
