//! **Commands about the desktop itself**: what it says, what it shows, and
//! what it does when it does not understand you.
//!
//! Nothing in here touches the disk or asks the kernel anything. That is the
//! whole boundary -- if a command needs a file it lives in `files.rs`, if it
//! needs an `OP_INFO` it lives in `system.rs`.

use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::calc::paint_calc;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_PLAIN};
use crate::scene::{paint_status, scene_color, INK_BAD, INK_DIM};
use crate::text::decimal;

pub(crate) fn nothing(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(&p, &dsk.run_box, "escribe algo", INK_DIM);
    After::Settle
}

/// ** ESTO NO ES UNA DISTRO, y se dice con un gato.
///
/// Un amigo del dueno, que viene de Linux, se sento
/// delante y dio por hecho que lo era. Es un
/// malentendido razonable --hay escritorio, ventanas
/// y una caja donde teclear-- y "no lo conozco"
/// habria sido correcto sin ensenar nada.
///
/// Asi que la respuesta cuenta lo que de verdad
/// separa a los dos sistemas: aqui no hay usuarios
/// que elevar ni paquetes que instalar. Hay
/// capabilities, y lo que no te dieron no existe
/// para ti. Se rie del malentendido, nunca de quien
/// lo tuvo.
pub(crate) fn not_linux(dsk: &mut Desktop, p: &bmo::Pantalla, verb: &[u8]) -> After {
    dsk.out.grid.text(b"    n_n_n
");
    dsk.out.grid.text(b"   ( -.- )   ~nya. eso aqui no se dice.
");
    dsk.out.grid.text(b"   ( u u )   esto NO es Linux, es BMO-X.
");
    dsk.out.grid.text(b"    ^^ ^^    no hay root que pedir:
");
    dsk.out.grid.text(b"             o te dieron la capability, o no existe.
");
    dsk.out.grid.text(b"
");
    let hint: &[u8] = match verb {
        b"sudo" | b"su" => {
            b"  aqui nadie ELEVA permisos: un proceso nace con lo que le
  concedieron, y no hay forma de pedir mas.
"
        }
        b"apt" | b"apt-get" | b"pacman" | b"yay" | b"dnf" | b"yum"
        | b"snap" => {
            b"  no hay repositorios. Los programas se compilan aqui, con el
  toolchain propio, y salen en .bex.
"
        }
        b"systemctl" => {
            b"  no hay demonios. Un servicio es un proceso de Ring 3 con su
  capability, y se lanza con `run`.
"
        }
        b"chmod" | b"chown" => {
            b"  no hay bits de permiso ni duenos. El permiso ES el handle:
  sin el, el objeto no se puede ni nombrar.
"
        }
        b"man" => b"  prueba `ayuda`. Es mas corta y cabe en la pantalla.
",
        b"grep" => b"  prueba `cat` y la rueda. O F11, que filtra por gravedad.
",
        _ => b"  prueba `ayuda` para ver lo que SI hay.
",
    };
    dsk.out.grid.text(hint);
    paint_status(&p, &dsk.run_box, "esto no es Linux :3", INK_DIM);
    dsk.tick.repaint_field = true;
    After::Settle
}

/// * `perf` -- el numero antes que la tarjeta.
///
/// Se pinta ANTES de leer los contadores no: se leen
/// aqui y se imprimen, y el fotograma que los pinta
/// sumara el suyo. Da igual: lo que interesa es el
/// orden de magnitud y el peor caso, no un digito.
pub(crate) fn paint_cost(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    let v = p.volcado();
    dsk.out.grid.text(b"  pintado\n");
    dsk.out.grid.text(b"    modo        ");
    dsk.out.grid.text(match v.modo {
        bmo::Volcador::Ninguno => b"directo al panel (SIN doble bufer)\n" as &[u8],
        bmo::Volcador::Directo => b"doble bufer, volcado por CPU\n",
    });
    dsk.out.grid.text(b"    fotogramas  ");
    let mut d = [0u8; 10];
    let k = decimal(v.fotogramas, &mut d);
    dsk.out.grid.text(&d[..k]);
    dsk.out.grid.text(b"   con algo que mover\n");
    if v.fotogramas > 0 {
        dsk.out.grid.text(b"    medio      ");
        let k = decimal(v.bytes / v.fotogramas / 1024, &mut d);
        dsk.out.grid.text(&d[..k]);
        dsk.out.grid.text(b" KiB por fotograma\n");
        // El PEOR caso va aparte y a proposito: un
        // tiron se nota y una media buena lo tapa.
        dsk.out.grid.text(b"    peor       ");
        let k = decimal(v.peor / 1024, &mut d);
        dsk.out.grid.text(&d[..k]);
        dsk.out.grid.text(b" KiB en un fotograma\n");
        dsk.out.grid.text(b"    total      ");
        // ** Y CUANTAS CAJAS tenia ese peor
        // fotograma. Con la caja unica de antes
        // esto seria SIEMPRE 1 y el `worst` la
        // pantalla entera; si aqui sale 2 o 3 con
        // un peor pequeno, el troceado trabaja.
        dsk.out.grid.text(b"    cajas      ");
        let k = decimal(v.cajas as u64, &mut d);
        dsk.out.grid.text(&d[..k]);
        dsk.out.grid.text(b"
");
        let k = decimal(v.bytes / 1024 / 1024, &mut d);
        dsk.out.grid.text(&d[..k]);
        dsk.out.grid.text(b" MiB movidos desde el arranque\n");
    }
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"    la caja de sucio ya recorta esto: una GPU solo\n");
    dsk.out.grid.text(b"    compra algo si estos numeros son grandes.\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    paint_status(&p, &dsk.run_box, "listo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn calculator(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    dsk.calc.visible = !dsk.calc.visible;
    if dsk.calc.visible {
        paint_calc(&p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
        dsk.out.grid.text(b"  calculadora: la cara en Rust, el calculo en COBOL
");
    } else {
        // Devolver esa zona a la escena.
        for f in 0..dsk.calc_pad.height {
            for co in 0..dsk.calc_pad.width {
                let (px, py) = (dsk.calc_pad.x + co, dsk.calc_pad.y + f);
                p.punto(px, py, scene_color(&dsk.run_box, dsk.win.visible, px, py, p.alto));
            }
        }
    }
    paint_status(&p, &dsk.run_box, "listo", INK_DIM);
    dsk.field.n = 0;
    dsk.field.cur = 0;
    After::Settle
}

pub(crate) fn clear(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    dsk.out.grid.clear();
    paint_status(&p, &dsk.run_box, "listo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn help(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    dsk.out.grid.text(b"  <ruta>       lanza un .bex   (cobol/banco.bex)\n");
    dsk.out.grid.text(b"  run <ruta>   lo mismo, como en el shell de Ring 0\n");
    // Va JUSTO detras de `run` porque es su hermana,
    // y con la consecuencia delante: lo que sorprende
    // no es que lance, es que el escritorio se vaya.
    dsk.out.grid.text(b"  presta <ruta>  se lo lanza CON LA PANTALLA: el\n");
    dsk.out.grid.text(b"               escritorio se aparta y vuelve cuando\n");
    dsk.out.grid.text(b"               el programa termina  (c/ray.bex)\n");
    dsk.out.grid.text(b"  cat <ruta>   ensena lo que hay dentro\n");
    dsk.out.grid.text(b"  write <ruta> <texto>     lo guarda\n");
    dsk.out.grid.text(b"  save [ruta]  vuelca esta salida a un .txt, con la\n");
    dsk.out.grid.text(b"               tabla de consumo dentro  (= guarda)\n");
    dsk.out.grid.text(b"               (por defecto datos/salida.txt, y cada\n");
    dsk.out.grid.text(b"                programa que corre lo deja solo ahi)\n");
    dsk.out.grid.text(b"  clear / cls  limpia esta salida\n");
    dsk.out.grid.text(b"  TAB          completa   Ctrl+A/E inicio/fin\n");
    dsk.out.grid.text(b"  Ctrl+K corta al final    Ctrl+W borra palabra\n");
    dsk.out.grid.text(b"  Ctrl+U borra linea       Ctrl+L limpia\n");
    dsk.out.grid.text(b"  info         RAM, CPU, tareas y disco\n");
    dsk.out.grid.text(b"  cpu / mem    solo esa parte del informe\n");
    dsk.out.grid.text(b"  ext          que ofrece el silicio y que coge BMO\n");
    dsk.out.grid.text(b"  consumo / w  nucleos, hilos, MHz, W y RAM en TABLA\n");
    dsk.out.grid.text(b"  apps         que programa tiene RAM pedida\n");
    dsk.out.grid.text(b"  save cpu|mem|consumo|apps   cada tabla en SU\n");
    dsk.out.grid.text(b"               fichero: datos/cpu.txt, mem.txt...\n");
    dsk.out.grid.text(b"  perf         lo que cuesta pintar, medido\n");
    // ** La unica caja de ordenes que puede cambiar el almacen. Se lista con su
    // suborden destructiva SEPARADA y con el `ya` a la vista: una ayuda que
    // dijera solo `disco` esconderia justo lo que hay que leer antes.
    dsk.out.grid.text(b"  disco        que aparato es, cuanto queda y que se le\n");
    dsk.out.grid.text(b"               ha devuelto  (disco espacio | barrera)\n");
    dsk.out.grid.text(b"  disco trim   PROPONE devolverle al disco la cola libre\n");
    dsk.out.grid.text(b"               del volumen; `disco trim ya` la manda\n");
    dsk.out.grid.text(b"  estratos sellar   ESCRIBE EN EL DISCO (commit vacio)\n");
    dsk.out.grid.text(b"  help         esto\n");
    dsk.out.grid.text(b"  reboot       reinicia la maquina\n");
    dsk.out.grid.text(b"  Ctrl+Alt     esconde o invoca esta ventana\n");
    paint_status(&p, &dsk.run_box, "listo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// Ni se intenta lanzar. Se dice lo que es y con
/// que se abre -- un mensaje sobre la FIRMA aqui
/// manda a buscar un permiso que no hace falta.
pub(crate) fn not_a_program(dsk: &mut Desktop, p: &bmo::Pantalla, r: &[u8]) -> After {
    dsk.out.grid.with_ink(INK_ERR);
    dsk.out.grid.text(b"  eso no es un programa (solo .bex se lanza).\n");
    dsk.out.grid.text(b"  para verlo:  cat ");
    dsk.out.grid.text(r);
    dsk.out.grid.byte(b'\n');
    dsk.out.grid.with_ink(INK_PLAIN);
    paint_status(&p, &dsk.run_box, "no es un programa: prueba lee", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn unknown(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    // El mensaje honesto. Antes se contestaba "no
    // esta: revisa la ruta" a quien escribia
    // `reboot`, y eso manda a buscar un archivo que
    // nunca existio en vez de decir la verdad.
    dsk.out.grid.text(b"  no es un comando ni una ruta. escribe 'help'.\n");
    paint_status(&p, &dsk.run_box, "no lo conozco: prueba help", INK_BAD);
    dsk.field.n = 0;
    After::Settle
}
