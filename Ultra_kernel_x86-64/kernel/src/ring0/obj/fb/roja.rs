//! **CARRIL ROJO** -- QUIEN tiene la pantalla, y el mapeo.
//!
//! [carril]  ROJO      el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
//!
//! [cuesta]  MAQUINA -- aqui se concede, se suelta, se rescata y se desmapea
//!           el framebuffer. Equivocarse deja la maquina CIEGA: dos duenos
//!           pintando el mismo sitio, o ninguno y sin panel al que volver.
//!
//! [riesgo]  AJENO ESPEJO
//!           AJENO  -- `process_died` corre DENTRO del syscall del que se
//!                     muere, o sea bajo SU `cr3`, y el framebuffer vive a
//!                     ~3,5 GiB. Pintar aqui sin cambiar a la del kernel es un
//!                     `#PF`; y el reporte de faults tambien pinta, o sea
//!                     `#PF` recursivo en IST1: congelacion muda. Costo dos
//!                     sesiones y la danza de CR3 sigue escrita ahi dentro.
//!           ESPEJO -- `OWNER`, `HANDLE` e `info::ceder_fb` son TRES sitios
//!                     que dicen la misma cosa. Soltar uno sin los otros deja
//!                     un kernel que cree que tiene pantalla y no la tiene.
//!
//! ** La linea con el verde de al lado, en una frase: aqui se cambia **quien
//! la tiene**; alli solo se contesta **que forma tiene**.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use crate::ring0::obj::cap;
use crate::ring0::mm::{self, vmm};
use super::verde::{mapped_bytes, ERROR_BUSY, ERROR_NO_SCREEN};

/// Nadie la tiene. Los pid validos son 0..MAX_PROCS, asi que hace falta un
/// centinela que no pueda ser un pid.
pub(super) const NO_OWNER: u32 = u32::MAX;


pub(super) static OWNER: AtomicU32 = AtomicU32::new(NO_OWNER);

/// El handle que se le concedio al dueno, para poder revocarlo si lo SUELTA.
///
/// Se guarda porque `release` tiene que revocarlo y `cap` no ofrece "revoca todo
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
static FIRST_OWNER: AtomicU32 = AtomicU32::new(NO_OWNER);


/// Concede la pantalla al proceso `pid` y la mapea en `aspace`.
///
/// Devuelve el handle, o el error. El mapeo es U/S + escritura sobre el mismo
/// rango fisico que usa el kernel: no hay copia ni doble bufer aqui -- el
/// proceso escribe donde el escaner lee. El doble bufer, si lo quiere, lo pone
/// el en su propia memoria, que es exactamente donde debe vivir esa decision.
pub fn claim(pid: u32, aspace: u64) -> Result<u64, u32> {
    if !crate::info::hay_fb_crudo() {
        return Err(ERROR_NO_SCREEN);
    }
    // Un solo dueno. `compare_exchange` y no "leer y luego escribir": dos
    // procesos pidiendola en el mismo tick no pueden ganar los dos.
    if OWNER
        .compare_exchange(NO_OWNER, pid, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(ERROR_BUSY);
    }

    let fisica = unsafe { crate::info::FB_ADDR };
    let bytes = mapped_bytes();
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
            let mut undo = 0u64;
            while undo < off {
                vmm::unmap_page(aspace, vmm::FRAMEBUFFER_VA_BASE + undo);
                undo += mm::PAGE;
            }
            OWNER.store(NO_OWNER, Ordering::SeqCst);
            return Err(ERROR_NO_SCREEN);
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
            OWNER.store(NO_OWNER, Ordering::SeqCst);
            return Err(cap::ERROR_PERMISSION_DENIED);
        }
    };

    // A partir de aqui el kernel no dibuja. El orden importa: ceder DESPUES de
    // que el mapeo y el handle esten hechos, para que un fallo a medias no
    // deje la maquina ciega y sin dueno.
    HANDLE.store(handle, Ordering::SeqCst);
    // El primero, y solo el primero. `compare_exchange` para que no lo mueva
    // una segunda reclamacion.
    let _ = FIRST_OWNER.compare_exchange(NO_OWNER, pid, Ordering::SeqCst, Ordering::SeqCst);
    crate::info::ceder_fb(true);
    crate::ring0::cabina::info("fb", "pantalla cedida a Ring 3", pid as u64);
    Ok(handle)
}


/// * SOLTAR LA PANTALLA SIN MORIRSE. El dueno la devuelve y sigue vivo.
///
/// # Por que faltaba, y que desbloquea
///
/// Habia `claim` y habia [`process_died`], o sea que **la unica forma de
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
/// # La diferencia con [`process_died`], que no es cosmetica
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
/// Se marca `NO_OWNER` **al final**: mientras se desmapea, la pantalla sigue
/// siendo de quien la suelta. Al reves habria un intervalo en el que otro puede
/// reclamarla y mapearla mientras el anterior todavia tiene las paginas.
pub fn release(pid: u32, aspace: u64) -> Result<(), u32> {
    if OWNER.load(Ordering::SeqCst) != pid {
        // No es suya. Se dice en vez de contestar OK: un "si" a quien no era
        // dueno le haria creer que la cedio.
        return Err(ERROR_BUSY);
    }
    let bytes = mapped_bytes();
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
    OWNER.store(NO_OWNER, Ordering::SeqCst);
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
/// [`FIRST_OWNER`] -- es una heuristica y esta dicha alli.
///
/// # Y por que DESMAPEA
///
/// Marcar la pantalla como libre no basta: el programa seguiria teniendo sus
/// paginas mapeadas y seguiria escribiendo encima del escritorio. Dos duenos
/// pintando el mismo sitio es peor que uno pintando mal. Se desmapea con el
/// `cr3` que da el planificador, y entonces su siguiente pixel es un fallo de
/// pagina -- que es la respuesta correcta a "ya no es tuya".
pub fn rescue() -> Option<u32> {
    let actual = OWNER.load(Ordering::SeqCst);
    if actual == NO_OWNER {
        return None;
    }
    if actual == FIRST_OWNER.load(Ordering::SeqCst) {
        // Es el escritorio. No se echa al que sostiene la casa.
        return None;
    }
    let aspace = crate::ring0::task::scheduler::cr3_de_pid(actual)?;
    if release(actual, aspace).is_err() {
        return None;
    }
    crate::ring0::cabina::warn("fb", "pantalla RESCATADA por el teclado", actual as u64);
    Some(actual)
}


/// **LA PATADA: el rescate que SI echa al escritorio.**
///
/// Devuelve el `pid` al que se le quito, o `None` si la pantalla ya era del
/// kernel.
///
/// # En que se diferencia de [`rescue`], y es lo unico que cambia
///
/// ```text
///    rescue()                  protege al PRIMER dueno (el escritorio)
///    rescate_de_emergencia()   no protege a nadie
/// ```
///
/// *** Y la diferencia no es de potencia: es de QUIEN LO PIDE.
///
/// `rescue` la dispara una tecla, o sea una persona, que puede haberse
/// equivocado -- y por eso no se le deja tirar la casa. Esto lo dispara el
/// kernel **despues de haber visto que su propia contabilidad esta danada**
/// (`core/emergencia.rs`). Un kernel que ya no se fia de Ring 3 y aun asi le
/// deja la pantalla no esta siendo prudente: esta apostando.
///
/// # [!] Y esto MATA al escritorio, dicho sin rodeos
///
/// Se desmapea, asi que su siguiente pixel es un `#PF` y la tarea cae. No es un
/// efecto secundario que se tolera: **es la respuesta correcta a "ya no es
/// tuya"**, la misma que ya documenta [`rescue`]. Lo que hace que se pueda pagar
/// ese precio es que hay donde aterrizar -- `run_shell` es un bucle que no
/// retorna y sigue leyendo el teclado.
pub fn rescate_de_emergencia() -> Option<u32> {
    let actual = OWNER.load(Ordering::SeqCst);
    if actual == NO_OWNER {
        // La pantalla ya es del kernel. No es un fallo: es que la corrupcion
        // llego cuando nadie la tenia, y entonces no hay nada que quitar.
        return None;
    }
    let aspace = crate::ring0::task::scheduler::cr3_de_pid(actual)?;
    if release(actual, aspace).is_err() {
        return None;
    }
    crate::ring0::cabina::warn(
        "fb",
        "pantalla RECUPERADA por la patada del kernel",
        actual as u64,
    );
    Some(actual)
}


/// El proceso `pid` murio (o salio). Si era el dueno, el kernel recupera la
/// pantalla. Lo llama `cap::revoke_all`, que corre en TODAS las salidas --
/// EXIT voluntario y muerte por fault.
///
/// No se desmapea nada: el espacio de direcciones entero se destruye con el
/// proceso, y desmapear paginas de un CR3 que esta a punto de morir es
/// trabajo para nadie.
pub fn process_died(pid: u32) {
    if OWNER
        .compare_exchange(pid, NO_OWNER, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        // El handle muere con el proceso, pero el static no: dejarlo puesto
        // haria que un `release` posterior intentase revocar el handle de un
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
        crate::ring0::core::dashboard::dashboard_log("  -- lo ULTIMO que dijo el dueno de la pantalla --");
        if crate::ring0::uconsole::hubo_palabras(pid) {
            crate::ring0::uconsole::ultimas_palabras(pid, |linea| {
                let mut buf = [0u8; 128];
                let cabeza = b"  | ";
                let n = linea.len().min(buf.len() - cabeza.len());
                buf[..cabeza.len()].copy_from_slice(cabeza);
                buf[cabeza.len()..cabeza.len() + n].copy_from_slice(&linea.as_bytes()[..n]);
                if let Ok(s) = core::str::from_utf8(&buf[..cabeza.len() + n]) {
                    crate::ring0::core::dashboard::dashboard_log(s);
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
            crate::ring0::core::dashboard::dashboard_log("  | (nada: murio sin decir una sola linea)");
            crate::ring0::cabina::warn("gui", "murio sin decir una sola linea", pid as u64);
        }
        if cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(cur);
        }
    }
}


