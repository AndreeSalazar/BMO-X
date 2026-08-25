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
    dsk.out.grid.text(b"    n_n_n\n");
    dsk.out.grid.text(b"   ( -.- )   ~nya. eso aqui no se dice.\n");
    dsk.out.grid.text(b"   ( u u )   esto NO es Linux, es BMO-X.\n");
    dsk.out.grid.text(b"    ^^ ^^    no hay root que pedir:\n");
    dsk.out.grid.text(b"             o te dieron la capability, o no existe.\n");
    dsk.out.grid.text(b"\n");
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
        dsk.out.grid.text(b"\n");
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
        // ** Abrirla le da el teclado, igual que `Ctrl+n` abre la consola de
        // ESTRATOS y se lo queda de paso. Pedir la calculadora Y ADEMAS pedirle
        // el teclado serian dos ordenes para una sola intencion.
        dsk.calc.keys = true;
        paint_calc(&p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
        // La cara ya no es Rust: la compila MAQUETA desde `calc.maqueta`. Este
        // renglon lo decia y llevaba mintiendo desde el dia del puerto.
        dsk.out.grid.text(b"  calculadora: la cara en MAQUETA, el calculo en COBOL\n");
        dsk.out.grid.text(b"  las teclas son suyas; Ctrl+n se las devuelve a la linea\n");
    } else {
        // Cerrarla suelta el teclado. Sin esto, volver a abrirla lo robaria en
        // silencio: la bandera seguiria puesta de la vez anterior.
        dsk.calc.keys = false;
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

/// **LA AYUDA, POR CATEGORIAS.**
///
/// === Por que dejo de ser una lista ===
///
/// Eran treinta renglones seguidos, sin un solo rotulo, y el dueno lo dijo con
/// la comparacion exacta: *"imaginate con help en CMD de Windows, es molesto"*.
/// Y es el mismo defecto: una lista plana obliga a **leerla entera** para saber
/// si lo que buscas esta, porque no hay forma de descartar un trozo de un
/// vistazo. Con seis rotulos, el que busca como se mira el disco lee cuatro
/// renglones y para.
///
/// El orden no es alfabetico ni historico: va **de lo que se usa mas a lo que se
/// usa menos**, y dentro de cada grupo, la orden entera antes que sus variantes.
///
/// ** Y de paso se cayo una mentira: `presta <ruta>` llevaba anunciado aqui
/// desde que se RETIRO. Hoy lo decide el compositor leyendo la bandera
/// `WANTS_SCREEN` de la cabecera BEF --ver `main.rs`-- porque saberse de memoria
/// que programas son graficos era poner la politica en los dedos del usuario.
/// Una ayuda que ofrece una orden que contesta "no lo conozco" es peor que una
/// ayuda incompleta: la incompleta no miente.
pub(crate) fn help(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"  PROGRAMAS ---------------------------------------------------\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.out.grid.text(b"    <ruta>        lanza un .bex      cobol/2/banco.bex\n");
    dsk.out.grid.text(b"    run <ruta>    lo mismo, como en el shell de Ring 0\n");
    dsk.out.grid.text(b"                  si el programa pide pantalla, se le presta sola\n");
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"  ARCHIVOS ----------------------------------------------------\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.out.grid.text(b"    ls [ruta]     que hay\n");
    dsk.out.grid.text(b"    cat <ruta>    que hay DENTRO\n");
    dsk.out.grid.text(b"    write <ruta> <texto>      lo guarda\n");
    dsk.out.grid.text(b"    save [ruta]   vuelca esta salida a un .txt, con el consumo\n");
    dsk.out.grid.text(b"                  dentro   (por defecto datos/salida.txt)\n");
    dsk.out.grid.text(b"    save cpu|mem|consumo|apps   cada tabla en SU fichero\n");
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"  EL DISCO ----------------------------------------------------\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.out.grid.text(b"    disco         que aparato es, cuanto queda y que se le ha\n");
    dsk.out.grid.text(b"                  devuelto      (disco espacio | disco barrera)\n");
    dsk.out.grid.text(b"    disco trim    PROPONE devolverle al disco la cola libre;\n");
    dsk.out.grid.text(b"                  `disco trim ya` la manda de verdad\n");
    dsk.out.grid.text(b"    estratos sellar   ESCRIBE EN EL DISCO (commit vacio)\n");
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"  EL SISTEMA --------------------------------------------------\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.out.grid.text(b"    info          RAM, CPU, tareas y disco\n");
    dsk.out.grid.text(b"    cpu / mem     solo esa parte del informe\n");
    dsk.out.grid.text(b"    consumo       nucleos, hilos, MHz, W y RAM en TABLA\n");
    dsk.out.grid.text(b"    apps          que programa tiene RAM pedida\n");
    dsk.out.grid.text(b"    ext           que ofrece el silicio y que coge BMO\n");
    dsk.out.grid.text(b"    fallo         la ultima autopsia de Ring 3\n");
    dsk.out.grid.text(b"    cabina        TODO lo que el kernel apunto  (todo | fallos | N)\n");
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"  LA MAQUINA --------------------------------------------------\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.out.grid.text(b"    smp           los nucleos    (all | N | test | stop)\n");
    dsk.out.grid.text(b"    red           tarjeta, enlace y tramas   (mac|link|tramas|phy)\n");
    dsk.out.grid.text(b"    banda         el ancho de banda de la RAM, y el techo de\n");
    dsk.out.grid.text(b"                  tokens/s que sale de el  (pide `smp all` antes)\n");
    dsk.out.grid.text(b"    audio         como quiere las muestras el aparato\n");
    dsk.out.grid.text(b"    reboot        reinicia la maquina\n");
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"  ESTA CAJA ---------------------------------------------------\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.out.grid.text(b"    clear         limpia esta salida\n");
    dsk.out.grid.text(b"    calc          la calculadora     perf   lo que cuesta pintar\n");
    dsk.out.grid.text(b"    TAB completa            Ctrl+A / Ctrl+E   inicio / fin\n");
    dsk.out.grid.text(b"    Ctrl+K corta al final   Ctrl+W borra palabra   Ctrl+U linea\n");
    dsk.out.grid.text(b"    Ctrl+Alt esconde o invoca esta ventana\n");
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"  LAS TECLAS --------------------------------------------------\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.out.grid.text(b"    F1..F10 ESCRIBEN la orden y la ejecutan, asi que queda en\n");
    dsk.out.grid.text(b"    el historial: la flecha arriba te ensena como se llama.\n");
    dsk.out.grid.text(b"    F11 y F12 no escriben nada -- ABREN UNA VENTANA.\n");
    dsk.out.grid.text(b"      ver           F1 help   F2 info    F3 consumo  F4 apps\n");
    dsk.out.grid.text(b"      la maquina    F5 red    F6 smp     F7 banda    F8 ext\n");
    dsk.out.grid.text(b"      cuando falla  F9 fallo  F10 disco\n");
    dsk.out.grid.text(b"      ventanas      F11 CABINA (el kernel)   F12 ESTRATOS\n");
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"  y si no sabes por donde empezar:  guia\n");
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
