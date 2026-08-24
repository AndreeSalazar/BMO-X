//! **The keyboard**, and the shape it turned out to already have.
//!
//! ## The ninety free names were a mirage
//!
//! This block was measured at **90 names it used but did not declare**, which
//! is why it was the one thing left inside `_start` after the mouse, the frame
//! close and the twenty-one commands had come out. That number was real but it
//! was measuring the wrong thing: the block does **two jobs in one scope**.
//!
//! - GATHERING needs `&Entrada`, and only that. Modifiers, the key ring, the
//!   pointer, the wheel -- all copied into plain values.
//! - INTERPRETING needs `dsk`, `p` and one byte.
//!
//! Scoping the borrow of `&Entrada` to the gathering is what splits them, and
//! it is also what frees `input` for `lend_screen` -- which takes it **by
//! value**. The ninety were the two halves' names added together.
//!
//! ## And what is left is a CASCADE
//!
//! Every guard in the old loop ended the same way: `continue` if the key was
//! its own. That is not a control-flow detail, it is the design -- the key is
//! offered to each handler in order until one takes it:
//!
//! ```text
//!   shortcuts   Alt+Tab, Alt+M, Alt+arrows   window management by keyboard
//!   combo       AltGr in progress            cancels the Ctrl+Alt tap
//!   windows     F7 F8 F10 F11 F12 / ESC      the five toggles
//!   panels      the open panel's own keys    guarded by focus
//!   focus       not Run? the key is dropped
//!   editor      the line                     may answer `Launch`
//! ```
//!
//! Written as `continue` that order is invisible; written as [`Key::Taken`] it
//! is the first line of every handler's contract.

/// **Las teclas de una app en ventana**: el orden de quien es cada tecla, y
/// el buzon por donde viajan. Paso 2c de `docs/plan/PLAN_DIRECTOR.md`.
pub(crate) mod app;
pub(crate) mod editor;
pub(crate) mod panels;
pub(crate) mod shortcuts;
pub(crate) mod windows;

use bmo_userland as bmo;

use super::{Desktop, Ventana};
use crate::scene::{self, scene_color};
use crate::scene::{paint_status, INK_DIM};
use crate::{erase_box, uncover};
use crate::PATH_MAX;

/// Whether a handler claimed the key.
///
/// This is what `continue` used to say inside one 1.883-line loop. Out here
/// there is no loop to continue, so the intent becomes a value -- and Rust
/// does not let that be wrong: a `continue` with nothing to continue is an
/// error, not a silent fall-through.
#[derive(PartialEq, Eq)]
pub(crate) enum Key {
    /// The key was mine. Move on to the next one.
    Taken,
    /// Not mine. Let the next handler try.
    Pass,
}

/// What the line editor decided.
pub(crate) enum Edit {
    /// Handled here, nothing owed to the caller.
    Taken,
    /// `run`. The editor cannot do this one: `lend_screen` takes the screen
    /// and the input capability **by value**, and those live in `_start`.
    Launch([u8; PATH_MAX], usize),
}

/// One frame's worth of input, copied out of the capability.
///
/// Everything in here is a plain value on purpose: once this returns, the
/// borrow of `&Entrada` is over and `input` can be moved into `lend_screen`.
pub(crate) struct Gathered {
    pub keys: [u8; 64],
    pub nt: usize,
    pub pos: bmo::Punto,
    pub wheel: i32,
    pub ctrl: bool,
    /// Los modificadores crudos: `shortcuts` distingue mover de encajar por Shift.
    pub m: u8,
    pub combo: bool,
    pub alt_alone: bool,
}

/// Drain the keyboard ring and read the pointer. Reads NOTHING of the desktop
/// except the keys it injected itself.
pub(crate) fn gather(dsk: &mut Desktop, e: &bmo::Entrada) -> Gathered {
    let m = e.modificadores();
    let ctrl = m & bmo::MOD_CTRL != 0;
    // En la distribucion espanola `Ctrl+Alt` **es** `AltGr` -- lo que produce
    // `@`, `#`, `[`, `]`, `\` y `EUR`. Por eso el atajo se dispara al SOLTAR y
    // solo si no llego ningun caracter mientras estaban pulsados.
    let combo = ctrl && m & bmo::MOD_ALT != 0;
    let alt_alone = m & bmo::MOD_ALT != 0 && !ctrl;

    // ** LA COLA CRUDA, ANTES DE COCINAR NADA.
    //
    // Va aqui y no en la cascada de `dispatch` porque son DOS COLAS distintas
    // y esta no pasa por ahi: la de abajo son caracteres --lo que se escribe en
    // la linea de Ejecutar-- y esta son scancodes con su flanco. Las dos se
    // llenan del mismo sondeo y leer una no le roba nada a la otra, asi que el
    // escritorio puede seguir cocinando sus atajos mientras la app con foco
    // recibe la tecla entera. Ver `keys::app`.
    app::reenviar(dsk, e, m);

    // ** EL INVARIANTE DEL CAMPO: el cursor NUNCA pasa del texto.
    //
    // `cur <= n` lo dan por hecho las tres teclas que borran, y las tres restan
    // de `n`. Romperlo una vez --un camino que pone `n = 0` y se olvida de
    // `cur`-- deja una mina que no explota hasta que alguien pulsa retroceso, y
    // entonces `n` se desborda por abajo y el escritorio entero se cae con un
    // `usize::MAX`. Paso en el Ryzen el 2026-08-09.
    //
    // Se restaura AQUI, una vez por vuelta y en un solo sitio, en vez de ir
    // persiguiendo cada `n = 0` del fichero. Cuesta una comparacion por
    // fotograma y **quita la clase entera de fallo**: cualquier camino futuro
    // que se olvide de `cur` queda corregido antes de que nadie pueda teclear.
    //
    // [!] Y por eso vive en `gather` y no en la cascada: `gather` corre SIEMPRE
    // que hay entrada, mientras que un manejador puede no llegar a correr.
    dsk.field.cur = dsk.field.cur.min(dsk.field.n);

    let mut keys = [0u8; 64];
    let mut nt = 0usize;
    // Primero las que se metio el propio escritorio (un clic en un icono), y
    // luego las del teclado. El orden importa: lo inyectado es mas viejo.
    for k in 0..dsk.field.ni.min(keys.len()) {
        keys[nt] = dsk.field.injected[k];
        nt += 1;
    }
    dsk.field.ni = 0;
    while nt < keys.len() {
        match e.tecla() {
            Some(c) => {
                keys[nt] = c;
                nt += 1;
            }
            None => break,
        }
    }

    Gathered {
        keys,
        nt,
        m,
        pos: e.puntero(),
        wheel: e.rueda(),
        ctrl,
        combo,
        alt_alone,
    }
}


/// The two edges that need the turn BEFORE: releasing Alt+Tab and the
/// Ctrl+Alt tap.
///
/// They are edges and not states on purpose. `Ctrl+Alt` **is** AltGr on the
/// Spanish layout -- `@`, `#`, `[`, `]`, `\` and `EUR` all come from it -- so
/// firing on the press would break typing every one of those characters. The
/// tap fires on RELEASE, and only if no character arrived in between.
pub(crate) fn edges(dsk: &mut Desktop, p: &bmo::Pantalla, g: &Gathered) {

    // -- Alt+Tab: el conmutador --
    //
    // La pila se reordena al SOLTAR, no en cada Tab: eso es lo que hace
    // que pulsarlo dos veces te devuelva a donde estabas. Ver
    // `bmo_input::focus`.
    // ** La guarda es `switcher_painted`, NO `focus.conmutando()`.
    //
    // Eran dos estados distintos gobernando la misma cosa: uno dice
    // *que hay dibujado en la pantalla* y el otro *que cree la politica
    // de foco*. Mientras coincidan, bien; el dia que no --y en el Ryzen
    // no coincidieron-- el conmutador se queda pintado para siempre,
    // porque el unico que sabia borrarlo estaba esperando permiso del
    // que no lo pinto.
    //
    // Lo que hay que borrar lo decide quien lo pinto. `soltar_conmutador`
    // se llama igual: pedirle a la politica que se suelte no puede
    // depender de que ella misma diga que estaba conmutando.
    if !g.alt_alone && dsk.win.alt_before && dsk.win.switcher_painted {
        dsk.win.focus.soltar_conmutador();
        let (bx, by, ba, bh) = scene::switcher::area(&p, dsk.win.focus.abiertas());
        for fy in 0..bh {
            for fx in 0..ba {
                let (x, y) = (bx + fx, by + fy);
                p.punto(x, y, scene_color(&dsk.run_box, dsk.win.visible, x, y, p.alto));
            }
        }
        dsk.win.switcher_painted = false;
        // Lo que tapaba vuelve a pintarse entero, **de abajo arriba**:
        // es el unico orden que deja la pantalla como estaba. Y quien
        // va arriba lo acaba de decidir el Alt que se solto.
        //
        // * Con tres ventanas esto se escribe como lo que es: pintar
        // TODAS las abiertas, y la que tiene el foco la ULTIMA. La
        // version de dos ventanas enumeraba los casos a mano, y con
        // tres eso son seis ramas que dicen una sola regla.
        // ** LA QUE TIENE EL FOCO, SI SIGUE ABIERTA. Y nada mas.
        //
        // Esto eran seis ramas `abierta && es_para(v)` encadenadas, de las
        // que **como mucho una podia ser cierta** -- el foco es UNO. O sea,
        // seis preguntas por nombre para leer un campo, y la septima lista
        // escrita a mano que habia que ampliar con cada ventana nueva. No
        // romperla no compilaba mal: daba una ventana que **nunca podia
        // estar arriba**, que es el sintoma suave de siempre.
        //
        // El `filter` es la unica parte que no es evidente: el foco puede
        // senalar una ventana ya CERRADA --se cierra sin sacarla de la MRU en
        // algun camino-- y entonces manda Ejecutar, que es lo que la cadena
        // hacia cayendose hasta el `else`.
        let top_now = dsk
            .win
            .focus
            .actual()
            .filter(|&v| dsk.win.abierta(v))
            .unwrap_or(Ventana::Run);
        // ** EL `match` NO LLEVA `_`, Y ESO ES LA MITAD DEL ARREGLO.
        //
        // Llevaba uno --`_ => {}`-- y ademas cada rama iba con guarda, asi
        // que una ventana olvidada aqui no daba error: daba una ventana que
        // no se repintaba. Con `Ventana` y sin `_`, anadir la septima no
        // compila hasta que se diga que hacer con ella, y la condicion de
        // "esta abierta" se pregunta DENTRO de su rama en vez de en la
        // guarda -- que es lo que deja el `match` exhaustivo de verdad.
        let paint_one = |v: Ventana, repintar: &mut bool, sal: &mut scene::output::Output| {
            // "Esta abierta?" se pregunta UNA vez y fuera del `match`. Estaba
            // seis veces dentro, una por rama, y cada una nombraba su bandera
            // a mano.
            if !dsk.win.abierta(v) {
                return;
            }
            match v {
                // Una app se pinta sola: el DIRECTOR pega su superficie en
                // `compose`, no la dibuja. Repintarla desde aqui seria
                // inventarse sus pixeles.
                Ventana::App(_) => {}
                Ventana::Cabina => scene::cabina::paint(&p, &dsk.win.cabina),
                Ventana::Data => scene::data::paint(&p, &dsk.win.data),
                // Las vitales son VISTAS: se repintan cada vez que les
                // toca turno, que es lo que las diferencia de `info`.
                Ventana::Cpu => scene::vitals::paint(&p, &dsk.win.cpu, dsk.tick.loops_per_second),
                Ventana::Mem => scene::vitals::paint(&p, &dsk.win.mem, dsk.tick.loops_per_second),
                Ventana::Sound => scene::sound::paint(
                    &p,
                    &dsk.win.sound,
                    dsk.snd.cap.is_some(),
                    dsk.snd.devices,
                    dsk.snd.volume,
                    dsk.snd.pressed,
                ),
                Ventana::Run => uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, sal, repintar),
            }
        };
        // ** Y LA LISTA ES `Ventana::TODAS`, NO UNA COPIA A MANO.
        //
        // Aqui decia `[Ventana::Run, Ventana::Data, Ventana::Cabina,
        // Ventana::Sound]` -- cuatro de seis. Las vitales no estaban, y lo
        // unico que las salvaba de quedarse tapadas era que se repintan solas
        // cada 15 fotogramas desde `paint.rs`. O sea que el z-order no las
        // ordenaba: flotaban, y a los ~250 ms volvian a aparecer por encima
        // de lo que las hubiera tapado.
        for v in Ventana::TODAS {
            if v != top_now {
                paint_one(v, &mut dsk.tick.repaint_field, &mut dsk.out.grid);
            }
        }
        paint_one(top_now, &mut dsk.tick.repaint_field, &mut dsk.out.grid);
        dsk.win.top_before = top_now;
    }
    dsk.win.alt_before = g.alt_alone;
    if g.combo && !dsk.tick.combo_before {
        dsk.tick.key_during_combo = false;
    }
    if !g.combo && dsk.tick.combo_before && !dsk.tick.key_during_combo {
        dsk.win.visible = !dsk.win.visible;
        if dsk.win.visible {
            // Esconderla y volver a invocarla es cerrarla y abrirla
            // para el foco. Sin esto, Alt+Tab llevaria el teclado a una
            // ventana que no esta en la pantalla: escribirias en algo
            // invisible, que es la peor forma de perder una linea.
            dsk.win.focus.open(Ventana::Run);
            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
            paint_status(&p, &dsk.run_box, "listo", INK_DIM);
        } else {
            dsk.win.focus.close(Ventana::Run);
            erase_box(&p, &dsk.run_box);
        }
    }
    dsk.tick.combo_before = g.combo;

    // -- Teclado --
    //
    // Se atienden TODAS las de la vuelta, no una por fotograma:
    // escribiendo rapido llegan varias entre vuelta y vuelta, y
    // quedarse con una seria perder letras de forma que pareceria un
    // teclado malo. Ya estan recogidas arriba.

}

/// Offer each key to the cascade. Returns the launch the editor asked for, if
/// any -- `_start` is the only place that can serve it.
pub(crate) fn dispatch(
    dsk: &mut Desktop,
    p: &bmo::Pantalla,
    g: &Gathered,
) -> Option<([u8; PATH_MAX], usize)> {
    for &c in &g.keys[..g.nt] {
        if shortcuts::on_key(dsk, p, c, g.alt_alone, g.m) == Key::Taken {
            continue;
        }
        // Cualquier tecla durante el combo lo convierte en AltGr y cancela el
        // toque: el usuario estaba escribiendo, no llamando.
        if g.combo {
            dsk.tick.key_during_combo = true;
        }
        if windows::on_key(dsk, p, c, g.alt_alone) == Key::Taken {
            continue;
        }
        if panels::on_key(dsk, p, c, g.alt_alone, g.ctrl) == Key::Taken {
            continue;
        }
        // -- * DE QUIEN es esta tecla? --
        //
        // Hasta que existio `bmo_input::focus`, TODA tecla se editaba en la
        // linea de Ejecutar aunque la consola de datos estuviera encima:
        // escribias en una ventana tapada, sin verlo.
        //
        // Ninguna abierta --todas escondidas-- tampoco es "Ejecutar por
        // defecto": las teclas se descartan y vuelven al invocarla.
        // ** Y UNA APP QUE NO LEE NO SE QUEDA LAS TECLAS: caen en Ejecutar,
        // que es donde caian antes de que existieran las cajas. Ver
        // `app::muda` para por que esto no se arregla en el foco.
        if !dsk.win.focus.es_para(Ventana::Run) && !app::muda(dsk) {
            continue;
        }
        if let Edit::Launch(target, n) = editor::on_key(dsk, p, c, g.ctrl) {
            return Some((target, n));
        }
    }
    None
}
