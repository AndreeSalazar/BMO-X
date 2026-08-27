//! `KIND_MMIO` -- **una ventana de registros de un aparato, cedida a Ring 3.**
//!
//! generacion: nieto -- no sabe quien lo llamo ni por que.
//!
//! [cuesta]  MAQUINA -- mapea fisica en un espacio de usuario. Un rango de
//!           mas es una ventana a la RAM del kernel, y eso no da un fault:
//!           da acceso. Por eso el juez que decide vive fuera y con pruebas
//!
//! Pieza **S1** del suelo de `docs/plan/PLAN_SUELO_RING3.md`. El censo que la
//! pidio: `docs/maestro/RING3_MAESTRO.md`.
//!
//! # *** LO PRIMERO, PORQUE ES LO QUE DECIDE LA FORMA DE TODO ESTO
//!
//! > Un proceso que puede decir *"mapeame la fisica 0x1000"* es un proceso que
//! > esta pidiendo ser el kernel.
//!
//! Con esa operacion, en tres pasos --mapear donde viven las tablas de pagina,
//! ponerse el bit U/S, quitar el NX-- **los siete muros de
//! `docs/identidad/EL_AISLAMIENTO.md` se caen a la vez**. No por un bug: por la
//! operacion funcionando como se pidio.
//!
//! Por eso aqui **no hay ninguna funcion que acepte una direccion**. El proceso
//! nombra un APARATO de una lista cerrada; la fisica sale del censo del kernel;
//! y aun asi pasa por [`bmo_mmio_juicio`] antes de mapearse.
//!
//! ```text
//!    MAL   mapear_fisica(0xF7A0_0000, 0x1000)   <- el proceso elige
//!    BIEN  tomar(APARATO_XHCI)                  <- el KERNEL elige la fisica
//! ```
//!
//! # Lo que se concede HOY, dicho para que no parezca mas
//!
//! ```text
//!    UNA pagina           la primera del BAR. Donde viven CAPLENGTH y HCIVERSION
//!    SOLO LECTURA         la pagina se mapea sin escritura, y sin RIGHT_WRITE
//!    exclusiva            un dueno a la vez, como la pantalla y el audio
//! ```
//!
//! ** Escribir en un aparato desde Ring 3 es **otra decision**, y va despues de
//! que leer este probado en metal. Un paso que se puede deshacer con `soltar` y
//! que no puede romper nada es el sitio correcto para estrenar un mecanismo.
//!
//! # [!] Y lo que este modulo NO arregla
//!
//! El DMA. Un aparato al que Ring 3 pudiera dar ordenes escribiria donde
//! quisiera, porque el DMA no pasa por la MMU del CPU. Hoy no se puede dar
//! ordenes --es de solo lectura-- y el dia que se pueda, hara falta una IOMMU.
//! Esta escrito en la parte 4 del plan y se repite aqui porque es lo que se
//! olvida.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::ring0::mm::{self, vmm};
use crate::ring0::obj::cap;

/// Nadie la tiene.
const NO_OWNER: u32 = u32::MAX;

/// **La lista cerrada de aparatos que se pueden pedir.**
///
/// Es una lista y no un rango a proposito: lo que no esta aqui no se puede
/// nombrar, y por tanto no se puede pedir por error ni por astucia.
pub const APARATO_XHCI: u64 = 0;

/// Donde se mapea, en el espacio del que la pide. Muy por encima de la imagen
/// (1 GiB) y de la pila (2 GiB), y por debajo del framebuffer.
pub const MMIO_VA_BASE: u64 = 0x0000_0000_9000_0000;

/// Lo que se cede: una pagina. Ver la cabecera.
const BYTES: u64 = mm::PAGE;

static OWNER: AtomicU32 = AtomicU32::new(NO_OWNER);
static HANDLE: AtomicU64 = AtomicU64::new(0);
/// La fisica que se cedio, para poder contarla en CABINA al soltar.
static FISICA: AtomicU64 = AtomicU64::new(0);

/// No hay aparato con ese numero.
pub const ERROR_NO_APARATO: u32 = 4;
/// El aparato existe pero todavia no esta en pie.
pub const ERROR_NO_LISTO: u32 = 5;
/// Ya lo tiene otro.
pub const ERROR_OCUPADO: u32 = 6;
/// El juez dijo que no. **El motivo va a CABINA con su nombre**, porque en un
/// codigo de error de 32 bits no cabe *"pisa la RAM que empieza en 0x100000"*.
pub const ERROR_NO_CEDIBLE: u32 = 7;

/// La fisica del aparato pedido, si esta en pie.
///
/// [!] Devuelve la **fisica**, no la virtual del kernel. El kernel lee sus
/// registros por el physmap; lo que se cede es la fisica, y confundirlas seria
/// mapear la ventana del kernel dentro de un proceso.
fn fisica_de(cual: u64) -> Option<u64> {
    match cual {
        APARATO_XHCI => {
            let virt = bmo_xhci::get_mmio()?;
            // `get_mmio` guarda la VIRTUAL del physmap, que es con la que el
            // driver trabaja. Se deshace la traduccion, que es exacta y no una
            // busqueda: el physmap es una suma.
            Some(mm::virt_to_phys_physmap(virt))
        }
        _ => None,
    }
}

/// Las reservas de la casa: rangos que se reparten por OTRA puerta.
///
/// La pantalla tiene `KIND_FRAMEBUFFER`. Cederla tambien por aqui seria dos
/// puertas al mismo aparato, y entonces "un dueno a la vez" deja de ser cierto
/// sin que nadie lo haya escrito.
fn reservas() -> [bmo_mmio_juicio::Reserva; 1] {
    let fb = unsafe { crate::info::FB_ADDR };
    let bytes = unsafe {
        (crate::info::FB_STRIDE as u64) * (crate::info::FB_HEIGHT as u64) * 4
    };
    [bmo_mmio_juicio::Reserva { base: fb, bytes, nombre: "la pantalla" }]
}

/// **Concede la ventana del aparato `cual` al proceso `pid`.**
///
/// El orden es el mismo que en `fb::claim` y por el mismo motivo: primero
/// asegurar que hay un solo dueno, luego mapear, y solo al final entregar el
/// handle. Un fallo a medias no puede dejar paginas de un aparato sueltas en un
/// espacio de usuario sin nadie que las suelte.
pub fn claim(pid: u32, aspace: u64, cual: u64) -> Result<u64, u32> {
    let fisica = match fisica_de(cual) {
        Some(f) if f != 0 => f,
        Some(_) => return Err(ERROR_NO_LISTO),
        None => return Err(ERROR_NO_APARATO),
    };

    // *** EL JUEZ VA ANTES QUE EL CERROJO, y no al reves.
    //
    // Si fuera despues, un rango que no se puede ceder dejaria el aparato
    // marcado como ocupado durante el rato que tarda en rechazarse -- y si
    // alguien anadiera un `return` por el medio, para siempre. Juzgar primero
    // deja el fallo sin efectos.
    let mapa = crate::ring0::mm::phys::tramos();
    let res = reservas();
    if let Err(v) = bmo_mmio_juicio::cedible(fisica, BYTES, mapa, &res) {
        // El nombre del veto Y su numero. El codigo de error solo puede decir
        // "no cedible"; cual de los nueve fue vive aqui.
        crate::ring0::cabina::fault("mmio", v.nombre(), v.donde());
        return Err(ERROR_NO_CEDIBLE);
    }

    if OWNER
        .compare_exchange(NO_OWNER, pid, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(ERROR_OCUPADO);
    }

    // ** `writable = false`. Es la mitad de lo que hace esto seguro: aunque el
    // proceso quisiera escribir en el controlador, la propia MMU se lo impide.
    // La otra mitad es no darle `RIGHT_WRITE`, y las dos estan a proposito --
    // una capability sin el derecho y una pagina sin el bit dicen lo mismo en
    // dos idiomas, y el dia que una se afloje la otra sigue.
    if vmm::map_page(aspace, MMIO_VA_BASE, fisica, true, false).is_err() {
        OWNER.store(NO_OWNER, Ordering::SeqCst);
        return Err(ERROR_NO_CEDIBLE);
    }

    let handle = match cap::grant(pid, cap::KIND_MMIO, cap::RIGHT_READ, MMIO_VA_BASE) {
        Some(h) => h,
        None => {
            vmm::unmap_page(aspace, MMIO_VA_BASE);
            OWNER.store(NO_OWNER, Ordering::SeqCst);
            return Err(cap::ERROR_PERMISSION_DENIED);
        }
    };
    HANDLE.store(handle, Ordering::SeqCst);
    FISICA.store(fisica, Ordering::SeqCst);
    crate::ring0::cabina::info("mmio", "ventana de aparato cedida a Ring 3", fisica);
    Ok(handle)
}

/// Devolver la ventana sin morirse.
pub fn release(pid: u32, aspace: u64) -> Result<(), u32> {
    if OWNER.load(Ordering::SeqCst) != pid {
        return Err(cap::ERROR_PERMISSION_DENIED);
    }
    vmm::unmap_page(aspace, MMIO_VA_BASE);
    let h = HANDLE.swap(0, Ordering::SeqCst);
    if h != 0 {
        cap::revoke(pid, h);
    }
    FISICA.store(0, Ordering::SeqCst);
    OWNER.store(NO_OWNER, Ordering::SeqCst);
    crate::ring0::cabina::info("mmio", "ventana de aparato devuelta", pid as u64);
    Ok(())
}

/// El dueno murio. **Se suelta sola**, igual que la pantalla.
///
/// Sin esto, un proceso que reventara dejaria el aparato ocupado para siempre y
/// haria falta reiniciar para volver a pedirlo. Es `R-APP6` --*muere sin
/// llevarse a nadie*-- y aqui el "nadie" es el proximo que lo pida.
///
/// [!] No se desmapea: el espacio de direcciones entero se destruye detras
/// (`destroy_address_space`), y la pagina del aparato **no lleva `PTE_NUESTRA`**,
/// asi que su marco no se devuelve al asignador. Que no lo lleve es justo lo que
/// impide que la RAM del sistema se "recicle" un registro de un controlador.
pub fn process_died(pid: u32) {
    if OWNER
        .compare_exchange(pid, NO_OWNER, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        HANDLE.store(0, Ordering::SeqCst);
        FISICA.store(0, Ordering::SeqCst);
        crate::ring0::cabina::info("mmio", "ventana de aparato liberada al morir", pid as u64);
    }
}

/// Las operaciones sobre el handle. `None` = esta capability no la conoce.
pub fn operation(object: u64, op: u64) -> Option<u64> {
    match op {
        crate::ring0::syscall::ops::APARATO_OP_BASE => Some(object),
        crate::ring0::syscall::ops::APARATO_OP_BYTES => Some(BYTES),
        _ => None,
    }
}
