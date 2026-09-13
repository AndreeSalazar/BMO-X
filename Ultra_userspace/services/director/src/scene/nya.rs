//! **EL GATO**: la ventanita que sale cuando alguien teclea Linux aqui.
//!
//! [consumo] NADA      no corre en reposo: se pinta UNA vez al teclear la
//!                     orden, y se borra con la siguiente tecla o clic (L6h)
//!
//! # De donde sale
//!
//! Un amigo del dueno, que venia de Linux, se sento delante y tecleo `sudo`.
//! La respuesta ya existia en `commands/shell.rs`, pero salia como texto suelto
//! dentro de la salida, mezclado con lo demas. El dueno lo pidio asi: *"que
//! genere ventana, con ASCII, como burla indirecta :3"*.
//!
//! ** Se rie del MALENTENDIDO, nunca de quien lo tuvo -- y cada burla cuenta la
//! diferencia de verdad entre los dos sistemas y a donde ir en su lugar. Una
//! broma que no ensena nada es ruido; esta es la guia de alguien que llega de
//! otro sitio.
//!
//! # Como vive
//!
//! Igual que el conmutador de Alt+Tab (`switcher.rs`): se pinta encima de todo,
//! una bandera dice que esta pintada, y quien la pinto la borra. Mientras esta,
//! la caja de Ejecutar no se repinta encima (ver las guardas de `paint.rs`), y
//! al cerrarla se devuelve el fondo y se repinta lo de debajo.

use bmo_userland as bmo;

use super::*;
use crate::desktop::Desktop;
use crate::uncover;

const N_FONDO: u32 = 0x0012_1826;
const N_BORDE: u32 = 0x00A0_78C8;
const N_TITULO: u32 = 0x0024_1B36;
const N_GATO: u32 = 0x00F0_C8E8;

const FILA: u32 = bmo::GLIFO_ALTO + 4;
/// Columnas de texto que caben: el gato ocupa 17 y el mensaje el resto.
const COLS: u32 = 64;
const FILAS: u32 = 12;
/// Donde empieza la columna del mensaje, en caracteres.
const COL_TEXTO: u32 = 18;

/// El gato. Solo ASCII: la fuente del escritorio es de 8x16 y estos glifos
/// estan todos.
const GATO: [&str; 7] = [
    r"   /\_____/\   ",
    r"  /  o   o  \  ",
    r" ( ==  ^  == ) ",
    r"  )         (  ",
    r" (           ) ",
    r"( (  )   (  ) )",
    r"(__(__)___(__))",
];

/// Lo que dice el gato: una frase y hasta tres lineas de "lo que hay aqui".
pub(crate) struct Burla {
    pub frase: &'static str,
    pub lineas: [&'static str; 3],
}

/// **La burla de cada verbo.** Todas caben en 45 columnas.
///
/// El `_` del final no es un "no lo se": `FROM_LINUX` (en `commands/mod.rs`) solo
/// manda aqui lo que es de Linux, asi que el comodin cubre a los que no tienen
/// una respuesta propia, con la respuesta general.
pub(crate) fn burla(verb: &[u8]) -> Burla {
    match verb {
        b"sudo" | b"su" | b"doas" => Burla {
            frase: "Nyaa~ sudo? aqui nadie es root.",
            lineas: [
                "Un proceso nace con sus capabilities",
                "y no hay a quien pedirle mas.",
                "Lo que no te dieron, no existe.",
            ],
        },
        b"apt" | b"apt-get" | b"pacman" | b"yay" | b"paru" | b"dnf" | b"yum" | b"zypper"
        | b"emerge" | b"snap" | b"flatpak" => Burla {
            frase: "Nyaa~ repositorios? que tierno.",
            lineas: [
                "Aqui no se instala nada: se compila.",
                "El toolchain es de la casa, y",
                "cada programa sale en un .bex.",
            ],
        },
        b"systemctl" | b"service" | b"journalctl" => Burla {
            frase: "Nyaa~ demonios? aqui no hay.",
            lineas: [
                "Un servicio es un proceso de Ring 3",
                "con su capability, y se lanza con run.",
                "Lo que apunta el kernel: cabina.",
            ],
        },
        b"chmod" | b"chown" | b"chgrp" => Burla {
            frase: "Nyaa~ chmod 777? no hay bits.",
            lineas: ["El permiso ES el handle: sin el,", "el objeto ni siquiera se nombra.", ""],
        },
        b"mount" | b"umount" | b"fdisk" | b"mkfs" | b"dd" | b"lsblk" => Burla {
            frase: "Nyaa~ montar? el disco ya esta.",
            lineas: [
                "El almacen: ESTRATOS y FAT32.",
                "Para mirarlo:          disco",
                "Para devolver bloques: disco trim",
            ],
        },
        b"kill" | b"killall" | b"ps" | b"top" | b"htop" => Burla {
            frase: "Nyaa~ matar procesos? que brusco.",
            lineas: ["Lo que corre y lo que gasta: consumo", "Cerrar una app: su X, o Alt+F4.", ""],
        },
        b"man" => Burla {
            frase: "Nyaa~ 400 paginas? aqui cabe en una.",
            lineas: ["ayuda   lo que hay, por temas", "guia    por donde empezar", ""],
        },
        b"grep" => Burla {
            frase: "Nyaa~ grep? tengo rueda y filtros.",
            lineas: ["cat y la rueda del raton.", "F11: CABINA filtra por gravedad.", ""],
        },
        b"vim" | b"vi" | b"nano" | b"emacs" => Burla {
            frase: "Nyaa~ salir de vim? aqui ni entras.",
            lineas: ["Para escribir un archivo:", "  write <ruta> <texto>", ""],
        },
        b"neofetch" | b"fastfetch" | b"uname" => Burla {
            frase: "Nyaa~ presumir la maquina? venga.",
            lineas: [
                "info      RAM, CPU, tareas y disco",
                "consumo   W, nucleos y MHz en tabla",
                "ext       lo que ofrece el silicio",
            ],
        },
        b"bash" | b"zsh" | b"fish" | b"sh" => Burla {
            frase: "Nyaa~ otro shell? ya estas en uno.",
            lineas: ["Esta caja ES la terminal.", "TAB completa; flecha arriba: historial.", ""],
        },
        _ => Burla {
            frase: "Nyaa~ eso es de Linux.",
            lineas: [
                "Esto es BMO-X: bare metal orquestal.",
                "No hay usuarios, ni paquetes, ni root.",
                "Escribe ayuda para ver lo que SI hay.",
            ],
        },
    }
}

/// El rectangulo de la ventanita. **Una sola cuenta**, porque la usan pintar y
/// borrar -- la leccion de `switcher::run_box`, que tuvo dos copias.
fn caja(p: &bmo::Pantalla) -> (u32, u32, u32, u32) {
    let w = (COLS * bmo::GLIFO_ANCHO + 32).min(p.ancho.saturating_sub(40));
    let h = FILAS * FILA + 24;
    (p.ancho.saturating_sub(w) / 2, p.alto.saturating_sub(h) / 2, w, h)
}

/// **Pinta el gato**, centrado y encima de todo.
pub(crate) fn mostrar(dsk: &mut Desktop, p: &bmo::Pantalla, verb: &[u8]) {
    let (x, y, w, h) = caja(p);
    p.rect(x, y, w, h, N_BORDE);
    p.rect(x + 2, y + 2, w - 4, h - 4, N_FONDO);
    p.rect(x + 2, y + 2, w - 4, FILA + 8, N_TITULO);
    p.texto(x + 14, y + 8, "BMO-X  //  METAKERNEL  //  LEY 24  //  SO: NONE", ACCENT);

    let arriba = y + FILA + 24;
    for (i, linea) in GATO.iter().enumerate() {
        p.texto(x + 16, arriba + i as u32 * FILA, linea, N_GATO);
    }

    let b = burla(verb);
    let tx = x + 16 + COL_TEXTO * bmo::GLIFO_ANCHO;
    // La orden que se tecleo, citada y recortada: una linea larga no puede
    // salirse del marco.
    let corte = verb.len().min(36);
    let orden = core::str::from_utf8(&verb[..corte]).unwrap_or("?");
    let cx = p.texto(tx, arriba, "$ ", INK_DIM);
    p.texto(cx, arriba, orden, INK_BAD);
    p.texto(tx, arriba + FILA * 2, b.frase, INK);
    for (i, linea) in b.lineas.iter().enumerate() {
        p.texto(tx, arriba + FILA * (4 + i as u32), linea, INK_DIM);
    }

    p.texto(
        x + 16,
        y + h - FILA - 6,
        "cualquier tecla o clic cierra   --   ayuda: lo que SI hay",
        INK_DIM,
    );
    dsk.win.nya_painted = true;
}

/// **Borra el gato** y devuelve lo que tapaba. No hace nada si no estaba.
pub(crate) fn borrar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    if !dsk.win.nya_painted {
        return;
    }
    let (bx, by, ba, bh) = caja(p);
    // Una marca para el area entera, como el conmutador: `punto` marca pixel a
    // pixel, y marcar cuesta mas que pintar.
    p.marcar(bx, by, ba, bh);
    for fy in 0..bh {
        for fx in 0..ba {
            let (px, py) = (bx + fx, by + fy);
            p.punto_ya_marcado(px, py, scene_color(&dsk.run_box, dsk.win.visible, px, py, p.alto));
        }
    }
    dsk.win.nya_painted = false;
    // Lo de debajo, de abajo arriba: la caja de Ejecutar y los iconos, las dos
    // ventanas del sistema si estan abiertas, y las apps.
    uncover(p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
    if dsk.win.data_open {
        super::data::paint(p, &dsk.win.data);
    }
    if dsk.win.cabina_open {
        super::cabina::paint(p, &dsk.win.cabina);
    }
    for s in dsk.table.iter_mut() {
        s.repaint_all();
    }
}
