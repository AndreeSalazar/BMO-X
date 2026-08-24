//! **El raton sobre la caja de una APP.**
//!
//! Va DESPUES de las ventanas del sistema y antes de las fichas: una app en su
//! caja esta por delante de ellas, asi que su clic manda.
//!
//! ** DEVUELVE `true` CUANDO YA ESTA ATENDIDO, por lo mismo que `super::datos`:
//! los dos `return` de dentro cortaban la vuelta entera del puntero --uno
//! cuando ninguna caja esta agarrada y otro al soltar-- y ese significado se
//! pierde en cuanto el bloque deja de vivir dentro de la funcion grande.

use bmo_userland as bmo;

use super::Golpe;
use crate::desktop::{keys, Desktop, Ventana};
use crate::scene;
use crate::{erase_window, uncover};

pub(crate) fn on_pointer(dsk: &mut Desktop, p: &bmo::Pantalla, g: &Golpe) -> bool {
    let pos = g.pos;
    let button = g.button;

    // ** DONDE ESTA EL PUNTERO, antes que ningun clic y pase lo que pase.
    //
    // Se cuenta cada vuelta y a TODAS las cajas: a la de debajo su pixel, a las
    // demas que no. Es un ESTADO y no un evento --ver `Surface::puntero`-- asi
    // que se pisa en un sitio fijo y no se encola.
    dsk.table.puntero(p, pos.x, pos.y, pos.botones);

    // Y el SOLTAR, que es la otra cara del clic. Va aqui arriba y no en el
    // `match` de los botones del marco: al soltar no hay `button_at` que
    // consultar, el gesto ya empezo.
    //
    // [!] Sin CAPTURA: se entrega a quien esta debajo AHORA. Si el dedo salio
    // de la ventana antes de levantarse, ese soltar no llega a nadie.
    if !button && dsk.tick.button_before {
        keys::app::raton(dsk, p, pos.x, pos.y, pos.botones, false);
    }

    // -- ** EL RATON SOBRE UNA CAJA DE APP --
    //
    // Los mismos tres gestos que las ventanas del sistema, y por eso son
    // ocho lineas: el marco ya sabe hacerlos. **Este es el cobro del
    // `chrome.rs`** -- se escribio para que la cuarta ventana saliera
    // gratis, y la cuarta ventana resulta ser un programa entero.
    //
    // Va DESPUES de las ventanas del sistema y antes de las fichas: una
    // app en su caja esta por delante de ellas, asi que su clic manda.
    {
        use scene::chrome::Button;

        if button && !dsk.tick.button_before {
            if let Some(i) = dsk.table.at(pos.x, pos.y) {
                // El realce se pone aunque no se pulse: si no, los tres
                // botones de una app serian los unicos del escritorio
                // que no se encienden al pasar por encima.
                let gesture = dsk.table.get_mut(i).and_then(|s| s.chrome.button_at(pos.x, pos.y));
                match gesture {
                    // ** CERRAR RETIRA LA CAJA **Y** CIERRA EL PROCESO --
                    // paso 3 del plan, hecho el 2026-08-19.
                    //
                    // Y sigue sin ser `root`: el DIRECTOR no cierra "porque
                    // es el DIRECTOR", cierra porque **tiene el handle de
                    // haberlo lanzado**. `Hijo::por_tid` solo encuentra lo
                    // que `EJECUTAR` concedio; sobre una app que lanzo otro,
                    // no hay nada que encontrar y este boton no hace nada.
                    //
                    // ** El orden importa: primero la caja, despues el
                    // proceso. Al reves, `revoke_all` correria mientras esta
                    // vuelta todavia puede leer su superficie.
                    Some(Button::Close) => {
                        // El tid ANTES de soltar: `close` se lleva la
                        // superficie, y con ella la unica forma de saber de
                        // quien era esa ventana.
                        let tid = dsk.table.get_mut(i).map(|s| s.tid);
                        if let Some((vx, vy, va, vl)) = dsk.table.close(i) {
                            erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                            for s in dsk.table.iter_mut() {
                                s.repaint_all();
                            }
                        }
                        // Una app en ventana puede no tener entrada --hoy
                        // ninguna la tiene-- asi que pedirle que se vaya
                        // seria pedirselo a alguien que no escucha. Sin
                        // esto, cerrar dejaba un proceso dibujando para
                        // nadie hasta reiniciar.
                        if let Some(tid) = tid {
                            if let Some(h) = bmo::Hijo::por_tid(tid) {
                                h.cerrar();
                            }
                        }
                        // Y el foco deja de conocerla. Sin esto, Alt+Tab
                        // seguiria parando en una caja que ya no existe.
                        dsk.win.focus.close(Ventana::App(i as u8));
                    }
                    Some(Button::Minimize) => {
                        if let Some(s) = dsk.table.get_mut(i) {
                            let (vx, vy, va, vl) =
                                (s.chrome.x, s.chrome.y, s.chrome.width, s.chrome.height);
                            s.chrome.minimized = true;
                            erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                        }
                    }
                    // ** PANTALLA COMPLETA = QUE NO SE DIBUJE EL BORDE.
                    //
                    // Y aqui todavia no: maximizar da el hueco entero
                    // bajo la barra, que es lo que hacen las demas. Lo
                    // que NO pasa --ni pasara-- es entregarle el
                    // aparato: se sigue componiendo, asi que Alt+Tab
                    // sigue y `Ctrl+Alt+ESC` sigue. Un juego colgado se
                    // cierra con el teclado y no con el boton de reset.
                    Some(Button::Maximize) => {
                        if let Some(s) = dsk.table.get_mut(i) {
                            let (vx, vy, va, vl) = s.chrome.toggle_maximized(&p);
                            erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                            s.repaint_all();
                        }
                    }
                    None => {
                        if let Some(s) = dsk.table.get_mut(i) {
                            s.chrome.grab(pos.x, pos.y);
                        }
                        // ** EL CLIC LE DA EL FOCO, y el turno largo sale de
                        // ahi: lo aplica `turno_al_foco` una vez por vuelta,
                        // en un solo sitio. Pedirlo tambien aqui seria la
                        // misma regla en dos sitios -- y ademas Alt+Tab se
                        // quedaria fuera, porque por aqui no pasa.
                        dsk.win.focus.clic_en(Ventana::App(i as u8));
                        // Y el clic ENTRA, traducido a pixeles de la app. Ver
                        // `keys::app::raton`: contesta que no si el punto cae
                        // fuera del contenido, asi que la barra de titulo
                        // sigue siendo del marco.
                        keys::app::raton(dsk, &p, pos.x, pos.y, pos.botones, true);
                    }
                }
            }
        }

        // Arrastrar y estirar. El sitio VIEJO se borra antes de mover:
        // aqui no hay nadie que repinte lo de debajo, asi que sin esto
        // la ventana deja un rastro de copias de si misma.
        for i in 0..scene::surface::MAX {
            let Some(s) = dsk.table.get_mut(i) else { continue };
            if !s.chrome.grabbed() {
                return true;
            }
            if !button {
                s.chrome.release();
                return true;
            }
            let (vx, vy, va, vl) = (s.chrome.x, s.chrome.y, s.chrome.width, s.chrome.height);
            if s.chrome.follow_pointer(&p, pos.x, pos.y) {
                s.repaint_all();
                erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
            }
        }
    }

    false
}
