//! **THE DESKTOP SUPERVISOR** -- launch it, notice when it dies, decide about
//! trying again.
//!
//! === Why this is a file of its own, and it is the clearest case in `core/` ===
//!
//! This is 156 lines of restart policy that were sitting inside `phase.rs`, a
//! file named after the boot sequence. The launch does happen during boot, but
//! **the deciding does not**: `desktop_died` and the attempt counter are
//! consulted for as long as the machine is up.
//!
//! A reader looking for "what happens when the desktop crashes" had no reason
//! to open a file about phases, and that is exactly the small-needle problem --
//! the answer was findable only by already knowing where it was.
//!
//! === The policy, which is the part worth stating out loud ===
//!
//! The desktop is retried a **bounded** number of times (`DESKTOP_MAX_ATTEMPTS`
//! = 2) and then left down with a report on screen. Unbounded retry of a thing
//! that paints the screen is not resilience: it is a machine that flickers
//! forever and never tells you why.
//!
//! [!] Names went to English; the text it prints stays in Spanish.

use super::shell::ui::{row, L};
use super::dashboard::{dash_log, dashboard_log, dashboard_log_color};
use super::phase::s_log;
use super::shell::ui::{SH_TITLE, SH_VALUE};

/// Donde vive el escritorio en el volumen de datos.
///
/// Es un CONTRATO, no una constante de conveniencia: quien quiera otro
/// escritorio deja su `.bex` ahi y arranca. Eso es exactamente lo que NO se
/// podia hacer mientras el compositor viajaba dentro del kernel.
///
/// * `gui` y no `compositor` porque **el nombre tiene que caber en 8.3**. El
/// driver FAT32 de `fs.rs` no lee nombres largos y se NIEGA a recortar: un
/// nombre recortado en silencio abre otro archivo, y en un cargador de
/// programas eso significa ejecutar otro binario. `compositor` son diez
/// caracteres; no cabia, y el fallo habria salido como `NameTooLong` despues
/// de copiarlo -- o sea, despues de creer que ya estaba.
///
/// `gui` ademas es el nombre que ya usa el crate (`bmo-service-gui`) y la
/// etiqueta con la que habla en CABINA. Un nombre, no tres.
pub(crate) const COMPOSITOR_PATH: &str = "sys/gui.bex";

/// Arranca el escritorio desde el disco. Va DESPUES de montar el volumen de
/// datos -- antes no habria de donde leerlo.
///
/// * Si arranca, el panel del kernel deja de verse: al reclamar la pantalla el
/// kernel deja de dibujar, y el compositor no termina nunca (si terminara,
/// `revoke_all` devolveria la pantalla y el panel se repintaria encima). Los
/// logs siguen enteros por serie y en CABINA, y un fault de kernel recupera la
/// pantalla para contarlo.
///
/// * Si NO arranca, no pasa nada malo: queda el panel y el shell de Ring 0, que
/// es un sistema perfectamente usable. Por eso esto no se planta ni reintenta --
/// pero lo DICE, y dice que hacer. Un escritorio que no sale sin explicar por
/// que manda a alguien a leer codigo.
/// El tid del escritorio, para poder preguntar despues si sigue vivo.
/// `0` = no se admitio.
static mut DESKTOP_TID: u32 = 0;
/// * Y su PID, que **no es el mismo numero**. `vive()` pregunta por tid (el
/// hilo) y `uconsole` guarda las lineas por pid (el proceso). Confundirlos aqui
/// haria que el informe de defuncion leyera las ultimas palabras de otro -- o de
/// nadie, que es peor, porque pareceria que se murio callado.
static mut DESKTOP_PID: u32 = 0;

/// Se admitio el escritorio y ya no esta?
///
/// Admitir no es arrancar: el compositor puede morirse a los pocos ticks --y se
/// ha muerto-- dejando la maquina en el panel del kernel. Hasta ahora eso no lo
/// decia nadie y habia que deducirlo de que la ventana no salia.
pub(crate) fn desktop_died() -> bool {
    let tid = unsafe { DESKTOP_TID };
    tid != 0 && !crate::ring0::task::scheduler::vive(tid)
}

/// Cuantas veces se ha intentado levantar el escritorio.
pub(crate) static mut DESKTOP_ATTEMPTS: u32 = 0;
/// Tope de relanzamientos automaticos.
///
/// Dos y no infinitos: un compositor que muere por una condicion de carrera del
/// arranque se levanta al segundo intento, y uno que muere por un bug lo hara
/// las veces que se le pida. Reintentar sin tope convertiria un fallo visible
/// en una maquina que parpadea para siempre -- y encima borrando su propio log
/// en cada vuelta. Es la misma leccion que los puertos USB.
pub(crate) const DESKTOP_MAX_ATTEMPTS: u32 = 2;

pub(crate) fn start_desktop() {
    use crate::ring0::task::lanzar;

    unsafe { DESKTOP_ATTEMPTS += 1 };
    let inf = lanzar::ruta(COMPOSITOR_PATH);
    match inf.res {
        Ok(tid) => {
            // * Arrancar y no decir su pid es un caso RARO, y por eso mismo
            // valia la pena distinguirlo: `unwrap_or(0)` lo dejaba en 0, que es
            // un pid con dueno. A partir de ahi, todo lo que se decida "para el
            // escritorio" mirando `DESKTOP_PID` apunta a otro.
            let pid = match inf.pid {
                Some(p) => p,
                None => {
                    crate::ring0::cabina::warn(
                        "gui", "el escritorio arranco y NO dijo su pid (queda 0)", tid as u64);
                    0
                }
            };
            unsafe {
                DESKTOP_TID = tid;
                DESKTOP_PID = pid;
            }
            row("escritorio", |l| {
                l.txt(COMPOSITOR_PATH);
                l.txt("   tid ");
                l.dec(tid as u64);
                l.txt("  pid ");
                l.dec(pid as u64);
            });
            crate::ring0::cabina::info("gui", "escritorio admitido desde disco", tid as u64);
        }
        Err(f) => {
            row("escritorio", |l| { l.txt("NO ARRANCA: "); l.txt(f.motivo()); });
            row("   copia", |l| { l.txt(COMPOSITOR_PATH); l.txt(" al volumen de datos"); });
            row("   o bien", |l| { l.txt("run "); l.txt(COMPOSITOR_PATH); l.txt("   desde este shell"); });
            crate::ring0::cabina::warn("gui", f.motivo(), 0);
        }
    }
}

/// **El informe de defuncion del escritorio.**
///
/// * Antes esto decia *"mira el panico en el log de arriba"*. Y el panico SI
/// estaba: el manejador del compositor imprime archivo y linea exactos. Solo que
/// el log del kernel sigue corriendo, asi que para cuando se mira la pantalla
/// esa linea ya subio y salio -- tres arranques seguidos con la respuesta
/// delante y sin poder leerla.
///
/// Ahora se reimprimen **sus ultimas palabras**, guardadas por `uconsole`
/// mientras aun vivia. Un registrador de vuelo que borra la caja negra al
/// aterrizar no es un registrador de vuelo.
pub(crate) fn death_report() {
    let tid = unsafe { DESKTOP_TID };
    // * Por PID, no por tid. Ver la nota de `DESKTOP_PID`.
    let pid = unsafe { DESKTOP_PID };
    row("escritorio", |l| {
        l.txt("MURIO tras arrancar, tid ");
        l.dec(tid as u64);
        l.txt(" -- esto es lo ULTIMO que dijo:");
    });
    if crate::ring0::uconsole::hubo_palabras(pid) {
        crate::ring0::uconsole::ultimas_palabras(pid, |linea| {
            row("   |", |l| { l.txt(linea); });
        });
    } else {
        // Que no dijera nada TAMBIEN es un dato: significa que se murio antes
        // de llegar a su primer mensaje, o que ni siquiera entro a CPL3.
        row("   |", |l| { l.txt("(nada: murio antes de decir una sola linea)"); });
    }
    row("   relanzar", |l| { l.txt("run "); l.txt(COMPOSITOR_PATH); });
    crate::ring0::cabina::warn("gui", "el escritorio murio tras arrancar", tid as u64);
}

/// Espera a que los programas de ejemplo de Ring 3 terminen.
///
/// Con tope de tiempo: uno que se cuelgue no puede impedir que arranque el
/// escritorio. Y con `hlt` en el bucle -- girar en vacio aqui seria quitarle al
/// planificador el CPU que necesita justo para que esos programas avancen.
pub(crate) fn wait_for_demo_tasks() {
    use crate::ring0::plat::timer;
    let limite = timer::ticks() + 400; // ~400 ms si el tick es de 1 ms
    loop {
        let (_total, listos) = crate::ring0::task::scheduler::counts();
        // 1 = solo queda la tarea del kernel. Los demos han acabado.
        if listos <= 1 {
            break;
        }
        if timer::ticks() > limite {
            crate::ring0::cabina::warn(
                "ring3", "los demos no acabaron a tiempo: se sigue igual", listos as u64);
            break;
        }
        unsafe { core::arch::asm!("hlt") };
    }
}
