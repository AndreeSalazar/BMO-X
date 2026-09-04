//! **The five window toggles**: F7 cpu, F8 memory, F10 sound, F11 CABINA,
//! F12 data -- and the ESC that closes each one.
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
if c == 0x8C && alt_alone {
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
            dsk.out.grid.text(b"  Alt+F4 cierra la ventana de delante. Esta es la casa.
");
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
