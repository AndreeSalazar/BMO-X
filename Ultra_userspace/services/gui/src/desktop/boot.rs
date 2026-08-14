//! **Getting the machine**: claim the screen, say what happened, paint the
//! first frame, and hand back a [`Desktop`] that the loop can drive.
//!
//! Everything in here runs exactly once. That is the whole reason it is its
//! own file: it used to be the first 310 lines of `_start`, sharing a scope
//! with the 52 locals of a loop that never ends, so reading "what happens at
//! startup" meant reading a frame loop first.
//!
//! The order of the four `bmo::consola` lines is not decoration -- it is the
//! only diagnosis available when the desktop dies before it can paint:
//!
//! ```text
//!   "reclamo pantalla y entrada"  -> died in startup or in the splash
//!   "entrada a Ring 3 pintada"    -> died building the desktop
//!   "escritorio pintado"          -> died without closing the first frame
//!   "primer fotograma completo"   -> died already inside the loop
//! ```

use bmo_userland as bmo;

use super::Desktop;
use crate::scene::launcher::Launcher;
use crate::scene::{
    paint_background, paint_field, paint_run_box, paint_status, ACCENT, INK, INK_BAD, INK_DIM,
};
use crate::{scene, paint_output};

/// Claim the machine and paint the first frame.
///
/// The screen and the input capability come back as plain bindings and NOT as
/// fields of `Desktop`: `lend_screen` takes both by value and hands them back,
/// so they have to be movable. See the header of `desktop/mod.rs`.
///
/// El `Desktop` vuelve como **referencia a `.bss`** y no por valor: son ~92 KiB
/// y la pila de Ring 3 mide 64. Devolverlo por valor pondria el struct entero
/// --posiblemente dos veces, contando la ranura de retorno-- en el marco de una
/// funcion. Ver la cabecera de `DESKTOP` en `desktop/mod.rs`, que lleva el
/// desbordamiento del 2026-08-14 con sus numeros.
pub(crate) fn boot() -> (bmo::Pantalla, Option<bmo::Entrada>, &'static mut Desktop) {
    // El aviso va ANTES de reclamar: en cuanto la cesion se consuma, el kernel
    // deja de dibujar y nada de lo que se imprima despues llega al panel.
    bmo::consola("reclamo pantalla y entrada\n");

    let Some(mut p) = bmo::Pantalla::claim() else {
        bmo::consola("sin pantalla que reclamar\n");
        bmo::salir()
    };

    // -- * EL DOBLE BUFER --
    //
    // Se pide ANTES de pintar nada, que es cuando la RAM esta menos
    // fragmentada: el bloque tiene que ser contiguo en fisico y son ~8 MB.
    //
    // Y se dice en los dos casos. Que no haya doble bufer **no impide arrancar**
    // --se dibuja en el panel, como siempre--, pero cambia dos cosas que se notan:
    // vuelve el riesgo de tearing y el cursor tiene que poner una barrera antes
    // de leer. Un escritorio que se degrada en silencio es un escritorio del que
    // no se puede diagnosticar nada.
    if p.activar_doble_bufer() {
        bmo::consola("doble bufer: pintando fuera de la pantalla\n");
    } else {
        bmo::consola("SIN doble bufer: no hubo bloque, pinto directo al panel\n");
    }
    // La entrada es opcional a proposito: sin ella hay escritorio, solo que
    // quieto y mudo. Un compositor que se niega a arrancar porque falta un
    // periferico es un compositor que no arranca el dia que el periferico falla.
    // `mut` porque `presta` la SUELTA y la vuelve a reclamar: la capability se
    // va y vuelve, asi que el binding tiene que poder cambiar.
    let input = bmo::Entrada::claim();

    // La consola de este terminal. Desde aqui, todo lo que lance escribe en
    // ESTE anillo y no en el panel del kernel -- que es lo unico que separaba
    // una caja de lanzar de un terminal de verdad.
    let child_console = bmo::Consola::create();
    let has_console = child_console.is_some();

    // -- LA ENTRADA A RING 3 --
    //
    // Antes de dibujar nada del escritorio, decir lo que acaba de pasar: el
    // userspace tiene la maquina. Hasta hoy este paso era invisible y por eso
    // un compositor muerto y un compositor que no pinta se veian igual -- un
    // shell donde debia haber un escritorio.
    //
    // Y lleva las dos capabilities OPCIONALES escritas en la cara, que es lo
    // que distingue "no funciona" de "no me la dieron".
    // * Y la espera del final se puede SALTAR con una tecla, por eso va la
    // capability y no un `bool`: 1.100 de los 1.205 ms hasta el escritorio eran
    // esa espera, y el dueno la leyo como un fallo mirando el cronometro del
    // klog. Tenia razon en sospechar.
    scene::splash::paint(&p, input.as_ref(), has_console);
    bmo::consola("entrada a Ring 3 pintada\n");

    // -- El escritorio --
    //
    // * Aqui vivian los SEIS PARCHES DE MEDIDA y el PULSOMETRO del raton, y se
    // han quitado el 2026-08-04. No eran decoracion: los parches contestaban
    // "el orden de canales es el que creo?" y la barra contestaba "llegan
    // informes del raton?". **Las dos preguntas estan contestadas** -- los
    // colores salen bien desde hace semanas y el puntero se mueve donde se
    // mueve la mano, o sea que el propio cursor ES el pulsometro.
    //
    // Un instrumento que ya no mide nada deja de ser un instrumento y pasa a
    // ser ruido: seis cuadrados de colores puros y una barra en mitad del
    // escritorio son lo que hacia que esto pareciera un panel de pruebas y no
    // una maquina. Si algun dia hay que volver a medir el formato del
    // framebuffer, el `git log` tiene los valores exactos con su porque.
    paint_background(&p);
    // ** LOS ICONOS, y se leen UNA VEZ.
    //
    // Recorrer `apps\` y sacarle el icono a cada `.bex` son varias lecturas de
    // disco por app, y ninguna cambia mientras la maquina esta encendida. Un
    // escritorio que releyera el directorio por fotograma haria E/S sesenta
    // veces por segundo para ensenar exactamente lo mismo.
    //
    // Va JUSTO DESPUES del fondo y antes de todo lo demas: los iconos son lo de
    // mas atras que se pinta, igual que en cualquier escritorio.
    let launcher = Launcher::new();
    scene::launcher::paint(&p, &launcher);
    p.rect(16, 13, 14, 14, ACCENT);
    p.texto(38, 14, "BMO-X", INK);

    // ** AQUI NACE EL ESTADO, y de una vez.
    //
    // Eran 52 `let mut` repartidos entre las lineas que pintan. Ahora es un
    // struct: lo que antes se declaraba a mitad del arranque ya existe entero
    // antes de que se pinte el primer campo, y el orden de las declaraciones
    // deja de ser algo que haya que respetar de memoria.
    // `install` lo construye DENTRO de `.bss` y devuelve la unica referencia:
    // aqui no se materializa ningun `Desktop` en la pila.
    let d = super::install(&p, child_console, launcher);

    // Lo que SI era informacion y no instrumento: si la entrada no se pudo
    // reclamar hay que decirlo, y ahora se dice con palabras en la barra en vez
    // de con el color de un marco. Un rojo sin texto obliga a saberse el
    // codigo de colores.
    if input.is_none() {
        // El aviso se coloca por su LARGO REAL y no por un numero a ojo: son
        // cuarenta letras, y con un hueco puesto a mano de treinta y cuatro se
        // saldria por la derecha justo el dia que haga falta leerlo.
        const WARN: &str = "SIN ENTRADA: teclado y raton son de otro";
        let width = bmo::Pantalla::ancho_escala(WARN, 1);
        p.texto(p.ancho.saturating_sub(width + 16), 14, WARN, INK_BAD);
    }

    paint_run_box(&p, &d.run_box);
    if !has_console {
        d.out
            .grid
            .text(b"sin consola: la salida de los programas ira al panel del kernel\n");
    }
    paint_field(&p, &d.run_box, d.field.line(), d.field.cur, true);
    paint_output(&p, &d.run_box, &d.out.grid);
    if input.is_some() {
        paint_status(&p, &d.run_box, "listo", INK_DIM);
    } else {
        // Decirlo, y decir por que. Una caja que no responde y no explica nada
        // es peor que no tener caja.
        paint_status(
            &p,
            &d.run_box,
            "sin teclado: la entrada no se pudo reclamar",
            INK_BAD,
        );
    }

    bmo::consola("escritorio pintado\n");
    (p, input, d)
}
