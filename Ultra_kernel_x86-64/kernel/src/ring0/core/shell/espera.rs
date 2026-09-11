//! **LA ESPERA DEL SHELL**: lo que hace el shell de Ring 0 mientras no llega
//! una tecla.
//!
//! [carril]  AMARILLO  un bucle que GIRA mientras el shell tiene el teclado;
//!                     cambiarle el ritmo cambia lo que gasta la maquina quieta
//! [consumo] LATE      con el teclado suyo GIRA: sondea serial, USB y PS/2 y
//!                     pinta CABINA en cada vuelta. Con la entrada cedida
//!                     duerme 4 ms por vuelta
//!
//! # Por que es un fichero y no un trozo de `ui.rs` (L6h)
//!
//! `ui.rs` contestaba dos preguntas que gastan distinto: *que hacer con una
//! tecla* --el editor de linea, que corre cuando llega una-- y *que hacer
//! mientras no llega* --que corre SIEMPRE--. La segunda es la que gasta con la
//! maquina quieta, y un fichero que late no esconde codigo que se pide.
//! `shell_read_line` sigue siendo el mismo bucle: antes de cada tecla llama a
//! [`siguiente_byte`], que hace exactamente lo que hacia la cabeza de su
//! vuelta -- movida, no reescrita.
//!
//! [!] Lo que late aqui, con su motivo: mientras el shell es el dueno del
//! teclado **gira**, porque no hay interrupcion del USB que lo despierte (W3
//! de `docs/plan/PLAN_VATIOS.md`) y dormir seria perder letras. Por eso
//! `consumo` tecleado en este shell mide una maquina que no descansa.

use super::super::dashboard::dash_log;
use super::ui::{clear_screen, dash_prompt};

/// Ultimo total de tareas visto por el shell, para detectar cuando un proceso
/// TERMINO (el total baja) y limpiar la pantalla automaticamente.
static mut LAST_TASK_TOTAL: usize = 0;

/// Cuanto duerme el shell cuando NO tiene nada que leer.
///
/// Cuatro milisegundos, el mismo numero que el latido del bus USB y por la
/// misma razon: es la distancia a la que un humano no nota la espera y la
/// maquina deja de girar en vacio.
const DESCANSO_MS: u64 = 4;

/// **El turno se devuelve cuando no hay nada que leer.**
///
/// == *** POR QUE EXISTE, y lo pidio el dueno (2026-09-08) =================
///
/// > *"me gustaria que el kernel no pierda tiempo chequeando si el guardian
/// > esta alli"*
///
/// Y tenia razon, con un numero detras. El pulso del escritorio salio en **50
/// vueltas por segundo** -- 20 ms por vuelta en un bucle que no tiene freno
/// ninguno. El escritorio no estaba lento: **estaba esperando turno**.
///
/// ** Y el turno se lo quedaba ESTE bucle. `shell_read_line` giraba a CPU
/// completa: recogia una decena de estadisticas, preguntaba por el serial, por
/// el USB, por el PS/2, y hacia `continue`. Sin `hlt`, sin ceder. Solo lo
/// echaba de ahi el reloj, al agotar su quantum de 4 ms.
///
/// *** Y lo peor es lo que giraba comprobando: cuando Ring 3 tiene la entrada,
/// este bucle **no puede leer el teclado ni aunque quiera** --lo dice tres
/// lineas mas arriba, `cedido es cedido`-- asi que gastaba su turno entero
/// preguntando por una tecla que tenia prohibido coger.
///
/// ```text
///    antes    gira 4 ms preguntando por un teclado que no es suyo
///    ahora    se duerme 4 ms y el turno se lo queda quien SI trabaja
/// ```
///
/// # El sacrificio (L3)
///
/// El cable del serial y la vuelta del teclado se ven con hasta 4 ms de
/// retraso. Es la misma apuesta que ya hace el hilo del bus con el teclado
/// USB, y por eso el numero es el mismo.
///
/// # Solo cuando la entrada es de Ring 3, y eso no es prudencia
///
/// Es la condicion que hace la afirmacion CIERTA. Mientras el shell es el dueno
/// del teclado, girar no es girar en vacio: cada vuelta puede traer una letra.
/// En cuanto la cede, no puede traer ninguna -- y solo entonces dormir es
/// gratis. Sin ese `if`, esto seria una espera puesta a ojo.
fn descansar() {
    use crate::ring0::task::scheduler;
    if !crate::ring0::obj::input::yielded() {
        return;
    }
    let hz = scheduler::tsc_freq();
    // ** SIN TSC NO SE DUERME. Un plazo que no se puede medir es un plazo
    // inventado, y quedarse Blocked con una hora falsa es no despertar. Se gira
    // como se giraba: peor, y vivo.
    if hz == 0 {
        return;
    }
    scheduler::park_until(scheduler::rdtsc() + (hz / 1_000) * DESCANSO_MS);
}

/// **El siguiente byte de entrada**, esperando lo que haga falta.
///
/// Cada vuelta es la cabeza de la vuelta de `shell_read_line`, tal cual: la
/// auto-limpieza, el prompt, CABINA, y preguntar al serial, al USB y al PS/2.
/// `linea` y `cursor` son lo que ya hay escrito, para pintar el prompt.
pub(crate) fn siguiente_byte(linea: &[u8], cursor: usize) -> u8 {
    use crate::ring0::dev::keyboard as kb;
    loop {
        // Auto-limpieza: si un proceso termino (el total de tareas bajo) y NO
        // estas escribiendo (linea vacia), limpia la pantalla -- como una
        // terminal que se refresca al acabar el programa. Nunca borra a media
        // escritura (solo con la linea vacia).
        let (total, _) = crate::ring0::task::scheduler::counts();
        unsafe {
            if linea.is_empty() && total < LAST_TASK_TOTAL {
                clear_screen();
                dash_log("== proceso terminado : pantalla limpia ==");
            }
            LAST_TASK_TOTAL = total;
        }
        dash_prompt(core::str::from_utf8(linea).unwrap_or(""), cursor);
        // CABINA -- cockpit omnisciente en la banda inferior.
        crate::ring0::cabina::render_hud();

        // Entrada: serial (COM1), teclado USB o PS/2, lo que tenga un byte.
        //
        // * El SERIAL nunca se cede. Es el cable del que depura, y sigue
        // hablando aunque Ring 3 sea dueno de la pantalla y del teclado -- que
        // es justo cuando mas falta hace.
        let mut byte = crate::ring0::dev::console::serial_read_byte();
        // * El teclado FISICO si. Si un proceso reclamo `KIND_INPUT`, las
        // teclas son suyas y este shell no las toca: los dos drenan la MISMA
        // cola, asi que leer aqui no seria "leer tambien", seria robarle letras
        // sueltas a la caja. Cedido es cedido, tambien para el que la cedio.
        if byte.is_none() && !crate::ring0::obj::input::yielded() {
            byte = crate::ring0::dev::usb::poll_ascii();
            if byte.is_none() {
                // PS/2 i8042 (mudo post-EBS en esta placa). Se conserva por si
                // algun dia reviviera (adaptador PS/2, otra placa).
                if let Some((_raw, ascii)) = kb::poll_event() {
                    byte = ascii;
                }
            }
        }
        if let Some(c) = byte {
            return c;
        }
        descansar();
    }
}
