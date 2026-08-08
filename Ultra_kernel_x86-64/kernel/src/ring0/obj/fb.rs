//! `KIND_FRAMEBUFFER` -- la pantalla como capability.
//!
//! ## Que es esto, y por que no es "un syscall para dibujar"
//!
//! La tentacion evidente seria `INVOKE(fb, DRAW_RECT, x, y, w, h)`. Seria mas
//! facil de escribir y seria un error de diseno: cada pixel cruzaria el
//! anillo, el kernel acabaria con un motor de dibujo dentro, y BMO-X seria un
//! monolito con la etiqueta de microkernel puesta encima.
//!
//! Lo que hace este modulo es lo contrario. Un proceso Ring 3 **reclama** la
//! pantalla una vez; el kernel le mapea el framebuffer en su espacio de
//! direcciones con U/S y R/W, le dice donde quedo y con que geometria, y a
//! partir de ahi **no vuelve a intervenir**. El compositor escribe pixeles con
//! `mov`, no con `syscall`. Ese es el momento library-OS: no se optimiza el
//! cruce de frontera, se borra la frontera.
//!
//! ## Exclusiva, y el kernel se calla
//!
//! Un solo proceso la tiene a la vez. Al concederla, el kernel **cede la
//! pantalla**: `info::has_fb()` pasa a ser falso y con eso se apagan de golpe
//! todos los caminos de dibujo de Ring 0 --panel, CABINA, logs de drivers--
//! porque todos preguntan por ahi. Dos duenos pintando el mismo framebuffer no
//! es compartir, es parpadeo.
//!
//! Se recupera sola: `cap::revoke_all` la suelta cuando el proceso muere, por
//! la razon que sea, y el kernel vuelve a tener pantalla. Un compositor que
//! se cae no deja la maquina ciega.
//!
//! ## Lo que este modulo TODAVIA NO decide
//!
//! * Hoy la reclama el primero que la pide. Eso no es cero-confianza, es
//! orden de llegada -- y esta escrito aqui para que se vea, no escondido. La
//! autoridad correcta es una bandera en el contenedor BEF verificada por el
//! gate al admitir el programa: "este binario declara que quiere la pantalla".
//! Cuando esa bandera exista, la comprobacion entra en `reclamar` y esta nota
//! se borra. Mientras tanto, el unico proceso que la pide es el que tu
//! arrancas.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::ring0::obj::cap;
use crate::ring0::mm::{self, vmm};

/// Nadie la tiene. Los pid validos son 0..MAX_PROCS, asi que hace falta un
/// centinela que no pueda ser un pid.
const SIN_DUENO: u32 = u32::MAX;

static DUENO: AtomicU32 = AtomicU32::new(SIN_DUENO);
/// El handle que se le concedio al dueno, para poder revocarlo si lo SUELTA.
///
/// Se guarda porque `soltar` tiene que revocarlo y `cap` no ofrece "revoca todo
/// lo de este tipo" -- solo por handle o todo lo del proceso, y lo segundo se
/// llevaria por delante su entrada y su consola. Vale `0` cuando no hay dueno.
static HANDLE: AtomicU64 = AtomicU64::new(0);
/// * El PRIMER proceso que reclamo la pantalla. Nunca se borra.
///
/// Es el compositor: la reclama al arrancar, antes que nadie. Se guarda para que
/// el rescate por teclado sepa **a quien NO puede echar** -- si echara al
/// escritorio, la tecla de emergencia seria la tecla de romper la maquina.
///
/// Es una heuristica y se dice: "el primero que la pidio es el escritorio" es
/// cierto hoy porque el escritorio ES el arranque. El dia que haya varios
/// compositores, esto tiene que pasar a ser una marca explicita del BEF, como
/// `WANTS_SCREEN`.
static DUENO_PRIMERO: AtomicU32 = AtomicU32::new(SIN_DUENO);

/// Ya la tiene otro proceso.
pub const ERROR_OCUPADO: u32 = 16;
/// Esta maquina arranco sin GOP: no hay pantalla que ceder.
pub const ERROR_SIN_PANTALLA: u32 = 17;

// Operaciones sobre un handle KIND_FRAMEBUFFER.
//
// Cada una devuelve UN `u64` porque eso es lo que cabe en `BmoStatus.value`.
// Los campos que van juntos viajan empaquetados en vez de gastar una llamada
// por numero: son datos que se leen una vez al arrancar el compositor.
/// Direccion virtual (en el espacio del proceso) donde quedo mapeada.
pub const FB_OP_BASE: u64 = 0x01;
/// `(ancho << 32) | alto`, en pixeles.
pub const FB_OP_DIMS: u64 = 0x02;
/// `(stride << 32) | formato`. El stride va en PIXELES, no en bytes -- es el
/// mismo numero que usa el kernel, y convertirlo aqui seria inventar una
/// unidad distinta a los dos lados de la frontera.
pub const FB_OP_STRIDE: u64 = 0x03;
/// Bytes mapeados en total. Es lo que hace falta para un `rep stosd` que
/// llene la pantalla entera sin multiplicar nada.
pub const FB_OP_BYTES: u64 = 0x04;

/// Bytes que ocupa el framebuffer, redondeado a pagina.
fn bytes_mapeados() -> u64 {
    let alto = unsafe { crate::info::FB_HEIGHT } as u64;
    let stride = unsafe { crate::info::FB_STRIDE } as u64;
    let crudo = alto * stride * 4;
    (crudo + mm::PAGE - 1) & !(mm::PAGE - 1)
}

/// Concede la pantalla al proceso `pid` y la mapea en `aspace`.
///
/// Devuelve el handle, o el error. El mapeo es U/S + escritura sobre el mismo
/// rango fisico que usa el kernel: no hay copia ni doble bufer aqui -- el
/// proceso escribe donde el escaner lee. El doble bufer, si lo quiere, lo pone
/// el en su propia memoria, que es exactamente donde debe vivir esa decision.
pub fn reclamar(pid: u32, aspace: u64) -> Result<u64, u32> {
    if !crate::info::hay_fb_crudo() {
        return Err(ERROR_SIN_PANTALLA);
    }
    // Un solo dueno. `compare_exchange` y no "leer y luego escribir": dos
    // procesos pidiendola en el mismo tick no pueden ganar los dos.
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
        // * WC y no normal: aqui se escriben millones de pixeles seguidos, y
        // juntar las escrituras es lo que separa repintar una ventana en
        // milisegundos de hacerlo en decenas. Ver `s1_cpu::init_pat`.
        if vmm::map_page_wc(aspace, vmm::FRAMEBUFFER_VA_BASE + off, fisica + off, true, true)
            .is_err()
        {
            // Mapeo a medias = paginas de pantalla sueltas en un espacio de
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

    // A partir de aqui el kernel no dibuja. El orden importa: ceder DESPUES de
    // que el mapeo y el handle esten hechos, para que un fallo a medias no
    // deje la maquina ciega y sin dueno.
    HANDLE.store(handle, Ordering::SeqCst);
    // El primero, y solo el primero. `compare_exchange` para que no lo mueva
    // una segunda reclamacion.
    let _ = DUENO_PRIMERO.compare_exchange(SIN_DUENO, pid, Ordering::SeqCst, Ordering::SeqCst);
    crate::info::ceder_fb(true);
    crate::ring0::cabina::info("fb", "pantalla cedida a Ring 3", pid as u64);
    Ok(handle)
}

/// * SOLTAR LA PANTALLA SIN MORIRSE. El dueno la devuelve y sigue vivo.
///
/// # Por que faltaba, y que desbloquea
///
/// Habia `reclamar` y habia [`proceso_muerto`], o sea que **la unica forma de
/// soltar la pantalla era morir**. Consecuencia concreta: `gui.bex` la reclama
/// al arrancar y no la suelta jamas, asi que cualquier programa que la pida
/// desde el escritorio se lleva un
///
/// ```text
/// la pantalla ya tiene dueno: el escritorio la reclamo al arrancar
/// ```
///
/// y eso incluye `ray.bex`, el ensayo general de DOOM. El compositor tenia
/// razon en no cederla a cualquiera que la pida --uno que lo hiciera no serviria
/// de compositor-- pero sin esta funcion **no podia cederla ni queriendo**.
///
/// # La diferencia con [`proceso_muerto`], que no es cosmetica
///
/// Alli no se desmapea nada porque el espacio de direcciones entero se destruye
/// con el proceso. **Aqui el proceso sigue vivo**, asi que sus paginas de
/// framebuffer hay que quitarlas de verdad: dejarlas mapeadas seria un proceso
/// que ya no es dueno de la pantalla y puede seguir escribiendo en ella --
/// exactamente el agujero que el modelo de un solo dueno existe para cerrar.
///
/// El handle tambien se revoca. Un handle vivo a una capability que ya no te
/// pertenece es la clase de cabo suelto que funciona hasta que dos procesos lo
/// usan a la vez.
///
/// # Orden
///
/// Se marca `SIN_DUENO` **al final**: mientras se desmapea, la pantalla sigue
/// siendo de quien la suelta. Al reves habria un intervalo en el que otro puede
/// reclamarla y mapearla mientras el anterior todavia tiene las paginas.
pub fn soltar(pid: u32, aspace: u64) -> Result<(), u32> {
    if DUENO.load(Ordering::SeqCst) != pid {
        // No es suya. Se dice en vez de contestar OK: un "si" a quien no era
        // dueno le haria creer que la cedio.
        return Err(ERROR_OCUPADO);
    }
    let bytes = bytes_mapeados();
    let mut off = 0u64;
    while off < bytes {
        vmm::unmap_page(aspace, vmm::FRAMEBUFFER_VA_BASE + off);
        off += mm::PAGE;
    }
    let h = HANDLE.swap(0, Ordering::SeqCst);
    if h != 0 {
        cap::revoke(pid, h);
    }
    crate::info::ceder_fb(false);
    DUENO.store(SIN_DUENO, Ordering::SeqCst);
    crate::ring0::cabina::info("fb", "pantalla SOLTADA por su dueno", pid as u64);
    Ok(())
}

/// ** EL RESCATE. Le quita la pantalla al dueno actual **sin pedirle permiso**.
///
/// Devuelve el `pid` al que se la quito, o `None` si no habia nada que rescatar.
///
/// # Por que esto tiene que existir
///
/// Todo lo demas de este modulo asume que el dueno colabora: la suelta al morir,
/// o la suelta porque quiere. Un programa que se queda la pantalla **y la
/// entrada** y no coopera tiene la maquina de rehen, y eso paso de verdad: el
/// raycaster tomo las dos y no podia leer su propio ESC. Sin teclado, sin
/// escritorio y sin forma de volver que no fuera el boton de reinicio.
///
/// Eddi lo llamo por su nombre: *"eso me recuerda a ransomware, eso que le quita
/// todo el control"*. Es exactamente la misma forma, y da igual que la causa sea
/// malicia o un `if` que falta.
///
/// **Un sistema donde un programa puede quedarse el teclado para siempre no es
/// un sistema seguro: es un sistema con suerte.** Por eso la tecla de rescate
/// vive en el KERNEL, que es quien ve las teclas antes que nadie y a quien no se
/// le puede quitar ese sitio.
///
/// # A quien NO echa
///
/// Al primer dueno, que es el compositor (reclama al arrancar). Si echara al
/// escritorio, la tecla de emergencia seria la tecla de romper la maquina. Ver
/// [`DUENO_PRIMERO`] -- es una heuristica y esta dicha alli.
///
/// # Y por que DESMAPEA
///
/// Marcar la pantalla como libre no basta: el programa seguiria teniendo sus
/// paginas mapeadas y seguiria escribiendo encima del escritorio. Dos duenos
/// pintando el mismo sitio es peor que uno pintando mal. Se desmapea con el
/// `cr3` que da el planificador, y entonces su siguiente pixel es un fallo de
/// pagina -- que es la respuesta correcta a "ya no es tuya".
pub fn rescatar() -> Option<u32> {
    let actual = DUENO.load(Ordering::SeqCst);
    if actual == SIN_DUENO {
        return None;
    }
    if actual == DUENO_PRIMERO.load(Ordering::SeqCst) {
        // Es el escritorio. No se echa al que sostiene la casa.
        return None;
    }
    let aspace = crate::ring0::task::scheduler::cr3_de_pid(actual)?;
    if soltar(actual, aspace).is_err() {
        return None;
    }
    crate::ring0::cabina::warn("fb", "pantalla RESCATADA por el teclado", actual as u64);
    Some(actual)
}

/// El proceso `pid` murio (o salio). Si era el dueno, el kernel recupera la
/// pantalla. Lo llama `cap::revoke_all`, que corre en TODAS las salidas --
/// EXIT voluntario y muerte por fault.
///
/// No se desmapea nada: el espacio de direcciones entero se destruye con el
/// proceso, y desmapear paginas de un CR3 que esta a punto de morir es
/// trabajo para nadie.
pub fn proceso_muerto(pid: u32) {
    if DUENO
        .compare_exchange(pid, SIN_DUENO, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        // El handle muere con el proceso, pero el static no: dejarlo puesto
        // haria que un `soltar` posterior intentase revocar el handle de un
        // muerto. Se limpia aqui, que es donde la propiedad cambia.
        HANDLE.store(0, Ordering::SeqCst);
        crate::info::ceder_fb(false);
        // WARN y no INFO: el que suelta la pantalla es el que la estaba
        // pintando, o sea el escritorio. Que muera NO es rutina -- es la
        // diferencia entre acabar en la ventana o acabar en el shell de Ring 0
        // sin saber por que. En verde entre veinte lineas verdes no se ve; en
        // ambar, en una foto de CABINA, si.
        crate::ring0::cabina::warn(
            "fb",
            "el dueno de la pantalla MURIO: se vuelve al panel del kernel",
            pid as u64,
        );
        // * Y SUS ULTIMAS PALABRAS, aqui y ahora.
        //
        // El manejador de panico del compositor dice el archivo y la linea
        // exactos... por la consola del kernel, que **mientras el tenia la
        // pantalla no se pintaba** (`has_fb()` estaba en falso porque la
        // pantalla era suya). O sea que el unico mensaje capaz de explicar la
        // muerte se escribia justo en el intervalo en el que nadie podia
        // leerlo.
        //
        // Este es el instante exacto en que la pantalla vuelve a ser del
        // kernel, asi que es el primer momento en que se puede pintar -- y el
        // ultimo en que alguien se acuerda de preguntar. Guardarlo para el
        // arranque del shell no bastaba: quien relanza a mano el escritorio no
        // pasa por ahi.
        //
        // [!] **Bajo la CR3 del KERNEL.** Esto corre dentro de `revoke_all`, o
        // sea dentro del syscall del proceso que se esta muriendo: la CR3 en
        // vigor es la SUYA, y su espacio comparte identidad solo en 0..1 GiB.
        // El framebuffer vive a ~3,5 GiB: pintar aqui sin cambiar de CR3 seria
        // un #PF, y el reporte de faults tambien pinta -> #PF recursivo en IST1
        // -> congelacion total y silenciosa. Es la mina que ya costo dos
        // sesiones (ver el patron del rango identidad alto), y la misma danza
        // que hace `uconsole::flush` por la misma razon.
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
                // * Y a CABINA, que es lo que de verdad sobrevive.
                //
                // El log del kernel lo tapa el escritorio siguiente en cuanto
                // se relanza: el mensaje se pinta, y dos segundos despues hay
                // un escritorio entero encima. CABINA tiene anillo propio y su
                // panel se sigue viendo con el escritorio puesto -- en la ultima
                // foto se leia perfectamente por debajo de la ventana.
                crate::ring0::cabina::warn("gui", linea, pid as u64);
            });
        } else {
            crate::ring0::core::phase::dashboard_log("  | (nada: murio sin decir una sola linea)");
            crate::ring0::cabina::warn("gui", "murio sin decir una sola linea", pid as u64);
        }
        if cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(cur);
        }
    }
}

/// Pid del dueno actual, o `None`.
pub fn dueno() -> Option<u32> {
    match DUENO.load(Ordering::SeqCst) {
        SIN_DUENO => None,
        pid => Some(pid),
    }
}

/// Despacho de las operaciones sincronas sobre la capability ya resuelta.
/// `base` es el objeto que guarda la capability: la VA donde se mapeo.
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
