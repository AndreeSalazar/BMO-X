//! **The six window toggles**: F1 ESTRUCTURA, F7 cpu, F8 memory, F10 sound,
//! F11 CABINA, F12 data -- and the ESC that closes each one.
//!
//! [consumo] NADA      no corre en reposo: lo llama el bucle SOLO si hubo una
//!                     tecla o el raton se movio. Sin entrada, no se entra
//!                     aqui (L6h)
//!
//! F1 is first in the list and last in the file, and both are on purpose: it is
//! the workshop, which is where you start, while F7..F12 are instruments you go
//! to. In the file it sits after F12 because that block is the one it was
//! copied from, and putting it next to its original is what makes a later
//! divergence between the two visible.
//!
//! Function keys produce no character in ANY layout, so they cannot collide
//! with typing. That is the only thing that matters in a system shortcut, and
//! it is exactly what `Ctrl+Alt` cannot offer: in Spanish it IS AltGr.

use bmo_userland as bmo;

use super::Key;
use crate::desktop::{Desktop, Ventana};
use crate::scene::{self};
use crate::{erase_window, uncover};

pub(crate) fn on_key(dsk: &mut Desktop, p: &bmo::Pantalla, c: u8, alt_alone: bool) -> Key {
// == ALT+F4: CERRAR LO DE DELANTE ====================================
//
// Lo pidio el dueno con estas palabras: *"agregar esa ventanita para cerrar
// y todo eso como tipico Alt+F4, que es para cerrar cualquier app"*.
//
// Va ANTES que nada por lo mismo que F12: un atajo que solo funciona si ya
// estas dentro de la ventana no sirve para cerrarla.
//
// ** Y usa el MISMO camino que la X del marco --`cerrar_app`-- porque cerrar
// tiene que significar lo mismo se pida como se pida. Dos cierres distintos
// para el mismo gesto es como se llega a que uno mate el proceso y el otro no.
//
// [!] LO QUE NO ALCANZA, dicho por delante: una app a PANTALLA COMPLETA. Con
// DOOM delante el escritorio esta dormido en `lend_screen` y no lee teclas --
// no hay a quien mandarle este atajo. Para esas sigue siendo `Ctrl+Alt+Esc`,
// que vive en Ring 0 justamente porque es el unico sitio por donde pasan
// TODAS las teclas. Un atajo de escritorio no puede rescatar de algo que se
// llevo el escritorio.
// 0x8C = F4, con el mismo criterio que el 0x94 de F12 mas abajo: el codigo
// crudo, porque asi esta escrito el resto de este fichero.
//
// *** CIERRA LO QUE HAYA DELANTE, SEA LO QUE SEA (corregido el 05-09).
//
// La primera version solo cerraba `Ventana::App`, y el dueno lo probo y dijo
// *"el Alt+F4 no me funciona"*. Tenia razon, y el fallo era de diseno: el
// atajo se llamaba *cerrar cualquier app* y solo cerraba una CLASE de ventana.
// Con Ejecutar delante --que es lo que hay en cuanto no tienes una app
// abierta-- no hacia nada Y NO DECIA NADA, que es la peor de las dos mitades.
//
// ** Un atajo que a veces no hace nada y nunca lo explica se lee como roto,
// aunque este haciendo exactamente lo que se le escribio. Es la misma familia
// que lleva toda la semana: algo correcto que se entiende como otra cosa.
// == *** Y CTRL+C HACE LO MISMO (2026-09-12) ==========================
//
// Lo pidio el dueno con el problema delante: *"control + C es que eso me
// permite restaurar mi terminal y cerrar a la fuerza el app arrancado, no me
// dejo poner save por eso"*.
//
// Y el problema es real y no es de comodidad: **mientras una app tiene el
// foco, las teclas son suyas** --es el orden de `keys::app`-- asi que la caja
// de Ejecutar se queda muda y no hay donde escribir `guarda`. Alt+F4 ya
// resolvia eso desde el 05-09, pero `Ctrl+C` es el gesto que el dueno ya tiene
// en los dedos de treinta anios de terminal, y un atajo que hay que recordar
// es un atajo que no se usa.
//
// ** Mismo camino, no uno nuevo: `cerrar_app`. Tres gestos --la X, Alt+F4 y
// esto-- y UN cierre. Dos cierres distintos para el mismo gesto es como se
// llega a que uno mate el proceso y el otro no.
//
// ** Y con EJECUTAR delante no dice una frase: LIMPIA LA LINEA. Es lo que
// hace `Ctrl+C` en cualquier terminal del mundo, y es ademas lo que el dueno
// estaba intentando conseguir -- recuperar la caja para escribir otra cosa.
//
// [!] Lo que NO alcanza es lo mismo que no alcanza Alt+F4, y por el mismo
// motivo: una app que se llevo la PANTALLA deja al escritorio dormido en
// `lend_screen`, sin leer teclas. Para esas sigue siendo `Ctrl+Alt+Esc`, que
// vive en Ring 0 porque es el unico sitio por donde pasan TODAS.
//
// [!] Y llega aqui de verdad: el kernel cuece `Ctrl+C` como el byte 0x03, la
// cola cruda no reenvia teclas con modificador (`del_escritorio`) y la cocida
// tampoco reenvia los codigos de control (`keys::app::caracter`). O sea que
// **ninguna app lo ve**, y eso es a proposito: un rescate que la app pudiera
// interceptar no seria un rescate.
let ctrl_c = c == 0x03;

// *** Y LO PRIMERO QUE MIRA CTRL+C ES LA CORRIDA EN VUELO. (2026-09-12)
//
// El dueno lo corrigio y tenia razon: *"el control + C es para frenar en
// comando como terminal de Windows"*. Un terminal no interrumpe *la ventana de
// delante*: interrumpe **el comando que tu lanzaste**, tenga ventana o no.
//
// La diferencia no es de matiz y se ve en el caso que importa: un programa de
// CONSOLA que se cuelga --`leer.bex`, uno de COBOL-- no tiene ventana, asi que
// el foco sigue en Ejecutar. Mirando el foco, Ctrl+C le habria limpiado la
// linea y **habria dejado el programa colgado**, que es exactamente lo que el
// dueno estaba sufriendo.
//
// ** `Out::run` ya existia y ya sabia que hay una corrida esperando final: lo
// unico que le faltaba era a QUIEN. `ejecutar_en` devolvia el tid desde
// siempre y este sitio lo tiraba.
//
// Y despues de frenarlo no hay que hacer nada mas: el vigilante de `watch.rs`
// ve que ya no hay hijo, DRENA lo que dejo dicho y lo guarda en su `.txt`. O
// sea que un programa frenado deja su volcado igual que uno que acaba solo --
// que es lo que uno espera de un terminal.
if ctrl_c {
    if let Some(r) = dsk.out.run.as_ref() {
        if let Some(h) = bmo::Hijo::por_tid(r.tid) {
            if h.vive() {
                h.cerrar();
                dsk.out.grid.text(b"  ^C  frenado
");
                dsk.tick.repaint_field = true;
                return Key::Taken;
            }
        }
    }
}

if (c == 0x8C && alt_alone) || ctrl_c {
    match dsk.win.focus.actual() {
        // Una app: se cierra de verdad, por el MISMO camino que la X.
        Some(Ventana::App(i)) => {
            crate::desktop::mouse::apps::cerrar_app(dsk, p, i as usize);
            return Key::Taken;
        }
        // Los paneles del propio escritorio se esconden, que es lo que
        // significa cerrar para algo que no es un proceso.
        Some(Ventana::Data) => {
            dsk.win.data_open = false;
            dsk.win.focus.close(Ventana::Data);
            erase_window(p, &dsk.run_box, dsk.win.data.x(), dsk.win.data.y(),
                         dsk.win.data.width(), dsk.win.data.height(), dsk.win.visible);
            dsk.win.top_before = Ventana::Run;
            uncover(p, &dsk.run_box, &dsk.launcher, dsk.win.visible,
                    &mut dsk.out.grid, &mut dsk.tick.repaint_field);
            return Key::Taken;
        }
        // [!] Y aqui NO se cierra nada, pero SE DICE. Ejecutar es la casa: si
        // Alt+F4 la cerrara no quedaria donde escribir. Callar seria dejar al
        // dueno pensando que el atajo esta roto -- que es exactamente lo que
        // paso el 05-09.
        _ => {
            if ctrl_c {
                // La caja delante y `Ctrl+C`: se limpia lo tecleado, como en
                // cualquier terminal. No hay nada que cerrar y SI hay algo que
                // devolver -- una linea en blanco donde escribir.
                dsk.field.n = 0;
                dsk.field.cur = 0;
                dsk.out.grid.text(b"  linea limpia
");
            } else {
                dsk.out.grid.text(b"  Alt+F4 cierra la ventana de delante. Esta es la casa.
");
            }
            dsk.tick.repaint_field = true;
            return Key::Taken;
        }
    }
}

// -- F12 es del SISTEMA, no de una ventana --
//
// Se atiende ANTES de preguntar por el foco, y tiene que ser
// asi: un atajo que solo funciona si ya estas en la ventana que
// abre no sirve para abrirla -- y peor, no sirve para cerrarla,
// porque para entonces el foco ya es suyo.
//
// ESC cierra la de arriba, que es lo que hace ESC en todas
// partes. En Ejecutar ESC sigue borrando la linea: son dos
// ventanas distintas y cada una contesta lo suyo.
let toggle_data = if c == 0x94 {
    Some(!dsk.win.data_open)
} else if c == 0x1B && dsk.win.data_open && dsk.win.focus.es_para(Ventana::Data) {
    Some(false)
} else {
    None
};
if let Some(open) = toggle_data {
    dsk.win.data_open = open;
    if open {
        // Abrir es decirselo al foco y ya: en modo `Fijo` la
        // ventana aparece y NO se lleva el teclado, y quien
        // decide eso es la politica, no esta tecla.
        dsk.win.focus.open(Ventana::Data);
        scene::data::paint(&p, &dsk.win.data);
        dsk.win.top_before = if dsk.win.focus.es_para(Ventana::Data) { Ventana::Data } else { Ventana::Run };
        // En `Fijo` se ha pintado encima de una caja que sigue
        // teniendo el teclado: hay que devolverla arriba.
        if dsk.win.top_before == Ventana::Run {
            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        }
    } else {
        // Al cerrarla hay que devolver el fondo Y repintar
        // lo que tapaba: la caja de Ejecutar esta debajo.
        dsk.win.focus.close(Ventana::Data);
        erase_window(
            &p, &dsk.run_box, dsk.win.data.x(), dsk.win.data.y(),
            dsk.win.data.width(), dsk.win.data.height(), dsk.win.visible,
        );
        dsk.win.top_before = Ventana::Run;
        uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
    }
    return Key::Taken;
}

// -- F1: ESTRUCTURA, el taller --
//
// Calcada de F12 y por los mismos motivos: se atiende ANTES de
// preguntar por el foco, porque un atajo que solo funciona si ya
// estas dentro de la ventana no sirve para abrirla, y peor, no
// sirve para cerrarla.
//
// ** F1 ESTABA LIBRE, y no de casualidad: `keys/app.rs` declaraba
// `SC_F1 = 0x3B` solo como frontera del rango que el escritorio
// retiene (`SC_F1..=SC_F10`), sin que nadie la usara. O sea que la
// tecla ya llegaba aqui y no habia quien la recogiera.
//
// El 0x89 es el codigo COCIDO, no el scancode: lo pone
// `ring0/dev/keyboard.rs` (`KEY_F1`), y las doce F son 0x89..0x94.
// Escribir aqui el 0x3B compilaria y no abriria nada -- son dos
// colas distintas, y esta es la cocida.
//
// Escalon 1 de `docs/plan/PLAN_ESTRUCTURA.md`: la ventana abre y
// dice en que escalon esta. Todavia no lee una tecla.
let toggle_est = if c == 0x89 {
    Some(!dsk.win.estructura_open)
} else if c == 0x1B && dsk.win.estructura_open && dsk.win.focus.es_para(Ventana::Estructura) {
    Some(false)
} else {
    None
};
if let Some(open) = toggle_est {
    dsk.win.estructura_open = open;
    if open {
        dsk.win.focus.open(Ventana::Estructura);
        scene::estructura::paint(&p, &dsk.win.estructura);
        dsk.win.top_before = if dsk.win.focus.es_para(Ventana::Estructura) {
            Ventana::Estructura
        } else {
            Ventana::Run
        };
        // En `Fijo` se ha pintado encima de una caja que sigue
        // teniendo el teclado: hay que devolverla arriba.
        if dsk.win.top_before == Ventana::Run {
            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        }
    } else {
        // Al cerrarla, devolver el fondo Y repintar lo que tapaba.
        dsk.win.focus.close(Ventana::Estructura);
        let ch = &dsk.win.estructura.chrome;
        erase_window(
            &p, &dsk.run_box, ch.x, ch.y, ch.width, ch.height, dsk.win.visible,
        );
        dsk.win.top_before = Ventana::Run;
        uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
    }
    return Key::Taken;
}

// -- F7 y F8: las vitales --
//
// Calcadas de F11 y por los mismos motivos: se atienden ANTES
// de preguntar por el foco, porque un atajo que solo funciona
// si ya estas dentro de la ventana no sirve para abrirla.
//
// ESC cierra la que este abierta. Si las dos lo estan, cierra
// primero la de memoria -- que es la que se abre encima.
let toggle_cpu = if c == 0x8F {
    Some(!dsk.win.cpu_open)
} else if c == 0x1B && dsk.win.cpu_open && !dsk.win.mem_open {
    Some(false)
} else {
    None
};
if let Some(open) = toggle_cpu {
    dsk.win.cpu_open = open;
    if open {
        dsk.win.focus.open(Ventana::Cpu);
        scene::vitals::paint(&p, &dsk.win.cpu, dsk.tick.loops_per_second);
    } else {
        dsk.win.focus.close(Ventana::Cpu);
        erase_window(
            &p, &dsk.run_box, dsk.win.cpu.chrome.x, dsk.win.cpu.chrome.y,
            dsk.win.cpu.chrome.width, dsk.win.cpu.chrome.height, dsk.win.visible,
        );
        uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
    }
    return Key::Taken;
}
let toggle_mem = if c == 0x90 {
    Some(!dsk.win.mem_open)
} else if c == 0x1B && dsk.win.mem_open {
    Some(false)
} else {
    None
};
if let Some(open) = toggle_mem {
    dsk.win.mem_open = open;
    if open {
        dsk.win.focus.open(Ventana::Mem);
        scene::vitals::paint(&p, &dsk.win.mem, dsk.tick.loops_per_second);
    } else {
        dsk.win.focus.close(Ventana::Mem);
        erase_window(
            &p, &dsk.run_box, dsk.win.mem.chrome.x, dsk.win.mem.chrome.y,
            dsk.win.mem.chrome.width, dsk.win.mem.chrome.height, dsk.win.visible,
        );
        uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
    }
    return Key::Taken;
}

// -- F11: la consola del KERNEL --
//
// Calcada de F12 y por los mismos motivos: se atiende ANTES de
// preguntar por el foco, porque un atajo que solo funciona si ya
// estas dentro de la ventana no sirve para abrirla.
let toggle_klog = if c == 0x93 {
    Some(!dsk.win.cabina_open)
} else if c == 0x1B && dsk.win.cabina_open {
    Some(false)
} else {
    None
};
if let Some(open) = toggle_klog {
    dsk.win.cabina_open = open;
    if open {
        // Se abre SIEMPRE por lo ultimo, que es lo que se quiere
        // ver el 90% de las veces. Para ir al arranque estan
        // RePag/AvPag.
        dsk.win.cabina.from = 0;
        dsk.win.focus.open(Ventana::Cabina);
        scene::cabina::paint(&p, &dsk.win.cabina);
        dsk.win.top_before = if dsk.win.focus.es_para(Ventana::Cabina) { Ventana::Cabina } else { Ventana::Run };
        if dsk.win.top_before == Ventana::Run {
            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        }
    } else {
        dsk.win.focus.close(Ventana::Cabina);
        erase_window(
            &p, &dsk.run_box, dsk.win.cabina.chrome.x, dsk.win.cabina.chrome.y,
            dsk.win.cabina.chrome.width, dsk.win.cabina.chrome.height, dsk.win.visible,
        );
        dsk.win.top_before = Ventana::Run;
        uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        // Si Datos estaba abierta debajo, vuelve a verse.
        if dsk.win.data_open {
            scene::data::paint(&p, &dsk.win.data);
        }
    }
    return Key::Taken;
}

// -- F10: la ventana del SONIDO --
//
// Calcada de F11, y con una diferencia que no es cosmetica:
// aqui abrir y cerrar **toman y devuelven un aparato**, no solo
// pintan. Por eso el orden importa en los dos sentidos --
// reclamar antes de pintar (para que la ventana ensene lo que
// de verdad hay) y CALLAR antes de soltar (un tono que sigue
// sonando despues de devolver el aparato es del sistema, y el
// sistema no pidio ese tono).
let toggle_sound = if c == 0x92 {
    Some(!dsk.win.sound_open)
} else if c == 0x1B && dsk.win.sound_open && dsk.win.focus.es_para(Ventana::Sound) {
    Some(false)
} else {
    None
};
if let Some(open) = toggle_sound {
    dsk.win.sound_open = open;
    if open {
        // Puede fallar, y entonces la ventana lo DICE en vez de
        // pintar un volumen que no manda sobre nada.
        dsk.snd.cap = bmo::Sonido::claim();
        dsk.snd.devices = match &dsk.snd.cap {
            Some(s) => {
                s.volumen(dsk.snd.volume);
                s.aparatos()
            }
            None => 0,
        };
        dsk.snd.pressed = None;
        dsk.win.focus.open(Ventana::Sound);
        scene::sound::paint(
            &p, &dsk.win.sound, dsk.snd.cap.is_some(),
            dsk.snd.devices, dsk.snd.volume, dsk.snd.pressed,
        );
        dsk.win.top_before = if dsk.win.focus.es_para(Ventana::Sound) { Ventana::Sound } else { Ventana::Run };
        if dsk.win.top_before == Ventana::Run {
            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        }
    } else {
        // * DEVOLVER EL APARATO. Esto es lo que impide que el
        // escritorio deje mudos a todos los programas que lanza.
        if let Some(s) = dsk.snd.cap.take() {
            s.callar();
            s.release();
        }
        dsk.win.focus.close(Ventana::Sound);
        erase_window(
            &p, &dsk.run_box, dsk.win.sound.chrome.x, dsk.win.sound.chrome.y,
            dsk.win.sound.chrome.width, dsk.win.sound.chrome.height, dsk.win.visible,
        );
        dsk.win.top_before = Ventana::Run;
        uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        // Si habia ventanas debajo, vuelven a verse.
        if dsk.win.data_open {
            scene::data::paint(&p, &dsk.win.data);
        }
        if dsk.win.cabina_open {
            scene::cabina::paint(&p, &dsk.win.cabina);
        }
    }
    return Key::Taken;
}
    Key::Pass
}
