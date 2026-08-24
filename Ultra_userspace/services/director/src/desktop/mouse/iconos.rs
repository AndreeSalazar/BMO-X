//! **El raton sobre la REJILLA DE ICONOS del escritorio.**
//!
//! Un clic senala, dos abren. La regla entera --y por que ese gesto se mide en
//! ciclos y no en vueltas del bucle-- vive en `scene::double_click`; aqui solo
//! esta quien la llama y que hace con la respuesta.

use bmo_userland as bmo;

use super::Golpe;
use crate::desktop::Desktop;
use crate::scene;

pub(crate) fn on_pointer(dsk: &mut Desktop, p: &bmo::Pantalla, g: &Golpe) {
    let pos = g.pos;
    let button = g.button;

    // -- ** UN CLIC EN UN ICONO: dar clic y ya --------------------
    //
    // Se rellena el campo con `run <path>` y se inyecta un Enter. No es
    // un atajo perezoso: es la afirmacion de que **pulsar un icono y
    // teclear su nombre son la misma cosa**, y por eso comparten camino
    // entero -- consola, prestamo de pantalla, eco y vigilante.
    //
    // Solo con la caja Ejecutar DELANTE, y esa condicion no es
    // cosmetica: si hay una ventana encima, el clic es de esa ventana.
    // Un escritorio que lanza programas a traves de lo que hay dibujado
    // encima es un escritorio en el que no se puede confiar al pulsar.
    if button
        && !dsk.tick.button_before
        && !dsk.calc.visible
        && !dsk.win.data_open
        && !dsk.win.cabina_open
        && !dsk.win.sound_open
    {
        // -- ** UN CLIC SENALA. DOS ABREN. Y ENTRAR tambien abre.
        //
        // Antes un solo clic LANZABA. Eso deja un escritorio en el que no se
        // puede mirar sin ejecutar: pulsar para ver como se llama un icono
        // arrancaba el programa, y de un lanzamiento no se vuelve solo.
        //
        // ** Lo bonito es que ENTRAR sale gratis. El primer clic deja escrito
        // `run <ruta>` en la caja de Ejecutar pero **no inyecta el salto de
        // linea**, asi que la orden queda preparada y es ENTRAR quien la
        // dispara -- que es exactamente lo que el dueno pidio, y sin una
        // segunda regla que mantener.
        if let Some(i) = dsk.launcher.app_at(&p, pos.x, pos.y) {
            let doble = dsk.launcher.clic(i);
            scene::launcher::repintar(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible);
            if let Some(app) = dsk.launcher.app(i) {
                let r = app.path();
                // `run ` + la ruta. Si no cupiera se deja como estaba:
                // media ruta lanzaria otra cosa, y eso es peor que no
                // lanzar nada.
                if 4 + r.len() <= dsk.field.path.len() {
                    dsk.field.path[..4].copy_from_slice(b"run ");
                    dsk.field.path[4..4 + r.len()].copy_from_slice(r);
                    dsk.field.n = 4 + r.len();
                    dsk.field.cur = dsk.field.n;
                    dsk.tick.repaint_field = true;
                    // Solo el SEGUNDO dispara.
                    if doble && dsk.field.ni < dsk.field.injected.len() {
                        dsk.field.injected[dsk.field.ni] = b'\n';
                        dsk.field.ni += 1;
                    }
                }
            }
        } else if dsk.launcher.soltar() {
            // Pulsar en el fondo quita el realce, como en cualquier escritorio.
            scene::launcher::repintar(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible);
        }
    }

}
