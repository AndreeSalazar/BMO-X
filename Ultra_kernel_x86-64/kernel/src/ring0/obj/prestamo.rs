//! **PRESTAR memoria**: un proceso cede un trozo del suyo a otro.
//!
//! ═══ Lo que este módulo NO sabe ═══
//!
//! No sabe qué es un lienzo, ni una ventana, ni un escritorio. **No sabe para
//! qué se presta.** Mueve páginas y comprueba que quien las presta es su dueño.
//!
//! Y eso es el cambio entero respecto a la versión anterior, que sí lo sabía:
//! tenía `KIND_LIENZO`, una operación para *"declarar mi lienzo"* y otra para
//! *"pedir un reflejo"*. Funcionaba, y metía un concepto de escritorio dentro
//! de Ring 0.
//!
//! ★ La pregunta del dueño lo destapó: *"¿Ring 3 no puede administrar eso él?"*.
//! Sí puede, y debe. Lo único que Ring 3 **no** puede hacer es tocar las tablas
//! de páginas — y eso es lo único que se queda aquí.
//!
//! | | Quién decide |
//! |---|---|
//! | cuánto se presta, a quién, cuándo | **el compositor**. Es política |
//! | mover las páginas | **el kernel**. Es mecanismo, y sólo él puede |
//!
//! Ésa es la separación que hace que un microkernel valga lo que cuesta, y es
//! el patrón de **seL4** — el linaje que la hoja de ruta declara como el más
//! cercano: el kernel no sabe qué es una ventana, un fichero ni un socket.
//!
//! ═══ Y lo que se gana, que es más que el lienzo ═══
//!
//! Con una operación genérica salen gratis el audio (un programa presta su búfer
//! al mezclador), la captura de vídeo, y el paso de bloques grandes entre
//! procesos, que hoy tendrían que ir por un canal de mensajitos. **Una
//! operación, cuatro problemas** — en vez de una operación por cada cosa nueva.
//!
//! ═══ Se OFRECE y se TOMA, y no al revés ═══
//!
//! El que presta **ofrece**: apunta que un trozo suyo es para tal proceso. El
//! que recibe **toma**: el mapeo ocurre dentro de SU llamada, en SU espacio de
//! direcciones.
//!
//! ★ No es un detalle de estilo. Mapear en el espacio de otro exigiría que el
//! kernel supiera el `CR3` de un proceso que no está corriendo, y eso es
//! infraestructura que hoy no existe. Tomando, el espacio de destino es
//! `read_cr3()` — el del que llama. El problema no se resuelve: se coloca donde
//! no existe.

use crate::ring0::mm::{self, vmm};
use crate::ring0::obj::cap;

/// Ofertas vivas a la vez. Ocho: hoy hay una (el escritorio a una app) y el
/// tope existe para que una oferta olvidada no crezca sin fin.
const MAX: usize = 8;

/// Dónde se mapea lo prestado en el espacio del que toma.
///
/// Lejos de `MEMORIA_VA_BASE` (`0xE000_0000`) a propósito: un proceso puede
/// tener bloques de `malloc` **y** algo prestado, y que se pisaran sería un
/// fallo sin mensaje — escribirías encima de tu propio `malloc`.
const PRESTAMO_VA_BASE: u64 = 0x0000_0001_0000_0000;

#[derive(Clone, Copy)]
struct Oferta {
    viva: bool,
    /// Quién presta, y su espacio: hace falta para traducir sus páginas.
    /// Se captura al ofrecer, que es cuando ese espacio está cargado.
    dueno: u32,
    aspace_dueno: u64,
    /// Dónde empieza lo ofrecido, **en el espacio del dueño**.
    origen: u64,
    bytes: u64,
    /// A quién va. Sólo él puede tomarla.
    destino: u32,
    /// Ya tomada: dónde quedó en el espacio del destino, para desmapear.
    tomada: bool,
    va_destino: u64,
}

const NADA: Oferta = Oferta {
    viva: false, dueno: 0, aspace_dueno: 0, origen: 0, bytes: 0,
    destino: 0, tomada: false, va_destino: 0,
};
static mut OFERTAS: [Oferta; MAX] = [NADA; MAX];

/// **Ofrecer un trozo del bloque propio.** Devuelve `true` si quedó apuntado.
///
/// `base` es la del bloque del que ofrece —ya resuelta por su capability, o sea
/// que **es suyo por construcción**— y `desde`/`bytes` el trozo. La única
/// comprobación que hace falta es que el trozo quepa dentro, y es una resta:
/// el rango lo concedió el kernel y lo tiene apuntado.
pub fn ofrecer(dueno: u32, aspace: u64, base: u64, entregado: u64, desde: u64, bytes: u64, destino: u32) -> bool {
    if bytes == 0 || desde.checked_add(bytes).map_or(true, |f| f > entregado) {
        crate::ring0::cabina::warn("prestamo", "el trozo no cabe en el bloque", desde);
        return false;
    }
    if destino == dueno {
        return false;
    }
    let ofertas = unsafe { &mut *core::ptr::addr_of_mut!(OFERTAS) };
    // Una oferta por pareja (dueño, destino): reofrecer sustituye, no apila.
    // Un programa que reintenta no debe llenar la tabla.
    for o in ofertas.iter_mut() {
        if o.viva && o.dueno == dueno && o.destino == destino && !o.tomada {
            o.origen = base + desde;
            o.bytes = bytes;
            o.aspace_dueno = aspace;
            return true;
        }
    }
    for o in ofertas.iter_mut() {
        if !o.viva {
            *o = Oferta {
                viva: true, dueno, aspace_dueno: aspace, origen: base + desde,
                bytes, destino, tomada: false, va_destino: 0,
            };
            crate::ring0::cabina::info("prestamo", "ofrecido al pid", destino as u64);
            return true;
        }
    }
    crate::ring0::cabina::warn("prestamo", "no quedan ofertas libres", MAX as u64);
    false
}

/// **Tomar lo que me ofrecieron.** Devuelve el handle, o `None`.
///
/// El mapeo ocurre aquí, en el espacio del que llama. Se traduce página a
/// página en el espacio del dueño y se mapea en el del que toma: **los marcos
/// son los mismos, las direcciones no.** Eso es todo el préstamo.
pub fn tomar(pid: u32, aspace: u64) -> Option<u64> {
    let ofertas = unsafe { &mut *core::ptr::addr_of_mut!(OFERTAS) };
    let i = ofertas.iter().position(|o| o.viva && o.destino == pid && !o.tomada)?;
    let (origen, bytes, aspace_dueno) =
        (ofertas[i].origen, ofertas[i].bytes, ofertas[i].aspace_dueno);

    let paginas = bytes.div_ceil(mm::PAGE) * mm::PAGE;
    let mut off = 0u64;
    while off < paginas {
        let Some(fisica) = vmm::translate(aspace_dueno, origen + off) else {
            deshacer(aspace, off);
            crate::ring0::cabina::warn("prestamo", "lo ofrecido no esta mapeado en el dueño", off);
            return None;
        };
        if vmm::map_page(aspace, PRESTAMO_VA_BASE + off, fisica, true, true).is_err() {
            // Igual que en `memoria::pedir`: un mapeo a medias deja páginas
            // sueltas en el espacio del usuario, y eso es peor que nada.
            deshacer(aspace, off);
            return None;
        }
        off += mm::PAGE;
    }

    let handle = cap::grant(
        pid,
        cap::KIND_PRESTADO,
        cap::RIGHT_READ | cap::RIGHT_WRITE,
        PRESTAMO_VA_BASE,
    );
    match handle {
        Some(h) => {
            ofertas[i].tomada = true;
            ofertas[i].va_destino = PRESTAMO_VA_BASE;
            crate::ring0::cabina::info("prestamo", "tomado, bytes", bytes);
            Some(h)
        }
        None => {
            deshacer(aspace, paginas);
            None
        }
    }
}

fn deshacer(aspace: u64, hasta: u64) {
    let mut off = 0u64;
    while off < hasta {
        vmm::unmap_page(aspace, PRESTAMO_VA_BASE + off);
        off += mm::PAGE;
    }
}

/// Lo que contesta el handle: `1` = dónde, `2` = cuánto.
pub fn operacion(base: u64, op: u64, pid: u32) -> Option<u64> {
    let ofertas = unsafe { &*core::ptr::addr_of!(OFERTAS) };
    let o = ofertas.iter().find(|o| o.viva && o.tomada && o.destino == pid && o.va_destino == base)?;
    match op {
        1 => Some(o.va_destino),
        2 => Some(o.bytes),
        _ => None,
    }
}

/// **Lo llama `cap::revoke_all`.**
///
/// ⚠️ Aquí está el truco que más caro se paga: `vmm::unmap_page` **devuelve el
/// marco y NO lo libera**, y eso es exactamente lo que hace falta. Los marcos
/// son **del que prestó**; devolverlos al pool sería entregarle su memoria a un
/// tercero, y el fallo aparecería tres arranques después y en otro sitio.
///
/// Se limpian las dos puntas: lo que este proceso tomó (se desmapea) y lo que
/// ofreció (se retira, porque su espacio ya no existe para traducir).
pub fn proceso_muerto(pid: u32, aspace: u64) {
    let ofertas = unsafe { &mut *core::ptr::addr_of_mut!(OFERTAS) };
    for o in ofertas.iter_mut() {
        if !o.viva {
            continue;
        }
        if o.destino == pid && o.tomada {
            let paginas = o.bytes.div_ceil(mm::PAGE) * mm::PAGE;
            deshacer(aspace, paginas);
            crate::ring0::cabina::info("prestamo", "devuelto por el pid", pid as u64);
            *o = NADA;
        } else if o.dueno == pid {
            // Murió el que prestaba. La oferta no vale: su espacio de
            // direcciones se destruye y no habría contra qué traducir.
            crate::ring0::cabina::warn("prestamo", "murio el dueño: oferta retirada", pid as u64);
            *o = NADA;
        }
    }
}
