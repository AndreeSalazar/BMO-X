//! **El LIENZO**: una app pinta donde se ve, sin copias.
//!
//! ═══ Qué resuelve ═══
//!
//! La pantalla tiene un solo dueño, y lo es el compositor. Cualquier otro
//! programa que la pida recibe un no — le pasó al raycaster. La salida no es
//! quitarle la pantalla al escritorio: es que le **preste un trozo del sitio
//! donde pinta**, y que la app escriba ahí directamente.
//!
//! Cero copias. La app escribe una vez, en el sitio donde se va a ver, y nadie
//! mueve nada después. El contrato completo está en `docs/LIENZO.md`.
//!
//! ═══ Las dos mitades, y por qué el kernel no sabe nombres propios ═══
//!
//! 1. **El compositor DECLARA** su lienzo: *"este bloque mío es el sitio del que
//!    se prestan reflejos"*.
//! 2. **Una app PIDE** un reflejo, y recibe páginas de ese bloque mapeadas en su
//!    propio espacio de direcciones.
//!
//! ★ Que el compositor lo declare, en vez de que el kernel dé por hecho que *"el
//! lienzo es el del proceso que arrancó como escritorio"*, es a propósito: el
//! kernel no sabe qué CPU es (lo dice el perfil), no supone los APIC IDs (los
//! dice la MADT), no fija el formato (lo dice la app). Saberse qué proceso es
//! "el escritorio" sería exactamente ese nombre propio que no tiene.
//!
//! Con esto, **el que cuelga el cartel es el escritorio** — y un compositor de
//! prueba de cincuenta líneas funciona sin tocar una coma del kernel.
//!
//! ═══ Uno solo, y a pantalla completa ═══
//!
//! No hay tabla de bandas ni z-order: un `Option<pid>` y ya. Cubre el caso que
//! importa —DOOM y el raycaster van a pantalla completa— y el día que hagan
//! falta dos a la vez, ése es el **modo ventana con copia**, que es el que
//! compone de verdad. Reflejo = uno, sin copias. Ventana = varios, con copia.

use bmo_abi_lienzo::*;
use crate::ring0::mm::{self, vmm};
use crate::ring0::obj::cap;

/// Espejo de las dos funciones de `bmo_abi::…::surface`. **Son divisiones**, y
/// eso es justo lo que permite tenerlas aquí sin compartir un crate: el diseño
/// anterior pedía un máximo común divisor, y un `mcd` duplicado es el fallo que
/// este proyecto ya pagó con BLAKE3. Una división no tiene dos formas de
/// escribirse mal.
mod bmo_abi_lienzo {
    /// Adelanta un desplazamiento al principio de la fila siguiente.
    pub const fn alinear_a_fila(offset: u64, stride_px: u32) -> u64 {
        let fila = stride_px as u64 * 4;
        if fila == 0 {
            return offset;
        }
        offset.div_ceil(fila) * fila
    }
    /// Filas enteras que caben en `bytes`.
    pub const fn filas(bytes: u64, stride_px: u32) -> u32 {
        let fila = stride_px as u64 * 4;
        if fila == 0 {
            return 0;
        }
        (bytes / fila) as u32
    }
    /// Sólo se sirve XRGB de 32 bits. El otro número está reservado.
    pub const FMT_XRGB32: u64 = 0x00;
    /// Filas de arriba que nunca se prestan: ahí vive la barra del escritorio.
    pub const FILAS_ARRIBA: u32 = 64;
}

/// Dónde se mapea el reflejo en el espacio de la app.
///
/// Lejos de `MEMORIA_VA_BASE` (`0xE000_0000`) a propósito: una app puede tener
/// bloques de `malloc` **y** un reflejo, y que se pisaran sería un fallo de los
/// que no dan error — simplemente pintas donde está tu `malloc`.
const LIENZO_VA_BASE: u64 = 0x0000_0001_0000_0000;

/// El lienzo declarado por el compositor.
struct Cartel {
    pid: u32,
    /// Su espacio de direcciones: hace falta para traducir sus páginas.
    aspace: u64,
    /// Dónde empieza el lienzo **en el espacio del compositor**.
    base: u64,
    bytes: u64,
    stride: u32,
}

static mut CARTEL: Option<Cartel> = None;
/// Quién tiene el reflejo prestado. Uno solo. Ver la cabecera.
static mut INQUILINO: Option<u32> = None;
/// Dónde y cuánto se le prestó, para poder desmapearlo al morir.
static mut PRESTADO: (u64, u64) = (0, 0);

/// **El compositor cuelga el cartel.** `base` es la de su bloque de memoria.
///
/// Se guarda su espacio de direcciones tal como está AHORA: es el que se usará
/// para traducir sus páginas a físicas. Si el compositor muriera, el cartel se
/// retira (ver [`soltar`]) y nadie traduce contra un espacio que ya no existe.
pub fn declarar(pid: u32, aspace: u64, base: u64, bytes: u64, stride: u32) -> bool {
    if base == 0 || bytes == 0 || stride == 0 {
        return false;
    }
    unsafe {
        CARTEL = Some(Cartel { pid, aspace, base, bytes, stride });
    }
    crate::ring0::cabina::info("lienzo", "el compositor declaro su lienzo, filas", filas(bytes, stride) as u64);
    true
}

/// **Una app pide el reflejo.** Devuelve el handle, o `None`.
///
/// `paginas` es lo que pide; se le prestan **las últimas** del lienzo, que es
/// donde no está la barra. Se piden en páginas y no en filas porque la página
/// es lo único que el kernel sabe repartir y proteger — la app saca sus filas
/// dividiendo, y ninguno de los dos necesita la aritmética del otro.
pub fn reflejo(pid: u32, aspace: u64, paginas: u64, formato: u64) -> Option<u64> {
    if formato != FMT_XRGB32 {
        crate::ring0::cabina::warn("lienzo", "formato que todavia no se sirve", formato);
        return None;
    }
    if paginas == 0 {
        return None;
    }
    let c = unsafe { (*core::ptr::addr_of!(CARTEL)).as_ref()? };
    if c.pid == pid {
        // El compositor pidiéndose a sí mismo su propio lienzo. No es un error
        // con nombre: es que no tiene sentido, y decirlo evita un mapeo que se
        // solaparía consigo mismo.
        crate::ring0::cabina::warn("lienzo", "el dueño del lienzo no se lo puede pedir", pid as u64);
        return None;
    }
    if unsafe { *core::ptr::addr_of!(INQUILINO) }.is_some() {
        crate::ring0::cabina::warn("lienzo", "ya hay un reflejo prestado: solo hay uno", 0);
        return None;
    }

    // Lo que se presta: las últimas `paginas`, sin comerse el techo de la barra.
    let pedido = paginas * mm::PAGE;
    let techo = FILAS_ARRIBA as u64 * c.stride as u64 * 4;
    if pedido + techo > c.bytes {
        crate::ring0::cabina::warn("lienzo", "no cabe: se pide mas lienzo del que sobra", paginas);
        return None;
    }
    let desde = c.bytes - pedido;

    // ── El préstamo, página a página ──────────────────────────────────
    //
    // Se traduce en el espacio del COMPOSITOR y se mapea en el de la APP: los
    // marcos son los mismos, las direcciones no. Eso es todo el reflejo.
    let mut off = 0u64;
    while off < pedido {
        let Some(fisica) = vmm::translate(c.aspace, c.base + desde + off) else {
            deshacer(aspace, off);
            crate::ring0::cabina::warn("lienzo", "el lienzo del compositor no esta mapeado", off);
            return None;
        };
        if vmm::map_page(aspace, LIENZO_VA_BASE + off, fisica, true, true).is_err() {
            // Igual que en `memoria::pedir`: un mapeo a medias deja páginas
            // sueltas en el espacio del usuario, y eso es peor que no tener
            // nada. Se deshace lo hecho.
            deshacer(aspace, off);
            return None;
        }
        off += mm::PAGE;
    }

    // La base que se le da a la app está adelantada al principio de fila: las
    // últimas N páginas empiezan donde empiezan, y eso puede caer a media fila.
    // Adelantar pierde menos de una fila y le ahorra a la app saberlo.
    let sesgo = alinear_a_fila(desde, c.stride) - desde;
    let base_app = LIENZO_VA_BASE + sesgo;
    let bytes_app = pedido - sesgo;

    let handle = cap::grant(
        pid,
        cap::KIND_LIENZO,
        cap::RIGHT_READ | cap::RIGHT_WRITE,
        base_app,
    )?;
    unsafe {
        INQUILINO = Some(pid);
        PRESTADO = (base_app, bytes_app);
    }
    crate::ring0::cabina::info("lienzo", "reflejo prestado, filas", filas(bytes_app, c.stride) as u64);
    Some(handle)
}

/// Deshace un mapeo a medias. **Desmapea sin liberar**: los marcos son del
/// compositor.
fn deshacer(aspace: u64, hasta: u64) {
    let mut off = 0u64;
    while off < hasta {
        vmm::unmap_page(aspace, LIENZO_VA_BASE + off);
        off += mm::PAGE;
    }
}

/// Lo que contesta el handle. Espejo de `LIENZO_OP_*`.
pub fn operacion(base: u64, op: u64) -> Option<u64> {
    let c = unsafe { (*core::ptr::addr_of!(CARTEL)).as_ref()? };
    let (b, bytes) = unsafe { *core::ptr::addr_of!(PRESTADO) };
    if b != base {
        return None;
    }
    match op {
        1 => Some(base),
        2 => Some(bytes),
        3 => Some(c.stride as u64),
        _ => None,
    }
}

/// **Lo llama `cap::revoke_all`.** Retira el cartel si muere el compositor, y
/// devuelve el reflejo si muere el inquilino.
///
/// ⚠️ Aquí está el truco que más caro se paga: `vmm::unmap_page` **devuelve el
/// marco y NO lo libera**, y eso es exactamente lo que hace falta. Los marcos
/// son del lienzo del compositor; devolverlos al pool sería entregarle el
/// escritorio a otro proceso, y el fallo aparecería tres arranques después.
pub fn soltar(pid: u32, aspace: u64) {
    unsafe {
        if *core::ptr::addr_of!(INQUILINO) == Some(pid) {
            let (base, bytes) = *core::ptr::addr_of!(PRESTADO);
            // Se desmapea desde la base SIN sesgo: lo mapeado empezó en
            // `LIENZO_VA_BASE`, y la base que ve la app está más adelante.
            let _ = base;
            let mut off = 0u64;
            let total = alinear_a_fila(bytes, 1) + mm::PAGE; // cota holgada
            while off < total {
                if vmm::unmap_page(aspace, LIENZO_VA_BASE + off).is_none() {
                    break; // ya no hay más mapeado: se acabó
                }
                off += mm::PAGE;
            }
            INQUILINO = None;
            PRESTADO = (0, 0);
            crate::ring0::cabina::info("lienzo", "reflejo devuelto por el pid", pid as u64);
        }
        if let Some(c) = (*core::ptr::addr_of!(CARTEL)).as_ref() {
            if c.pid == pid {
                CARTEL = None;
                crate::ring0::cabina::warn("lienzo", "murio el dueño del lienzo: cartel retirado", pid as u64);
            }
        }
    }
}

/// ¿Hay reflejo prestado? Lo pregunta el compositor para no limpiar esa zona.
/// Devuelve las **filas de abajo** que no son suyas, o `0`.
pub fn filas_prestadas() -> u64 {
    unsafe {
        let Some(c) = (*core::ptr::addr_of!(CARTEL)).as_ref() else { return 0 };
        if (*core::ptr::addr_of!(INQUILINO)).is_none() {
            return 0;
        }
        let (_, bytes) = *core::ptr::addr_of!(PRESTADO);
        filas(bytes, c.stride) as u64
    }
}
