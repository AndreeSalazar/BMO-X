//! **EL GATO**: la ventanita que sale cuando alguien teclea Linux, Windows o
//! Mac aqui.
//!
//! [consumo] NADA      no corre en reposo: se pinta UNA vez al teclear la
//!                     orden, y se borra con la siguiente tecla o clic (L6h)
//!
//! # De donde sale
//!
//! Un amigo del dueno, que venia de Linux, se sento delante y tecleo `sudo`.
//! La respuesta ya existia en `commands/shell.rs`, pero salia como texto suelto
//! dentro de la salida. El dueno lo pidio asi: *"que genere ventana, con ASCII,
//! como burla indirecta :3"* -- y despues, con un boceto delante: *"mas elegante
//! y simple, el gato variando en cada comando, y si es Windows XD, y si es Mac
//! pues ni modo"*.
//!
//! ** Se rie del MALENTENDIDO, nunca de quien lo tuvo, y cada burla dice la
//! diferencia de verdad y a donde ir. Una broma que no ensena nada es ruido.
//!
//! # La forma, que es la del boceto
//!
//! ```text
//!    +-----------------------------------------------------+
//!    | BMO-X // METAKERNEL // LEY 24 // SO: NONE           |
//!    +------------+----------------------------------------+
//!    |   /\_/\    | Comando:     `sudo`                    |
//!    |  ( o.o )   | Burla:       "Nyaa~ ..."               |
//!    |   > ^ <    | Explicacion: ...                       |
//!    +------------+----------------------------------------+
//! ```
//!
//! # Como vive
//!
//! Igual que el conmutador de Alt+Tab (`switcher.rs`): una bandera dice que
//! esta pintada, y quien la pinto la borra. Mientras esta, la caja de Ejecutar
//! no se repinta encima (las guardas de `paint.rs`).

use bmo_userland as bmo;

use super::*;
use crate::desktop::Desktop;
use crate::uncover;

const N_FONDO: u32 = 0x0010_141C;
const N_LINEA: u32 = 0x00B8_C4D0;
const N_GATO: u32 = 0x00F4_C6E4;

const FILA: u32 = bmo::GLIFO_ALTO + 6;
/// Ancho de la columna del gato, en caracteres.
const COL_GATO: u32 = 13;
/// Ancho de la columna del texto, en caracteres: "Explicacion: " y 45 mas.
const COL_TEXTO: u32 = 59;
const MARGEN: u32 = 12;

/// De donde viene la costumbre. Cambia la frase de la salida y de la barra.
#[derive(Clone, Copy)]
pub(crate) enum Familia {
    Linux,
    Windows,
    Mac,
}

impl Familia {
    /// Para la barra de estado de la caja de Ejecutar.
    pub(crate) fn estado(self) -> &'static str {
        match self {
            Familia::Linux => "esto no es Linux :3",
            Familia::Windows => "esto no es Windows XD",
            Familia::Mac => "esto no es Mac... ni modo",
        }
    }

    /// Para la linea que queda en la salida.
    pub(crate) fn nombre(self) -> &'static [u8] {
        match self {
            Familia::Linux => b"Linux",
            Familia::Windows => b"Windows",
            Familia::Mac => b"Mac",
        }
    }
}

/// Lo que dice el gato: su cara, la burla y la explicacion en una linea.
pub(crate) struct Burla {
    pub cara: [&'static str; 3],
    pub frase: &'static str,
    pub explica: &'static str,
    pub familia: Familia,
}

const fn b(
    cara: [&'static str; 3],
    frase: &'static str,
    explica: &'static str,
    familia: Familia,
) -> Burla {
    Burla { cara, frase, explica, familia }
}

/// **La burla de cada verbo.** Frase y explicacion caben en 45 columnas.
///
/// El `_` del final no es un "no lo se": `FROM_LINUX` (en `commands/mod.rs`,
/// que ya lleva tambien Windows y Mac) solo manda aqui lo que viene de fuera, asi que el comodin
/// es la respuesta general para un verbo sin burla propia.
pub(crate) fn burla(verb: &[u8]) -> Burla {
    use Familia::*;
    match verb {
        b"sudo" | b"su" | b"doas" => b(
            [r" /\_/\ ", r"( o.o )", r" > ^ < "],
            "Nyaa~ sudo? aqui nadie es root!",
            "Capabilities: lo que no te dieron, no existe",
            Linux,
        ),
        b"apt" | b"apt-get" | b"pacman" | b"yay" | b"paru" | b"dnf" | b"yum" | b"zypper"
        | b"emerge" | b"snap" | b"flatpak" => b(
            [r" /\_/\ ", r"( ^.^ )", r" > w < "],
            "Nyaa~ repositorios? que tierno!",
            "Aqui no se instala: se compila a un .bex",
            Linux,
        ),
        b"systemctl" | b"service" | b"journalctl" => b(
            [r" /\_/\ ", r"( -.- )", r" z z z "],
            "Nyaa~ demonios? aqui se duerme bien.",
            "Un servicio es un proceso de Ring 3: run",
            Linux,
        ),
        b"chmod" | b"chown" | b"chgrp" => b(
            [r" /\_/\ ", r"( >.< )", r" > ~ < "],
            "Nyaa~ chmod 777? no hay bits!",
            "El permiso ES el handle: sin el, ni existe",
            Linux,
        ),
        b"mount" | b"umount" | b"fdisk" | b"mkfs" | b"dd" | b"lsblk" => b(
            [r" /\_/\ ", r"( o_O )", r" / | \ "],
            "Nyaa~ montar? si ya esta montado.",
            "El almacen se mira con: disco",
            Linux,
        ),
        b"kill" | b"killall" | b"ps" | b"top" | b"htop" => b(
            [r" /\_/\ ", r"( T.T )", r" > n < "],
            "Nyaa~ matar procesos? que brusco!",
            "Lo que corre y lo que gasta: consumo",
            Linux,
        ),
        b"man" => b(
            [r" /\_/\ ", r"( @.@ )", r" > - < "],
            "Nyaa~ 400 paginas? cabe en una.",
            "Lo que hay: ayuda.  Por donde: guia",
            Linux,
        ),
        b"grep" => b(
            [r" /\_/\ ", r"( 0.0 )", r" > ? < "],
            "Nyaa~ grep? tengo rueda y filtros.",
            "La rueda del raton, o F11 con filtros",
            Linux,
        ),
        b"vim" | b"vi" | b"nano" | b"emacs" => b(
            [r" /\_/\ ", r"( x.x )", r"  :q!  "],
            "Nyaa~ salir de vim? aqui ni entras.",
            "Para escribir: write <ruta> <texto>",
            Linux,
        ),
        b"neofetch" | b"fastfetch" | b"uname" => b(
            [r" /\_/\ ", r"( *.* )", r" > v < "],
            "Nyaa~ presumir la maquina? venga!",
            "La maquina: info, consumo y ext",
            Linux,
        ),
        b"bash" | b"zsh" | b"fish" | b"sh" => b(
            [r" /\_/\ ", r"( u.u )", r" > _ < "],
            "Nyaa~ otro shell? ya estas en uno.",
            "Esta caja ES la terminal. TAB completa",
            Linux,
        ),
        b"ipconfig" | b"tasklist" | b"taskkill" | b"regedit" | b"chkdsk" | b"diskpart"
        | b"sfc" | b"winget" | b"choco" | b"powershell" | b"cmd" | b"del" | b"notepad"
        | b"explorer" | b"systeminfo" => b(
            [r" /\_/\ ", r"( =w= )", r" > XD< "],
            "Nyaa~ Windows?! XD aqui no hay C:",
            "Ni registro ni C: -- hay handles y .bex",
            Windows,
        ),
        b"brew" | b"sw_vers" | b"diskutil" | b"launchctl" | b"defaults" | b"pbcopy"
        | b"open" | b"softwareupdate" | b"xcode-select" => b(
            [r" /\_/\ ", r"( ._. )", r" > . < "],
            "Nyaa~ Mac? ...bueno, ni modo.",
            "Ni brew ni Finder: toolchain propio y .bex",
            Mac,
        ),
        _ => b(
            [r" /\_/\ ", r"( o.o )", r" > ^ < "],
            "Nyaa~ eso es de otro sistema!",
            "BMO-X: bare metal orquestal. Prueba: ayuda",
            Linux,
        ),
    }
}

/// El rectangulo de la ventanita. **Una sola cuenta**, porque la usan pintar y
/// borrar -- la leccion de `switcher::run_box`, que tuvo dos copias.
fn caja(p: &bmo::Pantalla) -> (u32, u32, u32, u32) {
    let w = ((COL_GATO + COL_TEXTO) * bmo::GLIFO_ANCHO + MARGEN * 4 + 2)
        .min(p.ancho.saturating_sub(40));
    // titulo + raya + tres filas + pie, con aire arriba y abajo
    let h = FILA * 5 + MARGEN * 4 + 4;
    (p.ancho.saturating_sub(w) / 2, p.alto.saturating_sub(h) / 2, w, h)
}

/// **Pinta el gato**, centrado y encima de todo.
pub(crate) fn mostrar(dsk: &mut Desktop, p: &bmo::Pantalla, verb: &[u8]) {
    let (x, y, w, h) = caja(p);
    let bu = burla(verb);

    // El marco: fondo y una raya de un pixel alrededor.
    p.rect(x, y, w, h, N_LINEA);
    p.rect(x + 1, y + 1, w - 2, h - 2, N_FONDO);

    // El titulo, y la raya que lo separa.
    let ty = y + MARGEN;
    p.texto(x + MARGEN, ty, "BMO-X // METAKERNEL // LEY 24 // SO: NONE", ACCENT);
    let raya = ty + FILA + MARGEN / 2;
    p.rect(x + 1, raya, w - 2, 1, N_LINEA);

    // La raya vertical entre el gato y el texto.
    let cuerpo = raya + MARGEN;
    let div_x = x + MARGEN + COL_GATO * bmo::GLIFO_ANCHO;
    p.rect(div_x, raya, 1, FILA * 3 + MARGEN * 2, N_LINEA);
    p.rect(x + 1, cuerpo + FILA * 3 + MARGEN, w - 2, 1, N_LINEA);

    // El gato, centrado en su columna.
    let gx = x + MARGEN + (COL_GATO * bmo::GLIFO_ANCHO).saturating_sub(7 * bmo::GLIFO_ANCHO) / 2;
    for (i, linea) in bu.cara.iter().enumerate() {
        p.texto(gx, cuerpo + i as u32 * FILA, linea, N_GATO);
    }

    // Las tres filas, con su etiqueta.
    let tx = div_x + MARGEN;
    let corte = verb.len().min(30);
    let orden = core::str::from_utf8(&verb[..corte]).unwrap_or("?");
    let cx = p.texto(tx, cuerpo, "Comando:     `", INK_DIM);
    let cx = p.texto(cx, cuerpo, orden, INK_BAD);
    p.texto(cx, cuerpo, "`", INK_DIM);

    let bx = p.texto(tx, cuerpo + FILA, "Burla:       \"", INK_DIM);
    let bx = p.texto(bx, cuerpo + FILA, bu.frase, INK);
    p.texto(bx, cuerpo + FILA, "\"", INK_DIM);

    let ex = p.texto(tx, cuerpo + FILA * 2, "Explicacion: ", INK_DIM);
    p.texto(ex, cuerpo + FILA * 2, bu.explica, ACCENT);

    // El pie, pequeno y apagado.
    p.texto(
        x + MARGEN,
        cuerpo + FILA * 3 + MARGEN * 2,
        "cualquier tecla o clic cierra  :3",
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
