//! **El ANTENISTA (N3, 2026-09-18): la lamina que trajo la antena vive en un
//! BLOQUE del DIRECTOR y se OFRECE a NAVEGAR cuando se lanza.** Sin el disco
//! en medio.
//!
//! [consumo] NADA      no tiene bucle: escribe al llegar una lamina y ofrece
//!                     al lanzar una app
//!
//! `red pagina <ip> <url>` (`red_tcp.rs`) trae la lamina, la juzga y la deja
//! aqui. Cuando el escritorio lanza `apps/navegar.ibx` --desde la caja de
//! `run` o desde su icono-- se le ofrece el trozo por `MEM_OP_OFRECER`, y
//! NAVEGAR lo toma con `op_tomar` en sus primeros fotogramas. Es el mismo
//! camino por el que una app entrega su superficie al DIRECTOR, al reves:
//! ni una copia, ni un fichero, ni un syscall por byte.
//!
//! ```text
//!    red pagina ...   -> guardar(lamina)       copia al bloque (una vez)
//!    run navegar.ibx  -> tras_lanzar(tid)      MEM_OP_OFRECER(0, largo, tid)
//!    NAVEGAR          -> op_tomar / prestado_base / prestado_bytes / pinta
//! ```
//!
//! ** El disco sigue: `datos/pagina.lam` se escribe igual que en N3a, y es lo
//! que NAVEGAR pinta si arranca sin oferta (desde el shell de Ring 0, o si
//! este bloque no se pudo pedir). No es un cache: es la MISMA lamina, y la
//! app dice de donde la saco.
//!
//! [!] El bloque es UNA de las cuatro peticiones de memoria que el kernel da
//! a cada proceso (`obj/memory.rs`, `MAX_PETICIONES`), y el DIRECTOR ya gasta
//! otras en la consola, el visor y el fondo. Se pide la primera vez que hace
//! falta y si no queda, se dice y queda el disco. Ese tope es deuda del
//! kernel, no de aqui.

use core::ptr::{addr_of, addr_of_mut};

use bmo_userland as bmo;

use crate::scene::output::{Output, INK_ERR, INK_GOOD};

/// Lo mismo que `red_tcp::LAMINA_MAX`: lo que cabe en el bloque.
pub(crate) const BLOQUE_BYTES: u64 = 256 * 1024;

/// A quien se ofrece: la app que sabe tomar una lamina. Se compara el final
/// de la ruta, para que `apps/navegar.ibx` y `navegar.ibx` sean lo mismo.
const CLIENTE: &[u8] = b"navegar.ibx";

static mut BLOQUE: Option<bmo::Memoria> = None;
/// Bytes validos en el bloque; 0 = no hay lamina.
static mut LARGO: usize = 0;
/// Cuantas veces se ofrecio, y cuantas el kernel dijo que no.
static mut OFRECIDAS: u32 = 0;
static mut NEGADAS: u32 = 0;

fn bloque() -> Option<&'static bmo::Memoria> {
    let slot = addr_of_mut!(BLOQUE);
    unsafe {
        if (*slot).is_none() {
            *slot = bmo::Memoria::request(BLOQUE_BYTES);
        }
        (*slot).as_ref()
    }
}

/// **La lamina, al bloque.** Sustituye a la anterior. `false` si no hay
/// bloque (el tope de peticiones) o no cabe: entonces solo queda el disco.
pub(crate) fn guardar(lam: &[u8]) -> bool {
    if lam.is_empty() || lam.len() as u64 > BLOQUE_BYTES {
        return false;
    }
    let Some(b) = bloque() else { return false };
    // SAFETY: el bloque mide `BLOQUE_BYTES` y `lam` cabe; nadie mas escribe
    // en el (NAVEGAR lo recibe de solo lectura por el prestamo).
    unsafe {
        core::ptr::copy_nonoverlapping(lam.as_ptr(), b.base(), lam.len());
        *addr_of_mut!(LARGO) = lam.len();
    }
    true
}

/// Hay lamina en el bloque?
pub(crate) fn hay() -> bool {
    unsafe { *addr_of!(LARGO) != 0 }
}

/// **Al lanzar una app**: si es NAVEGAR y hay lamina, se le ofrece. Devuelve
/// `Some(motivo)` (0 = ofrecida, el resto `bmo::offer_nombre`) si se intento,
/// `None` si no era para ella o no habia nada que dar.
pub(crate) fn tras_lanzar(ruta: &[u8], tid: u32) -> Option<u32> {
    if !ruta.ends_with(CLIENTE) || !hay() {
        return None;
    }
    let b = bloque()?;
    let largo = unsafe { *addr_of!(LARGO) } as u64;
    let motivo = bmo::offer_motivo(b.handle(), 0, largo, tid);
    unsafe {
        *addr_of_mut!(OFRECIDAS) += 1;
        if motivo != bmo::OFRECIDO {
            *addr_of_mut!(NEGADAS) += 1;
        }
    }
    Some(motivo)
}

/// Para `save`/`red`: lo que hay y lo que paso.
pub(crate) fn informar(s: &mut Output) {
    let (largo, ofrecidas, negadas) = unsafe { (*addr_of!(LARGO), *addr_of!(OFRECIDAS), *addr_of!(NEGADAS)) };
    s.text(b"  [antenista] lamina en bloque: ");
    if largo == 0 {
        s.text(b"ninguna");
    } else {
        s.dec(largo as u64);
        s.text(b" bytes");
    }
    s.text(b"; ofrecida ");
    s.dec(ofrecidas as u64);
    s.text(b" veces");
    if negadas != 0 {
        s.with_ink(INK_ERR);
        s.text(b", negada ");
        s.dec(negadas as u64);
    } else if ofrecidas != 0 {
        s.with_ink(INK_GOOD);
        s.text(b", todas aceptadas");
    }
    s.byte(b'\n');
}
