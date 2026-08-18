//! **Window management without letting go of the keyboard**: Alt+Tab, Alt+M,
//! Alt+arrows.
//!
//! These are served BEFORE anything asks about focus, and that is the point --
//! a shortcut that only works once you are already in the window you want is
//! not a shortcut.

use bmo_userland as bmo;

use super::Key;
use crate::desktop::{Desktop, Ventana};
use crate::scene::{self, paint_status, ACCENT};
use crate::{erase_window, uncover};

pub(crate) fn on_key(
    dsk: &mut Desktop,
    p: &bmo::Pantalla,
    c: u8,
    alt_alone: bool,
    m: u8,
) -> Key {
if alt_alone && c == 0x09 {
    if m & bmo::MOD_SHIFT != 0 {
        dsk.win.focus.conmutar_atras();
    } else {
        dsk.win.focus.conmutar();
    }
    scene::switcher::paint(
        &p,
        dsk.win.focus.lista(),
        dsk.win.focus.pointed_index(),
        dsk.win.focus.modo().name(),
    );
    dsk.win.switcher_painted = true;
    return Key::Taken;
}
// -- Alt+M: cambiar el MODO del foco --
//
// Sin una tecla, los tres modos son decoracion: `Fijo` y
// `Puntero` existirian sin forma de llegar a ellos. Va con Alt
// por lo mismo que el Tab --`Alt` solo no produce caracter en
// ninguna distribucion, `Ctrl+Alt` SI (es AltGr)-- y se anuncia
// en la propia ventanita, que es donde se lee el modo.
if alt_alone && (c == b'm' || c == b'M') {
    dsk.win.focus.poner_modo(dsk.win.focus.modo().next());
    if dsk.win.switcher_painted {
        scene::switcher::paint(
            &p,
            dsk.win.focus.lista(),
            dsk.win.focus.pointed_index(),
            dsk.win.focus.modo().name(),
        );
    } else if dsk.win.visible {
        // Cambiarlo sin el conmutador abierto tambien tiene que
        // verse: un modo que cambia en silencio se descubre
        // cuando el teclado ya se fue a otra ventana.
        paint_status(&p, &dsk.run_box, dsk.win.focus.modo().nombre_largo(), ACCENT);
    }
    return Key::Taken;
}
// -- ** ALT+FLECHAS: MOVER Y ENCAJAR SIN SOLTAR EL TECLADO --
//
// Alt+Tab ya elegia ventana y no podia hacer nada con ella. Esto
// cierra el gesto: se elige con Tab y se coloca con las flechas,
// sin que la mano salga del teclado.
//
// * **A secas mueve; con Shift encaja** -- media pantalla a los
// lados, el panel entero arriba, y abajo deshace el maximizado.
// Es lo que hace Windows con la tecla de la ventanita, y se
// copia el reparto a proposito: un atajo de colocar ventanas que
// no es el que ya tienes en los dedos se usa una vez.
//
// Va con `Alt` por lo mismo que el Tab y la M, y esta escrito
// dos lineas mas arriba: `Alt` solo no produce caracter en
// ninguna distribucion y `Ctrl+Alt` SI, porque es AltGr.
//
// [!] Se atiende ANTES que las flechas de las ventanas, y por eso
// no les quita nada: sin `Alt` esto no entra, y las flechas de
// Datos y el volumen de Sonido siguen llegando enteras.
if alt_alone && (0x80..=0x83).contains(&c) {
    use scene::chrome::Heading;
    let heading = match c {
        0x80 => Heading::Up,
        0x81 => Heading::Down,
        0x82 => Heading::Left,
        _ => Heading::Right,
    };
    let fit = m & bmo::MOD_SHIFT != 0;
    let mut moved = false;
    // -- ** SE MUEVE LA SENALADA, NO LA QUE TIENE EL FOCO --
    //
    // `focus.actual()` parece lo obvio y es justo lo que no vale:
    // **no cambia mientras conmutas**, a proposito --lo dice su
    // propia documentacion-- porque una letra escrita a mitad de
    // un Alt+Tab no puede caer en una ventana que todavia no has
    // elegido.
    //
    // Pero estas flechas se pulsan CON EL ALT PULSADO, que es
    // exactamente "a mitad de un Alt+Tab". Con `actual()`, elegir
    // CABINA con Tab y darle a la flecha moveria la ventana
    // ANTERIOR -- se veria moverse la que no es, que es peor que
    // no moverse nada.
    //
    // `pointed_at()` contesta las dos situaciones con una regla:
    // conmutando es la resaltada, y sin conmutar es la que ya
    // tiene el foco. La que se mueve es **la que estas mirando en
    // la ventanita**, y eso se puede explicar en una frase.
    match dsk.win.focus.pointed_at() {
        Some(Ventana::Data) => {
            if dsk.win.data_open && !dsk.win.data.chrome.minimized {
                let (vx, vy, va, vl) = (
                    dsk.win.data.x(), dsk.win.data.y(),
                    dsk.win.data.width(), dsk.win.data.height(),
                );
                let cambio = if fit {
                    dsk.win.data.chrome.snap(&p, heading)
                } else {
                    dsk.win.data.chrome.push(&p, heading)
                };
                if cambio {
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    // Encajar CAMBIA el tamano, asi que las cajas del
                    // grafo hay que recolocarlas: sin esto la ventana
                    // mide una cosa y su contenido sigue midiendo otra.
                    dsk.win.data.relayout();
                    scene::data::paint(&p, &dsk.win.data);
                    dsk.win.top_before = Ventana::Data;
                    moved = true;
                }
            }
        }
        Some(Ventana::Cabina) => {
            if dsk.win.cabina_open && !dsk.win.cabina.chrome.minimized {
                let (vx, vy, va, vl) = (
                    dsk.win.cabina.chrome.x, dsk.win.cabina.chrome.y,
                    dsk.win.cabina.chrome.width, dsk.win.cabina.chrome.height,
                );
                let cambio = if fit {
                    dsk.win.cabina.chrome.snap(&p, heading)
                } else {
                    dsk.win.cabina.chrome.push(&p, heading)
                };
                if cambio {
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    scene::cabina::paint(&p, &dsk.win.cabina);
                    dsk.win.top_before = Ventana::Cabina;
                    moved = true;
                }
            }
        }
        Some(Ventana::Sound) => {
            if dsk.win.sound_open && !dsk.win.sound.chrome.minimized {
                let (vx, vy, va, vl) = (
                    dsk.win.sound.chrome.x, dsk.win.sound.chrome.y,
                    dsk.win.sound.chrome.width, dsk.win.sound.chrome.height,
                );
                let cambio = if fit {
                    dsk.win.sound.chrome.snap(&p, heading)
                } else {
                    dsk.win.sound.chrome.push(&p, heading)
                };
                if cambio {
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    scene::sound::paint(
                        &p, &dsk.win.sound, dsk.snd.cap.is_some(),
                        dsk.snd.devices, dsk.snd.volume, dsk.snd.pressed,
                    );
                    dsk.win.top_before = Ventana::Sound;
                    moved = true;
                }
            }
        }
        // ** LA TERMINAL, que hasta el 2026-08-16 no se movia.
        //
        // Aqui decia *"Ejecutar no se mueve: es el escritorio, no
        // una ventana"*. Era la descripcion de una limitacion
        // escrita como si fuera un principio -- y ni siquiera era
        // cierta: tenia barra de titulo, sombra y esquinas
        // redondeadas como las demas, solo que no se podia agarrar.
        // El dueno lo dijo mirandola: *"me gustaria que sea
        // movible"*.
        // ** ESTA RAMA DECIA `Some(W_RUN)` Y NO ERA ESTA RAMA.
        //
        // `W_RUN` era una constante que este fichero **no importaba**, y un
        // nombre desconocido en un patron de Rust no es una constante: es una
        // VARIABLE nueva que casa con todo. Asi que con las vitales senaladas
        // --o con cualquier ventana cuya guarda fallara-- la flecha movia la
        // TERMINAL, y la linea de abajo guardaba en `top_before` el id que
        // hubiera casado en vez del de Ejecutar.
        //
        // El compilador lo estuvo diciendo todo el tiempo, en un aviso que no
        // parece lo que es: `variable W_RUN should have a snake case name`.
        //
        // ** Y CON `Ventana` ESO NO SE PUEDE ESCRIBIR. Un patron con `::`
        // nunca es un enlace: si el tipo no esta importado no compila, en vez
        // de tragarse todos los casos en silencio. La clase entera de fallo se
        // fue con las constantes sueltas.
        Some(Ventana::Run) => {
            if dsk.win.visible {
                let (vx, vy, va, vl) = (
                    dsk.run_box.x, dsk.run_box.y,
                    dsk.run_box.w(), dsk.run_box.h(),
                );
                let cambio = if fit {
                    dsk.run_box.chrome.snap(&p, heading)
                } else {
                    dsk.run_box.chrome.push(&p, heading)
                };
                if cambio {
                    // El orden importa y es distinto del de las otras
                    // tres: ahi `uncover` repinta la terminal, que no
                    // se habia movido. Aqui la que se movio ES la
                    // terminal, asi que primero se recolocan sus
                    // medidas y solo despues se borra y se repinta --
                    // al reves, `erase_window` preguntaria por el
                    // color de fondo con la geometria vieja y dejaria
                    // el rastro que este mismo fichero ya cazo tres
                    // veces.
                    dsk.run_relayout();
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    dsk.win.top_before = Ventana::Run;
                    moved = true;
                }
            }
        }
        // [!] LAS VITALES NO SE MUEVEN CON EL TECLADO, y aqui lo pone.
        //
        // Es el mismo hueco que en el raton: F7 y F8 tienen marco, titulo y
        // un pie que anuncia "arrastra el titulo", y no las mueve nada. Antes
        // caian en un `_ => {}` donde no se distinguian de "no hay foco"; con
        // el `match` sin comodin son un caso con nombre, y el dia que se
        // arreglen no hay que buscar donde.
        Some(Ventana::Cpu) | Some(Ventana::Mem) => {}
        // Sin foco no hay a quien mover. La tecla se come igual: dejarla
        // pasar mandaria un Alt+flecha a la linea de comandos.
        None => {}
    }
    // La ventana se acaba de pintar ENCIMA del conmutador, que
    // esta en el centro. Sin esto, mover tapa la ventanita que
    // dice cual estas moviendo -- y a la segunda flecha ya no
    // sabes en cual estas. Al soltar Alt se repinta todo de abajo
    // arriba, asi que el destrozo se repara solo; lo que hay que
    // arreglar es lo que se ve MIENTRAS.
    if moved && dsk.win.switcher_painted {
        scene::switcher::paint(
            &p,
            dsk.win.focus.lista(),
            dsk.win.focus.pointed_index(),
            dsk.win.focus.modo().name(),
        );
    }
    return Key::Taken;
}
    Key::Pass
}
