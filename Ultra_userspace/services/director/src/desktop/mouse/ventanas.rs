//! **El raton sobre las tres ventanas con marco que no son una app**: CABINA,
//! la TERMINAL y la de SONIDO.
//!
//! Van juntas porque comparten la unica cosa que las hace ventanas --el marco
//! de `scene::chrome`: arrastrar, estirar y los tres botones-- y se diferencian
//! solo en que hay dentro. Separarlas en tres ficheros seria escribir el mismo
//! arrastre tres veces.

use bmo_userland as bmo;

use super::Golpe;
use crate::desktop::{Desktop, Ventana};
use crate::scene;
use crate::uncover;

pub(crate) fn on_pointer(dsk: &mut Desktop, p: &bmo::Pantalla, g: &Golpe) {
    let pos = g.pos;
    let button = g.button;

    // -- ** EL ARRASTRE DE LAS OTRAS DOS VENTANAS --
    //
    // CABINA y Sonido nacieron con `Chrome` --que trae `grab`,
    // `follow_pointer` y `release`-- y **nadie las llamaba**. El
    // resultado en el Ryzen: dos ventanas con barra de titulo, con sus
    // tres botones pintados, y clavadas en el sitio.
    //
    // Es el patron 24 de `BITACORA.md` calcado: la politica escrita y
    // sin lector. Alli fue `es_para` del foco, que existia entera con
    // tests y no se llamaba ni una vez; aqui es el arrastre. Dar el
    // mecanismo NO es cablearlo, y la unica forma de notar la
    // diferencia es ejecutandolo -- por eso salio en metal y no antes.
    //
    // * Y el cableado nacio ANIDADO dentro del `if data_open`, que
    // es el mismo patron una vuelta mas: el lector existia y solo se le
    // daba corriente cuando ESTRATOS estaba abierta y sin minimizar.
    // Con Datos cerrada, las dos ventanas volvian a estar clavadas. El
    // arrastre de una ventana no depende de otra ventana, asi que va
    // aqui, al nivel de las demas.
    if dsk.win.cabina_open && !dsk.win.cabina.chrome.minimized {
        if button && !dsk.win.cabina.chrome.grabbed() && dsk.win.focus.es_para(Ventana::Cabina)
            && dsk.win.cabina.chrome.on_the_grip(pos.x, pos.y)
        {
            dsk.win.cabina.chrome.grab(pos.x, pos.y);
        } else if !button && dsk.win.cabina.chrome.grabbed() {
            dsk.win.cabina.chrome.release();
        } else if button && dsk.win.cabina.chrome.grabbed() {
            // El sitio VIEJO se borra antes de mover: aqui no hay
            // compositor que repinte lo de debajo, asi que sin esto
            // la ventana deja un rastro de copias de si misma.
            let (vx, vy, va, vl) = (
                dsk.win.cabina.chrome.x, dsk.win.cabina.chrome.y,
                dsk.win.cabina.chrome.width, dsk.win.cabina.chrome.height,
            );
            if dsk.win.cabina.chrome.follow_pointer(&p, pos.x, pos.y) {
                scene::erase_moved(
                    &p,
                    &dsk.run_box,
                    (vx, vy, va, vl),
                    (
                        dsk.win.cabina.chrome.x,
                        dsk.win.cabina.chrome.y,
                        dsk.win.cabina.chrome.width,
                        dsk.win.cabina.chrome.height,
                    ),
                    dsk.win.visible,
                );
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                scene::cabina::paint(&p, &dsk.win.cabina);
                dsk.win.top_before = Ventana::Cabina;
            }
        }
    }

    // -- ** EL RATON SOBRE LA TERMINAL --
    //
    // Va ANTES que las demas ventanas a proposito: la terminal esta
    // DEBAJO de todas, asi que si CABINA o Datos estan encima y el
    // puntero cae en la zona compartida, la de arriba tiene que ganar
    // -- y gana porque su bloque se evalua despues y el `grab` de esta
    // no se dispara si ya hay otro agarrado por el suyo.
    //
    // ** Y este bloque existe porque el marco NO se cablea solo. Este
    // mismo fichero ya cazo el fallo una vez, con estas palabras:
    // *"CABINA y Sonido nacieron con `Chrome` y nadie las llamaba. El
    // resultado en el Ryzen: dos ventanas con barra de titulo, con sus
    // tres botones pintados, y clavadas en el sitio."* Pintar los
    // botones no es tenerlos.
    if dsk.win.visible {
        use scene::chrome::Button;

        let hover_now = dsk.run_box.chrome.button_at(pos.x, pos.y);
        if hover_now != dsk.run_box.chrome.hover {
            dsk.run_box.chrome.hover = hover_now;
            dsk.run_box.chrome.paint_buttons(&p, scene::BOX_TITLE);
        }

        if button && !dsk.tick.button_before {
            match dsk.run_box.chrome.button_at(pos.x, pos.y) {
                // No hay aspa: `sin_cerrar()`. Ver la cabecera de
                // `Chrome::closable` -- cerrar la unica ventana donde se
                // escriben ordenes dejaria la maquina sin linea de
                // ordenes, y al shell de Ring 0 no se vuelve.
                Some(Button::Close) => {}
                // Minimizar es **lo que ya hacia Ctrl+Alt**: esconderla
                // entera. No se inventa un segundo estado escondido
                // --`chrome.minimized` ademas de `win.visible`-- porque
                // dos banderas para una sola pregunta acaban diciendo
                // cosas distintas. Vuelve por donde ya volvia.
                Some(Button::Minimize) => {
                    scene::erase_box(&p, &dsk.run_box);
                    dsk.win.visible = false;
                    dsk.win.taskbar_dirty = true;
                }
                Some(Button::Maximize) => {
                    let (vx, vy, va, vl) = dsk.run_box.chrome.toggle_maximized(&p);
                    dsk.run_relayout(&p);
                    scene::erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    dsk.win.top_before = Ventana::Run;
                }
                None => {}
            }
        }

        if button && !dsk.run_box.chrome.grabbed()
            && (dsk.run_box.chrome.on_the_grip(pos.x, pos.y)
                || dsk.run_box.chrome.on_the_corner(pos.x, pos.y))
        {
            dsk.run_box.chrome.grab(pos.x, pos.y);
        } else if !button && dsk.run_box.chrome.grabbed() {
            dsk.run_box.chrome.release();
        } else if button && dsk.run_box.chrome.grabbed() {
            let (vx, vy, va, vl) = (
                dsk.run_box.x, dsk.run_box.y,
                dsk.run_box.w(), dsk.run_box.h(),
            );
            if dsk.run_box.chrome.follow_pointer(&p, pos.x, pos.y) {
                // Primero recolocar, luego borrar el sitio viejo y
                // repintar: la que se movio es ESTA, asi que
                // `erase_window` tiene que preguntar por el color de
                // fondo con la geometria NUEVA o deja un rastro de
                // copias de si misma.
                dsk.run_relayout(&p);
                // ** Se borra la RESTA, no el rectangulo entero.
                //
                // Arrastrar movia la ventana unos pocos pixeles y borraba sus
                // ~325.000 pixeles viejos --4,33 ms, la cuarta parte de un
                // fotograma-- para descubrir una tira estrecha. El resto lo
                // volvia a tapar ella misma un instante despues.
                scene::erase_moved(
                    &p,
                    &dsk.run_box,
                    (vx, vy, va, vl),
                    (dsk.run_box.x, dsk.run_box.y, dsk.run_box.w(), dsk.run_box.h()),
                    dsk.win.visible,
                );
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                dsk.win.top_before = Ventana::Run;
            }
        }
    }

    if dsk.win.sound_open && !dsk.win.sound.chrome.minimized {
        if button && !dsk.win.sound.chrome.grabbed() && dsk.win.focus.es_para(Ventana::Sound)
            && dsk.win.sound.chrome.on_the_grip(pos.x, pos.y)
        {
            dsk.win.sound.chrome.grab(pos.x, pos.y);
        } else if !button && dsk.win.sound.chrome.grabbed() {
            dsk.win.sound.chrome.release();
        } else if button && dsk.win.sound.chrome.grabbed() {
            let (vx, vy, va, vl) = (
                dsk.win.sound.chrome.x, dsk.win.sound.chrome.y,
                dsk.win.sound.chrome.width, dsk.win.sound.chrome.height,
            );
            if dsk.win.sound.chrome.follow_pointer(&p, pos.x, pos.y) {
                scene::erase_moved(
                    &p,
                    &dsk.run_box,
                    (vx, vy, va, vl),
                    (
                        dsk.win.sound.chrome.x,
                        dsk.win.sound.chrome.y,
                        dsk.win.sound.chrome.width,
                        dsk.win.sound.chrome.height,
                    ),
                    dsk.win.visible,
                );
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                scene::sound::paint(
                    &p, &dsk.win.sound, dsk.snd.cap.is_some(),
                    dsk.snd.devices, dsk.snd.volume, dsk.snd.pressed,
                );
                dsk.win.top_before = Ventana::Sound;
            }
        }
    }

}
