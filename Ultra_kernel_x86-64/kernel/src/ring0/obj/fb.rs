//! `KIND_FRAMEBUFFER` — la pantalla como capability.
//!
//! ## Qué es esto, y por qué no es "un syscall para dibujar"
//!
//! La tentación evidente sería `INVOKE(fb, DRAW_RECT, x, y, w, h)`. Sería más
//! fácil de escribir y sería un error de diseño: cada píxel cruzaría el
//! anillo, el kernel acabaría con un motor de dibujo dentro, y BMO-X sería un
//! monolito con la etiqueta de microkernel puesta encima.
//!
//! Lo que hace este módulo es lo contrario. Un proceso Ring 3 **reclama** la
//! pantalla una vez; el kernel le mapea el framebuffer en su espacio de
//! direcciones con U/S y R/W, le dice dónde quedó y con qué geometría, y a
//! partir de ahí **no vuelve a intervenir**. El compositor escribe píxeles con
//! `mov`, no con `syscall`. Ese es el momento library-OS: no se optimiza el
//! cruce de frontera, se borra la frontera.
//!
//! ## Exclusiva, y el kernel se calla
//!
//! Un solo proceso la tiene a la vez. Al concederla, el kernel **cede la
//! pantalla**: `info::has_fb()` pasa a ser falso y con eso se apagan de golpe
//! todos los caminos de dibujo de Ring 0 —panel, CABINA, logs de drivers—
//! porque todos preguntan por ahí. Dos dueños pintando el mismo framebuffer no
//! es compartir, es parpadeo.
//!
//! Se recupera sola: `cap::revoke_all` la suelta cuando el proceso muere, por
//! la razón que sea, y el kernel vuelve a tener pantalla. Un compositor que
//! se cae no deja la máquina ciega.
//!
//! ## Lo que este módulo TODAVÍA NO decide
//!
//! ★ Hoy la reclama el primero que la pide. Eso no es cero-confianza, es
//! orden de llegada — y está escrito aquí para que se vea, no escondido. La
//! autoridad correcta es una bandera en el contenedor BEF verificada por el
//! gate al admitir el programa: "este binario declara que quiere la pantalla".
//! Cuando esa bandera exista, la comprobación entra en `reclamar` y esta nota
//! se borra. Mientras tanto, el único proceso que la pide es el que tú
//! arrancas.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::ring0::obj::cap;
use crate::ring0::mm::{self, vmm};

/// Nadie la tiene. Los pid válidos son 0..MAX_PROCS, así que hace falta un
/// centinela que no pueda ser un pid.
const SIN_DUENO: u32 = u32::MAX;

static DUENO: AtomicU32 = AtomicU32::new(SIN_DUENO);

/// Ya la tiene otro proceso.
pub const ERROR_OCUPADO: u32 = 16;
/// Esta máquina arrancó sin GOP: no hay pantalla que ceder.
pub const ERROR_SIN_PANTALLA: u32 = 17;

// Operaciones sobre un handle KIND_FRAMEBUFFER.
//
// Cada una devuelve UN `u64` porque eso es lo que cabe en `BmoStatus.value`.
// Los campos que van juntos viajan empaquetados en vez de gastar una llamada
// por número: son datos que se leen una vez al arrancar el compositor.
/// Dirección virtual (en el espacio del proceso) donde quedó mapeada.
pub const FB_OP_BASE: u64 = 0x01;
/// `(ancho << 32) | alto`, en píxeles.
pub const FB_OP_DIMS: u64 = 0x02;
/// `(stride << 32) | formato`. El stride va en PÍXELES, no en bytes — es el
/// mismo número que usa el kernel, y convertirlo aquí sería inventar una
/// unidad distinta a los dos lados de la frontera.
pub const FB_OP_STRIDE: u64 = 0x03;
/// Bytes mapeados en total. Es lo que hace falta para un `rep stosd` que
/// llene la pantalla entera sin multiplicar nada.
pub const FB_OP_BYTES: u64 = 0x04;

/// Bytes que ocupa el framebuffer, redondeado a página.
fn bytes_mapeados() -> u64 {
    let alto = unsafe { crate::info::FB_HEIGHT } as u64;
    let stride = unsafe { crate::info::FB_STRIDE } as u64;
    let crudo = alto * stride * 4;
    (crudo + mm::PAGE - 1) & !(mm::PAGE - 1)
}

/// Concede la pantalla al proceso `pid` y la mapea en `aspace`.
///
/// Devuelve el handle, o el error. El mapeo es U/S + escritura sobre el mismo
/// rango físico que usa el kernel: no hay copia ni doble búfer aquí — el
/// proceso escribe donde el escáner lee. El doble búfer, si lo quiere, lo pone
/// él en su propia memoria, que es exactamente donde debe vivir esa decisión.
pub fn reclamar(pid: u32, aspace: u64) -> Result<u64, u32> {
    if !crate::info::hay_fb_crudo() {
        return Err(ERROR_SIN_PANTALLA);
    }
    // Un solo dueño. `compare_exchange` y no "leer y luego escribir": dos
    // procesos pidiéndola en el mismo tick no pueden ganar los dos.
    if DUENO
        .compare_exchange(SIN_DUENO, pid, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(ERROR_OCUPADO);
    }

    let fisica = unsafe { crate::info::FB_ADDR };
    let bytes = bytes_mapeados();
    let mut off = 0u64;
    while off < bytes {
        if vmm::map_page(aspace, vmm::FRAMEBUFFER_VA_BASE + off, fisica + off, true, true).is_err()
        {
            // Mapeo a medias = páginas de pantalla sueltas en un espacio de
            // usuario. Se deshace lo hecho antes de devolver el error: quedarse
            // con la mitad mapeada y sin handle es peor que no tener nada.
            let mut deshacer = 0u64;
            while deshacer < off {
                vmm::unmap_page(aspace, vmm::FRAMEBUFFER_VA_BASE + deshacer);
                deshacer += mm::PAGE;
            }
            DUENO.store(SIN_DUENO, Ordering::SeqCst);
            return Err(ERROR_SIN_PANTALLA);
        }
        off += mm::PAGE;
    }

    let handle = match cap::grant(
        pid,
        cap::KIND_FRAMEBUFFER,
        cap::RIGHT_READ | cap::RIGHT_WRITE,
        vmm::FRAMEBUFFER_VA_BASE,
    ) {
        Some(h) => h,
        None => {
            DUENO.store(SIN_DUENO, Ordering::SeqCst);
            return Err(cap::ERROR_PERMISSION_DENIED);
        }
    };

    // A partir de aquí el kernel no dibuja. El orden importa: ceder DESPUÉS de
    // que el mapeo y el handle estén hechos, para que un fallo a medias no
    // deje la máquina ciega y sin dueño.
    crate::info::ceder_fb(true);
    crate::ring0::cabina::info("fb", "pantalla cedida a Ring 3", pid as u64);
    Ok(handle)
}

/// El proceso `pid` murió (o salió). Si era el dueño, el kernel recupera la
/// pantalla. Lo llama `cap::revoke_all`, que corre en TODAS las salidas —
/// EXIT voluntario y muerte por fault.
///
/// No se desmapea nada: el espacio de direcciones entero se destruye con el
/// proceso, y desmapear páginas de un CR3 que está a punto de morir es
/// trabajo para nadie.
pub fn proceso_muerto(pid: u32) {
    if DUENO
        .compare_exchange(pid, SIN_DUENO, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        crate::info::ceder_fb(false);
        // WARN y no INFO: el que suelta la pantalla es el que la estaba
        // pintando, o sea el escritorio. Que muera NO es rutina — es la
        // diferencia entre acabar en la ventana o acabar en el shell de Ring 0
        // sin saber por qué. En verde entre veinte líneas verdes no se ve; en
        // ámbar, en una foto de CABINA, sí.
        crate::ring0::cabina::warn(
            "fb",
            "el dueño de la pantalla MURIO: se vuelve al panel del kernel",
            pid as u64,
        );
        // ★ Y SUS ÚLTIMAS PALABRAS, aquí y ahora.
        //
        // El manejador de pánico del compositor dice el archivo y la línea
        // exactos... por la consola del kernel, que **mientras él tenía la
        // pantalla no se pintaba** (`has_fb()` estaba en falso porque la
        // pantalla era suya). O sea que el único mensaje capaz de explicar la
        // muerte se escribía justo en el intervalo en el que nadie podía
        // leerlo.
        //
        // Este es el instante exacto en que la pantalla vuelve a ser del
        // kernel, así que es el primer momento en que se puede pintar — y el
        // último en que alguien se acuerda de preguntar. Guardarlo para el
        // arranque del shell no bastaba: quien relanza a mano el escritorio no
        // pasa por ahí.
        //
        // ⚠️ **Bajo la CR3 del KERNEL.** Esto corre dentro de `revoke_all`, o
        // sea dentro del syscall del proceso que se está muriendo: la CR3 en
        // vigor es la SUYA, y su espacio comparte identidad sólo en 0..1 GiB.
        // El framebuffer vive a ~3,5 GiB: pintar aquí sin cambiar de CR3 sería
        // un #PF, y el reporte de faults también pinta → #PF recursivo en IST1
        // → congelación total y silenciosa. Es la mina que ya costó dos
        // sesiones (ver el patrón del rango identidad alto), y la misma danza
        // que hace `uconsole::flush` por la misma razón.
        let cur = crate::ring0::mm::vmm::read_cr3();
        let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
        if cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(kpml4);
        }
        crate::ring0::core::phase::dashboard_log("  -- lo ULTIMO que dijo el dueno de la pantalla --");
        if crate::ring0::uconsole::hubo_palabras(pid) {
            crate::ring0::uconsole::ultimas_palabras(pid, |linea| {
                let mut buf = [0u8; 128];
                let cabeza = b"  | ";
                let n = linea.len().min(buf.len() - cabeza.len());
                buf[..cabeza.len()].copy_from_slice(cabeza);
                buf[cabeza.len()..cabeza.len() + n].copy_from_slice(&linea.as_bytes()[..n]);
                if let Ok(s) = core::str::from_utf8(&buf[..cabeza.len() + n]) {
                    crate::ring0::core::phase::dashboard_log(s);
                }
            });
        } else {
            crate::ring0::core::phase::dashboard_log("  | (nada: murio sin decir una sola linea)");
        }
        if cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(cur);
        }
    }
}

/// Pid del dueño actual, o `None`.
pub fn dueno() -> Option<u32> {
    match DUENO.load(Ordering::SeqCst) {
        SIN_DUENO => None,
        pid => Some(pid),
    }
}

/// Despacho de las operaciones síncronas sobre la capability ya resuelta.
/// `base` es el objeto que guarda la capability: la VA donde se mapeó.
pub fn operacion(base: u64, operacion: u64) -> Option<u64> {
    let (ancho, alto, stride, formato) = unsafe {
        (
            crate::info::FB_WIDTH as u64,
            crate::info::FB_HEIGHT as u64,
            crate::info::FB_STRIDE as u64,
            crate::info::FB_PIXEL_FORMAT as u64,
        )
    };
    match operacion {
        FB_OP_BASE => Some(base),
        FB_OP_DIMS => Some((ancho << 32) | alto),
        FB_OP_STRIDE => Some((stride << 32) | formato),
        FB_OP_BYTES => Some(bytes_mapeados()),
        _ => None,
    }
}
