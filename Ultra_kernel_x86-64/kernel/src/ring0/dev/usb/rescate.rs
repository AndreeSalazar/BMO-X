//! **El atajo que le devuelve la maquina al dueno: `Ctrl+Alt+Esc`.**
//!
//! [carril]  ROJO      Ctrl+Alt+Esc. Es el ultimo recurso: si falla, no hay otro
//!
//! Salio de `dev/usb/mod.rs` el 2026-08-12. Se puede sacar solo porque **es
//! POLITICA y no driver**: no habla con el xHC ni con un endpoint, decide que
//! pasa cuando un programa se queda la pantalla y la entrada.
//!
//! Vive del lado del kernel y no del compositor por una razon que ya se pago:
//! un programa con `KIND_INPUT` se queda TODAS las teclas, incluida la que
//! serviria para quitarselas. Si el rescate viviera en el escritorio, el primer
//! programa que tomara la entrada lo desactivaria -- y eso paso: el raycaster se
//! quedo pantalla y entrada y no habia forma de volver que no fuera el boton de
//! reinicio. El dueno lo dijo con la palabra exacta: *"eso me recuerda a
//! ransomware"*.
//!
//! > Un sistema donde un programa puede quedarse el teclado para siempre no es
//! > un sistema seguro: es un sistema con suerte.

use super::{modificadores, HELD_CODE, MOD_ALT, MOD_CTRL};

/// ** LA TECLA QUE NO SE PUEDE QUITAR: `Ctrl+Alt+Esc`.
///
/// Se mira AQUI y no en Ring 3, y esa es toda la idea. `poll_ascii` es el punto
/// unico por el que pasan las teclas --el shell de Ring 0 y el
/// `INPUT_OP_TECLA` de cualquier proceso que tenga la capability-- asi que una
/// comprobacion en este sitio **la ve nadie puede saltar**.
///
/// # Por que el kernel y no el compositor
///
/// Porque la entrada es EXCLUSIVA. Un programa que tiene `KIND_INPUT` se queda
/// todas las teclas, incluido el atajo que serviria para quitarselas. Si el
/// rescate viviera en el escritorio, el primer programa que tomara la entrada lo
/// desactivaria -- y eso ya paso: el raycaster se quedo pantalla y entrada, y no
/// habia forma de volver que no fuera el boton de reinicio.
///
/// Eddi lo dijo con la palabra exacta: **"eso me recuerda a ransomware"**. La
/// forma es la misma, y da igual si la causa es malicia o un `if` que falta.
///
/// > Un sistema donde un programa puede quedarse el teclado para siempre no es
/// > un sistema seguro: es un sistema con suerte.
///
/// # Que hace
///
/// Le quita la pantalla al dueno actual (ver `fb::rescue`, que no echa al
/// compositor) y la entrada. El escritorio esta esperando en su bucle a que el
/// dueno vuelva a `0`, asi que **se recupera solo** -- no hace falta avisarle.
///
/// # Y la tecla NO se entrega
///
/// Devuelve `None` para que el atajo no acabe ademas escrito en la caja del
/// escritorio ni movido al programa. Un atajo que hace dos cosas es un atajo que
/// hay que deshacer.
pub(super) fn tecla_del_dueno(t: Option<u8>) -> Option<u8> {
    let b = t?;
    // 27 = ESC. Con Ctrl y Alt a la vez: tres teclas, imposible de pulsar por
    // accidente y con la misma memoria muscular que el Ctrl+Alt+Del de toda la
    // vida.
    if b != 27 {
        return Some(b);
    }
    let m = modificadores();
    if m & MOD_CTRL == 0 || m & MOD_ALT == 0 {
        return Some(b);
    }
    if rescue_owner() { None } else { Some(b) }
}

/// The rescue itself, **split out so BOTH keyboard doors can call it**.
///
/// # Why this function exists, and it is a bug that reached metal
///
/// The rescue used to live entirely inside [`tecla_del_dueno`], which only looks
/// at the CHARACTER queue. And there are two keyboard doors, not one:
///
/// * `INPUT_OP_TECLA` -> [`poll_ascii`] -> characters. This one was checked.
/// * `INPUT_OP_EVENTO_TECLA` -> [`evento_tecla`] -> raw keys. **This one wasn't.**
///
/// The second door was added so games could see key *releases*, which means it
/// is used by exactly the kind of program that takes the screen and the input.
/// Result: **the anti-hijack shortcut was not watching the hijacker's door.** A
/// program reading raw keys was immune to Ctrl+Alt+Esc, and the only way out was
/// the reset button -- the very thing this mechanism exists to avoid.
///
/// Returns `true` if there was someone to rescue.
fn rescue_owner() -> bool {
    match crate::ring0::obj::fb::rescue() {
        Some(pid) => {
            let _ = crate::ring0::obj::input::release(pid);
            crate::ring0::cabina::warn("input", "entrada RESCATADA por el teclado", pid as u64);
            unsafe { PRIMER_INTENTO = 0 };
            true
        }
        // Nadie a quien rescatar. Y hasta el 2026-08-26 eso era el final de la
        // historia -- ver `segunda_llamada`.
        None => segunda_llamada(),
    }
}

/// Cuando se pidio ayuda y no habia a quien rescatar. `0` = no hay intento vivo.
static mut PRIMER_INTENTO: u64 = 0;

/// Segundos que dura la ventana de la segunda llamada.
///
/// Tres: bastante para pulsar el atajo dos veces a conciencia, poco para que dos
/// pulsaciones separadas por un cafe cuenten como una insistencia.
const VENTANA_S: u64 = 3;

/// **LA SEGUNDA LLAMADA: `Ctrl+Alt+Esc` otra vez, y esta SI echa al escritorio.**
///
/// # El agujero que esto cierra, y lo conto el metal
///
/// El 2026-08-26 el dueno abrio la calculadora, tecleo `10 * 60 =`, el motor de
/// COBOL se murio, y **`Ctrl+Alt+Esc` no hizo nada**. No era un bug: `fb::rescue`
/// se niega a echar al PRIMER dueno --el escritorio-- porque *"seria la tecla de
/// romper la maquina"*.
///
/// *** A ese razonamiento le faltaba un dato: **`run_shell` no se para nunca.**
/// Es un bucle que no retorna y que sigue leyendo el teclado mientras el
/// escritorio corre. Hay donde aterrizar, asi que quitarle la pantalla al
/// escritorio no es romper la maquina -- es volver al sitio del que se salio.
///
/// # Por que DOS pulsaciones y no una
///
/// Porque las dos cosas son verdad a la vez:
///
/// ```text
///    una pulsacion    puede ser un error, y tirar el escritorio por un error
///                     es exactamente lo que la version anterior evitaba
///    dos seguidas     ya no es un error: es alguien diciendo "de verdad"
/// ```
///
/// La primera no hace nada visible salvo apuntar la hora y dejarlo dicho en
/// CABINA. La segunda, dentro de la ventana, da la patada.
///
/// [!] Y el ESC **se traga igual en la primera**, aunque no rescate a nadie. Si
/// se entregara, la aplicacion recibiria un ESC que el usuario no le mando -- y
/// en un dialogo eso es un "cancelar" que nadie pulso.
fn segunda_llamada() -> bool {
    use crate::ring0::task::scheduler;
    let ahora = scheduler::rdtsc();
    let hz = scheduler::tsc_freq();
    let anterior = unsafe { PRIMER_INTENTO };
    // Sin TSC medido no hay ventana que medir. Se trata como primera llamada
    // siempre: **mejor no dar la patada que darla sin querer.** Es la regla de
    // los jueces de esta casa -- cuando falta un dato, la respuesta es la que no
    // asume.
    let dentro = hz != 0 && anterior != 0 && ahora.wrapping_sub(anterior) < hz * VENTANA_S;
    if !dentro {
        unsafe { PRIMER_INTENTO = ahora };
        crate::ring0::cabina::warn(
            "input",
            "la pantalla la tiene el ESCRITORIO: pulsa otra vez para echarlo",
            0,
        );
        // Se traga el ESC igual. Ver la nota de arriba.
        return true;
    }
    unsafe { PRIMER_INTENTO = 0 };
    let echado = crate::ring0::obj::fb::rescate_de_emergencia();
    if let Some(pid) = echado {
        let _ = crate::ring0::obj::input::release(pid);
        crate::ring0::cabina::warn(
            "input",
            "SEGUNDA llamada: el escritorio pierde la pantalla",
            pid as u64,
        );
    }
    // *** Y AHORA LA LIMPIEZA ENTERA, no solo el dueno de la pantalla.
    //
    // ** Lo pidio el dueno con la maquina en la mano: *"que haga limpieza total
    // en la RAM en Ring 3 como si estuviera reiniciando, porque ya llevo asi
    // repitiendo constantemente"*.
    //
    // Hasta hoy esta tecla echaba **al dueno de la pantalla** y a nadie mas.
    // Devuelve la imagen --que era el problema del 26-08-- y deja en pie a todos
    // los demas procesos de Ring 3, con su espacio, sus capabilities y sus
    // marcos. Y despues se relanza el escritorio ENCIMA de eso.
    //
    // > Una vuelta a cero que no vuelve a cero no es un punto de partida: es
    // > otro estado mas, y encima uno que nadie ha escrito.
    //
    // *** Y esto vale aunque NO sea la causa de la pantalla azul, que es lo que
    // se esta investigando: **una patada que limpia a medias no sirve para
    // descartar nada**. Despues del segundo intento ya no se sabe que quedaba de
    // antes, y sin eso ningun arranque contesta una pregunta.
    //
    // El desmontaje de verdad lo hace `reap`, que es el unico sitio que ya sabe
    // hacerlo bien. Esto solo dice quienes mueren.
    // *** Y NO SOLO MARCAR: PURGAR Y CONTAR.
    //
    // ** La primera version marcaba las tareas y se iba, y el dueno lo llamo
    // por su nombre: *"se siente que es superficial"*. Tenia razon, y el
    // defecto era EL MISMO que el de la patada vieja, un nivel mas arriba:
    //
    // ```text
    //    la patada vieja   echaba al dueno de la pantalla y a nadie mas
    //    la limpieza v1    marcaba a todos... y no comprobaba nada
    // ```
    //
    // Las dos dejan la maquina en un estado que nadie puede nombrar. Ahora
    // `core::purga` cierra, **cede el CPU hasta que `reap` recoge**, y dice
    // cuantos marcos y cuantas ranuras volvieron. Ver `core/purga.rs`.
    let parte = crate::ring0::core::purga::purgar();
    crate::ring0::core::purga::contar(&parte);
    echado.is_some() || parte.tareas > 0
}

/// ESC as a **Set 1** scancode, which is what the raw queue carries (`hid_to_ps2`
/// maps HID usage 0x29 to this 0x01). It is NOT the 27 of the character queue:
/// they are two different alphabets, and confusing them leaves the shortcut mute
/// without a single compile error.
const SC1_ESC: u8 = 0x01;

/// If the rescue swallowed the ESC PRESS, it also swallows its RELEASE.
///
/// Without this the program would get a "ESC released" for a key it never saw
/// pressed. Not fatal -- almost nobody looks at the ESC release -- but an
/// unpaired event is the kind of thing that costs an afternoon to find.
static mut SWALLOW_ESC_RELEASE: bool = false;

/// [`rescue_owner`] seen from the RAW door. Counterpart of [`tecla_del_dueno`].
pub(super) fn raw_key_from_owner(t: Option<(u8, bool)>) -> Option<(u8, bool)> {
    let (sc, pressed) = t?;
    if sc != SC1_ESC {
        return Some((sc, pressed));
    }
    if !pressed {
        // The ESC release whose press the rescue already ate.
        if unsafe { SWALLOW_ESC_RELEASE } {
            unsafe { SWALLOW_ESC_RELEASE = false };
            return None;
        }
        return Some((sc, pressed));
    }
    let m = modificadores();
    if m & MOD_CTRL == 0 || m & MOD_ALT == 0 {
        return Some((sc, pressed));
    }
    if rescue_owner() {
        unsafe { SWALLOW_ESC_RELEASE = true };
        None
    } else {
        Some((sc, pressed))
    }
}

/// **The rescue, checked without consuming anything.**
///
/// The thread must not pop from the queues: those keys belong to the input owner
/// and stealing them would trade one bug for another. And it does not need to --
/// `Ctrl`, `Alt` and the held key are **state**, not queue:
///
/// * `modificadores()` reads the flags the poll already maintains;
/// * `HELD_CODE` is the scancode of the key that is down RIGHT NOW, which the
///   key-repeat path needs anyway and therefore already existed.
///
/// So the question *"is the user calling for help at this instant?"* is answered
/// by looking, and the program loses no key at all when the answer is no.
pub(super) fn watch_rescue() {
    let held = unsafe { HELD_CODE };
    if held != SC1_ESC {
        return;
    }
    let m = modificadores();
    if m & MOD_CTRL == 0 || m & MOD_ALT == 0 {
        return;
    }
    // `HELD_CODE` is not cleared here: the KeyUp clears it. What keeps the rescue
    // from firing sixty times a second while the combo is still held is that
    // `fb::rescue()` returns `None` as soon as there is no owner left.
    if rescue_owner() {
        unsafe { SWALLOW_ESC_RELEASE = true };
    }
}
