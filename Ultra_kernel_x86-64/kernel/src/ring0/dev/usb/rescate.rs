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
    // == ** LA SEGUNDA PULSACION MANDA SIEMPRE (corregido el 2026-09-01) =====
    //
    // ** El dueno lo probo y no paso nada: *"no se cumplio"*. Y la razon estaba
    // en el orden de este `match`:
    //
    // ```text
    //    fb::rescue() -> Some   le quita la pantalla al dueno y SE VA
    //    fb::rescue() -> None   ...y solo por AQUI se llegaba a la purga
    // ```
    //
    // O sea que la limpieza total solo ocurria cuando el dueno de la pantalla
    // era **el escritorio**, que es el unico a quien `fb::rescue` se niega a
    // echar. Con DOOM delante --que NO es el primer dueno-- la primera
    // pulsacion lo echaba, devolvia `Some`, y la purga no se pedia jamas.
    //
    // *** Asi que la ventana se mira ANTES que la pantalla, y con eso el atajo
    // tiene por fin dos significados limpios y predecibles:
    //
    // ```text
    //    una pulsacion    devuelveme la pantalla
    //    dos seguidas     REINICIA RING 3 -- mire quien mire la pantalla
    // ```
    //
    // La segunda ya no depende de QUIEN sea el dueno, que es exactamente lo que
    // hacia que la tecla se comportara distinto segun lo que hubiera delante.
    if segunda_pulsacion() {
        unsafe { PRIMER_INTENTO = 0 };
        // La pantalla, sin respetar al primer dueno: aqui ya se pidio dos veces.
        if let Some(pid) = crate::ring0::obj::fb::rescate_de_emergencia() {
            let _ = crate::ring0::obj::input::release(pid);
            crate::ring0::cabina::warn(
                "input", "SEGUNDA llamada: la pantalla vuelve al kernel", pid as u64);
        }
        // Y la limpieza entera. Se PIDE: la recoge el hilo del bus, porque esto
        // se alcanza tambien desde un syscall. Ver `core/purga.rs`.
        crate::ring0::cabina::warn(
            "input", "SEGUNDA llamada: se pide la PURGA de Ring 3 entero", 0);
        crate::ring0::core::purga::pedir();
        return true;
    }
    match crate::ring0::obj::fb::rescue() {
        Some(pid) => {
            let _ = crate::ring0::obj::input::release(pid);
            crate::ring0::cabina::warn("input", "entrada RESCATADA por el teclado", pid as u64);
            true
        }
        // Nadie a quien rescatar por las buenas. La ventana ya quedo abierta
        // arriba, asi que la siguiente pulsacion purga.
        None => {
            crate::ring0::cabina::warn(
                "input", "nadie a quien rescatar: pulsa otra vez para REINICIAR Ring 3", 0);
            true
        }
    }
}

/// **Es esta la SEGUNDA pulsacion dentro de la ventana?**
///
/// Si no lo es, abre la ventana y contesta `false`. Salio de `segunda_llamada`
/// el 2026-09-01 para poder preguntarlo **antes** de mirar quien tiene la
/// pantalla: mientras la pregunta vivio dentro de aquella funcion, la purga
/// dependia de que el dueno fuera el escritorio.
///
/// [!] Sin TSC medido no hay ventana que medir, y entonces se trata como
/// primera llamada SIEMPRE: **mejor no dar la patada que darla sin querer.** Es
/// la regla de los jueces de esta casa -- cuando falta un dato, la respuesta es
/// la que no asume.
fn segunda_pulsacion() -> bool {
    use crate::ring0::task::scheduler;
    // == *** SIN UN SOLTAR DE POR MEDIO NO ES OTRA PULSACION (2026-09-04) ====
    //
    // ** El dueno pulso `Ctrl+Alt+Esc` para volver del juego y se le murio
    // Ring 3 entero. No fue la purga: fue que la purga se PIDIO sola.
    //
    // `watch_rescue` corre en el hilo del bus **cada 4 ms** y dispara mientras
    // la tecla sigue pulsada. El suelo de 100 ms tapaba los primeros
    // veinticinco disparos... y al vigesimosexto, `d >= hz/10` se cumple y esto
    // devolvia `true`. **Una pulsacion humana normal dura de 100 a 200 ms**, o
    // sea que casi cualquier uso del atajo pedia la purga en el primer intento.
    //
    // *** Y el fichero llevaba escrita una proteccion que ya no protegia:
    //
    // > lo que impide que el rescate dispare sesenta veces por segundo mientras
    // > el combo sigue pulsado es que `fb::rescue()` devuelve `None` en cuanto
    // > no queda dueno
    //
    // Eso era verdad hasta el 01-09, cuando la ventana se movio DELANTE de la
    // pantalla para que la segunda llamada dejara de depender de quien fuera el
    // dueno. Aquel arreglo fue bueno **y dejo la rama de la purga por delante
    // del unico freno que tenia.** El comentario se quedo describiendo un muro
    // que ya no estaba en el camino.
    //
    // ** LA LECCION, y es la de toda la semana: **un TIEMPO no distingue lo que
    // un FLANCO si distingue.** "Dos pulsaciones" no es una cuestion de reloj:
    // es que entre una y otra hay que SOLTAR. Medirlo con un plazo es medir la
    // sombra de la pregunta.
    if !unsafe { SOLTADA } {
        return false;
    }
    // Se consume: una pulsacion fisica se evalua UNA vez, pase lo que pase
    // debajo. Sin esto, los disparos de los 4 ms volverian a contar.
    unsafe { SOLTADA = false };
    let ahora = scheduler::rdtsc();
    let hz = scheduler::tsc_freq();
    let anterior = unsafe { PRIMER_INTENTO };
    // ** EL SUELO SIGUE, PERO CAMBIA DE OFICIO (2026-09-04).
    //
    // Nacio el 01-09 para que mantener el atajo apretado no reiniciara Ring 3.
    // **No lo conseguia**: solo tapaba los primeros 100 ms de una pulsacion que
    // dura el doble. Ese trabajo lo hace ahora el flanco de aqui arriba.
    //
    // Lo que si hace, y por eso se queda: **REBOTE**. Un contacto fisico que
    // hace doble en unos milisegundos entregaria dos pulsaciones de verdad --
    // con su soltar y todo-- y ningun dedo humano pulsa dos veces en menos de
    // una decima. El numero era bueno; el oficio era otro.
    //
    // > Una confirmacion que se puede dar sin querer no confirma nada.
    let d = ahora.wrapping_sub(anterior);
    if hz != 0 && anterior != 0 && d >= hz / 10 && d < hz * VENTANA_S {
        return true;
    }
    // [!] Dentro de los 100 ms NO se reabre la ventana: es la MISMA pulsacion
    // repitiendose, y reabrirla moveria el reloj hacia adelante para siempre --
    // la segunda de verdad nunca caeria dentro del plazo.
    if hz != 0 && anterior != 0 && d < hz / 10 {
        return false;
    }
    unsafe { PRIMER_INTENTO = ahora };
    false
}

/// Cuando se pidio ayuda y no habia a quien rescatar. `0` = no hay intento vivo.
static mut PRIMER_INTENTO: u64 = 0;

/// Segundos que dura la ventana de la segunda llamada.
///
/// Tres: bastante para pulsar el atajo dos veces a conciencia, poco para que dos
/// pulsaciones separadas por un cafe cuenten como una insistencia.
const VENTANA_S: u64 = 3;

// ** `segunda_llamada` SE RETIRO el 2026-09-01, y su doctrina se quedo.
//
// Lo que hacia --la ventana de tres segundos y la patada al escritorio-- vive
// ahora repartido entre `segunda_pulsacion` y `rescue_owner`, y el motivo del
// reparto es que la pregunta *"es la segunda?"* tenia que poder hacerse ANTES
// de mirar quien tiene la pantalla. Mientras vivio aqui dentro, la purga
// dependia de que el dueno fuera el escritorio.
//
// Lo que sigue valiendo igual, y por eso se copia y no se borra:
//
//   * DOS pulsaciones y no una. Una puede ser un error, y tirar Ring 3 por un
//     error es lo que la version de antes del 26-08 evitaba. Dos seguidas ya no
//     es un error: es alguien diciendo "de verdad".
//
//   * El ESC se traga IGUAL en la primera. Si se entregara, la aplicacion
//     recibiria un ESC que el usuario no le mando -- y en un dialogo eso es un
//     "cancelar" que nadie pulso.
//
//   * Hay donde aterrizar: `run_shell` es un bucle que no retorna y sigue
//     leyendo el teclado. Por eso quitarle la pantalla al escritorio no es
//     romper la maquina, es volver al sitio del que se salio.

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

/// **Se solto el atajo desde la ultima vez que conto?**
///
/// Empieza en `true` porque al arrancar no hay nada pulsado, y la primera
/// pulsacion tiene que contar.
///
/// Es lo que convierte "dos pulsaciones" en una pregunta con respuesta: un
/// plazo mide cuanto ha pasado, y lo que hacia falta saber es si el dedo se
/// levanto. Ver `segunda_pulsacion`.
static mut SOLTADA: bool = true;

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
        // *** AQUI ES DONDE SE SABE QUE SE SOLTO, y es el unico sitio.
        //
        // `HELD_CODE` lo pone el KeyDown y lo limpia el KeyUp; este hilo pasa
        // por aqui cada 4 ms. Que la tecla no este es el FLANCO que separa dos
        // pulsaciones de una sostenida, y sin el la ventana de tres segundos no
        // distingue una cosa de la otra. Ver `segunda_pulsacion`.
        unsafe { SOLTADA = true };
        return;
    }
    let m = modificadores();
    if m & MOD_CTRL == 0 || m & MOD_ALT == 0 {
        return;
    }
    // `HELD_CODE` is not cleared here: the KeyUp clears it.
    //
    // [!] AQUI DECIA que lo que impedia disparar sesenta veces por segundo era
    // que `fb::rescue()` devuelve `None` en cuanto no queda dueno. **Eso dejo
    // de ser cierto el 01-09**, cuando la ventana de la segunda llamada se
    // movio DELANTE de esa comprobacion: desde entonces la rama de la purga se
    // alcanzaba sin pasar por ella. Un comentario que describe un muro que ya
    // no esta en el camino es peor que ninguno.
    //
    // Lo que lo impide hoy es el FLANCO: `SOLTADA` arriba.
    if rescue_owner() {
        unsafe { SWALLOW_ESC_RELEASE = true };
    }
}
