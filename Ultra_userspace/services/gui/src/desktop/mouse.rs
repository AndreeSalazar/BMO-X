//! **The pointer**: what a click, a drag and a wheel turn mean.
//!
//! 556 lines that lived inside `_start`, under the `if let Some(e)` that reads
//! the input. They come out whole because they need **five names**: the
//! desktop, the screen, where the pointer is, how far the wheel turned and
//! whether Ctrl is down. Measured before cutting, not guessed -- the keyboard
//! block next door needs ninety, which is why it is still in there.
//!
//! ⚠️ The two `continue` in here used to belong to the `for &c in keys` loop.
//! Outside of it there is no loop to continue, so they are `return`. Rust does
//! not let that be wrong: a `continue` with nothing to continue is an error,
//! which is exactly what makes this cut mechanical instead of risky.

use bmo_userland as bmo;

use super::{Desktop, W_CABINA, W_DATA, W_RUN, W_SOUND};
use crate::scene::calc::paint_calc;
use crate::scene::{self, paint_status, INK_BAD, TASKBAR_H};
use crate::{erase_window, uncover};

/// One frame's worth of pointer.
///
/// Order inside is not free: the wheel is served AFTER working out which
/// window the pointer is over, because before that it always scrolled the
/// output history -- with the kernel console open on top, turning the wheel
/// scrolled a grid that was not even visible.
pub(crate) fn on_pointer(
    dsk: &mut Desktop,
    p: &bmo::Pantalla,
    pos: bmo::Punto,
    wheel: i32,
    ctrl: bool,
) {

    // -- Raton --
    // La rueda, primero: mueve el historial de la salida. Es lo que
    // pidio Eddi --"ver y scrollear"-- y funciona con la rueda o con
    // PgUp/PgDn, porque un teclado siempre hay.
    // * La rueda se atiende MAS ABAJO, cuando ya se sabe sobre que
    // ventana esta el puntero. Antes se atendia aqui y siempre movia el
    // historial de salida: con la consola del kernel abierta y encima,
    // girar la rueda desplazaba una rejilla que ni siquiera se veia.
    //
    // Ver `under_pointer`.
    // -- Los botones de la calculadora --
    let button = pos.botones != 0;

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
        if let Some(i) = dsk.launcher.app_at(&p, pos.x, pos.y) {
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
                    if dsk.field.ni < dsk.field.injected.len() {
                        dsk.field.injected[dsk.field.ni] = b'\n';
                        dsk.field.ni += 1;
                    }
                }
            }
        }
    }

    if dsk.calc.visible && button && !dsk.tick.button_before && !dsk.calc.waiting {
        if let Some(t) = dsk.calc_pad.key_at(pos.x, pos.y) {
            match t {
                b'C' => dsk.calc.clear(),
                b'+' => dsk.calc.operator(1),
                b'-' => dsk.calc.operator(2),
                b'*' => dsk.calc.operator(3),
                b'/' => dsk.calc.operator(4),
                b'=' => {
                    if dsk.calc.op != 0 && dsk.calc.saved_n > 0 && dsk.calc.n > 0 {
                        // Lanzar el MOTOR y darle los tres datos por su
                        // consola. Aqui es donde la cara deja de saber
                        // de aritmetica y empieza a saber COBOL.
                        let cap = dsk.out.console.as_ref().map(|c| c.cap).unwrap_or(0);
                        if bmo::ejecutar_en(b"cobol/calcgui.bex", cap).is_ok() {
                            if let Some(cc) = dsk.out.console.as_ref() {
                                cc.write(&dsk.calc.saved_path[..dsk.calc.saved_n]);
                                cc.write(b"\n");
                                cc.write(&[b'0' + dsk.calc.op]);
                                cc.write(b"\n");
                                cc.write(&dsk.calc.input[..dsk.calc.n]);
                                cc.write(b"\n");
                            }
                            dsk.calc.waiting = true;
                            dsk.resp_n = 0;
                        } else {
                            paint_status(&p, &dsk.run_box, "falta cobol/calcgui.bex", INK_BAD);
                        }
                    }
                }
                d => dsk.calc.feed(d),
            }
            paint_calc(&p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
        }
    }
    // -- El raton tambien manda en el foco --
    //
    // Sin esto, dos de los tres modos son decoracion: `click-to-focus`
    // no existiria y `focus-follows-mouse` no tendria quien le dijera
    // por donde va el puntero.
    //
    // * El orden de estas dos preguntas ES el Z-order: Datos se pinta
    // ENCIMA de Ejecutar, asi que se pregunta primero, y un clic en la
    // zona compartida es de la de arriba. `bmo_input::focus` no sabe que
    // ventana tapa a cual y no tiene por que: eso lo sabe el que pinta.
    // * Con TRES ventanas, el orden de las preguntas deja de caber en
    // un `if/else` escrito a mano por pares. Se pregunta primero por la
    // que esta ARRIBA --sea cual sea-- y despues por las demas: un clic
    // en la zona compartida es siempre de la de encima, y eso es una
    // regla, no una lista de casos.
    let at = |v: u8| match v {
        W_DATA => dsk.win.data_open && dsk.win.data.contains(pos.x, pos.y),
        W_CABINA => dsk.win.cabina_open && dsk.win.cabina.chrome.contains(pos.x, pos.y),
        W_SOUND => dsk.win.sound_open && dsk.win.sound.chrome.contains(pos.x, pos.y),
        _ => dsk.win.visible && dsk.run_box.contains(pos.x, pos.y),
    };
    let under_pointer = if at(dsk.win.top_before) {
        Some(dsk.win.top_before)
    } else {
        [W_SOUND, W_CABINA, W_DATA, W_RUN]
            .into_iter()
            .find(|&v| v != dsk.win.top_before && at(v))
    };
    // -- * LA RUEDA VA A LA VENTANA QUE HAY DEBAJO --
    //
    // Es lo que hace cualquier sistema y lo que la mano espera sin
    // pensarlo: se gira donde se mira. Antes iba SIEMPRE al historial
    // de salida, asi que con la consola del kernel delante la rueda
    // movia una rejilla tapada -- el gesto no hacia nada visible y
    // parecia que la rueda no funcionaba.
    //
    // Sin ventana debajo no se hace nada, y eso tambien es una
    // decision: mandar el giro a la ventana con el foco cuando el
    // puntero esta en el escritorio mueve cosas que no se estan
    // mirando.
    if wheel != 0 {
        match under_pointer {
            Some(W_CABINA) => {
                // Positivo es hacia arriba, y en un log "arriba" es
                // hacia ATRAS en el tiempo: el desplazamiento cuenta
                // lineas hacia el pasado, asi que suma.
                let any = bmo::cabina_disponibles();
                let step = (wheel * 3) as i64;
                let new = dsk.win.cabina.from as i64 + step;
                dsk.win.cabina.from =
                    new.clamp(0, any.saturating_sub(1) as i64) as u64;
                scene::cabina::paint(&p, &dsk.win.cabina);
            }
            Some(W_RUN) => {
                // Tres filas por muesca: una sola se queda corta y una
                // pagina entera se pasa. Es el paso de un terminal.
                dsk.out.grid.scroll_view(wheel * 3);
            }
            // La rueda sobre el arbol de nodos mueve la seleccion. En la
            // pestana de numeros no hay nada que desplazar: cabe entera.
            Some(W_DATA) if dsk.win.data.view == scene::data::View::Nodes => {
                // Girar hacia arriba sube por la lista: `wheel` positivo
                // es hacia arriba y la seleccion de arriba es la menor.
                let how_many = bmo::estratos::hijos() as usize;
                dsk.win.data.move_sel(-wheel, how_many);
                scene::data::paint(&p, &dsk.win.data);
            }
            _ => {}
        }
    }

    // -- El realce de la calculadora --
    //
    // Solo cuando CAMBIA la tecla senalada, y solo si la calculadora se
    // ve y no esta tapada. Al salir de ella el realce se apaga, que es
    // la mitad que se olvida siempre: un boton que se queda encendido
    // cuando ya no lo senalas miente sobre donde esta el raton.
    let hover_now = if dsk.calc.visible && dsk.win.top_before == W_RUN {
        dsk.calc_pad.key_at(pos.x, pos.y)
    } else {
        None
    };
    if hover_now != dsk.tick.calc_hover {
        dsk.tick.calc_hover = hover_now;
        if dsk.calc.visible {
            paint_calc(&p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
        }
    }

    if let Some(v) = under_pointer {
        // Pasar por encima: solo hace algo en modo `Puntero`, y la
        // guarda esta DENTRO de la politica -- aqui solo se cuenta lo
        // que pasa, no se decide lo que significa.
        if pos.x != dsk.tick.ax || pos.y != dsk.tick.ay {
            dsk.win.focus.puntero_en(v);
        }
        // Un clic lo pide en CUALQUIER modo, incluido `Fijo`: lo que
        // ese modo impide es que una ventana se lo tome sin que nadie
        // se lo pida, no que tu se lo des.
        if button && !dsk.tick.button_before {
            dsk.win.focus.clic_en(v);
        }
    }

    // -- * EL RATON SOBRE LA VENTANA DE DATOS --
    //
    // Tres gestos que comparten estructura: los BOTONES de la barra,
    // ARRASTRAR por el asa y ESTIRAR por la esquina. Quien decide cual
    // es el marco, no esto: aqui solo se le cuenta lo que paso.
    if dsk.win.data_open && !dsk.win.data.chrome.minimized {
        use scene::chrome::Button;

        // El realce de los botones. Solo cuando CAMBIA -- repintarlo
        // cada fotograma serian 1.700 pixeles de memoria de video sin
        // cache para dejarlo igual, y ademas pisaria el cursor.
        let hover_now = dsk.win.data.chrome.button_at(pos.x, pos.y);
        if hover_now != dsk.win.data.chrome.hover {
            dsk.win.data.chrome.hover = hover_now;
            scene::data::paint(&p, &dsk.win.data);
            dsk.win.top_before = W_DATA;
        }

        if button && !dsk.tick.button_before {
            // Un boton se dispara al PULSAR y no al soltar. Es lo que
            // hace todo el mundo, y con `close` importa: soltar fuera
            // para arrepentirse no funciona en ningun escritorio, asi
            // que fingirlo aqui seria inventarse una costumbre.
            match dsk.win.data.chrome.button_at(pos.x, pos.y) {
                Some(Button::Close) => {
                    dsk.win.data_open = false;
                    dsk.win.focus.close(W_DATA);
                    erase_window(
                        &p, &dsk.run_box, dsk.win.data.x(), dsk.win.data.y(),
                        dsk.win.data.width(), dsk.win.data.height(), dsk.win.visible,
                    );
                    dsk.win.top_before = W_RUN;
                    uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                }
                Some(Button::Minimize) => {
                    // Minimizar NO es cerrar: la ventana sigue abierta
                    // y conserva su sitio, su tamano y lo que estuviera
                    // mirando. Se va a su ficha de la barra.
                    let (vx, vy, va, vl) = (
                        dsk.win.data.x(), dsk.win.data.y(),
                        dsk.win.data.width(), dsk.win.data.height(),
                    );
                    dsk.win.data.chrome.minimized = true;
                    dsk.win.focus.close(W_DATA);
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    dsk.win.top_before = W_RUN;
                    uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    dsk.win.taskbar_dirty = true;
                }
                Some(Button::Maximize) => {
                    let (vx, vy, va, vl) = dsk.win.data.chrome.toggle_maximized(&p);
                    // Al restaurar, el hueco que deja hay que
                    // devolverselo al escritorio; al maximizar no sobra
                    // nada, pero borrar el rectangulo viejo entero
                    // cubre los dos casos con una sola regla.
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    dsk.win.data.relayout();
                    scene::data::paint(&p, &dsk.win.data);
                    dsk.win.top_before = W_DATA;
                }
                None => {
                    // -- * CLIC DENTRO DEL GRAFO --
                    //
                    // El gesto que faltaba: hasta ahora el raton solo
                    // servia para mover la ventana, y una ventana llena
                    // de cajas en la que no se puede pulsar ninguna es
                    // una ventana que parece interactiva y no lo es.
                    let how_many = bmo::estratos::hijos() as usize;
                    match dsk.win.data.box_at(pos.x, pos.y, how_many) {
                        // La caja del PADRE: sube un nivel. Es el gesto
                        // que la mano busca sola cuando ya has bajado.
                        Some(i) if i == usize::MAX => {
                            if bmo::estratos::subir() {
                                dsk.win.data.to_top();
                                dsk.win.data.verified = None;
                                scene::data::paint(&p, &dsk.win.data);
                                dsk.win.top_before = W_DATA;
                            }
                        }
                        Some(i) => {
                            dsk.win.data.sel = i;
                            // El resultado de una verificacion es de UN
                            // archivo: al cambiar de caja se borra. Si
                            // no, un `CUADRA` viejo se quedaria debajo
                            // del nombre de otro.
                            dsk.win.data.verified = None;
                            // * Ctrl+clic BAJA de una vez, sin tener que
                            // senalar y pulsar ENTRAR. El clic a secas
                            // solo senala, porque senalar tiene que
                            // poder hacerse sin miedo a moverte de sitio.
                            if ctrl && bmo::estratos::entrar(i as u64) {
                                dsk.win.data.to_top();
                            }
                            scene::data::paint(&p, &dsk.win.data);
                            dsk.win.top_before = W_DATA;
                        }
                        None => {
                            dsk.win.data.chrome.grab(pos.x, pos.y);
                        }
                    }
                }
            }
        }

        if !button && dsk.win.data.chrome.grabbed() {
            dsk.win.data.chrome.release();
        } else if button && dsk.win.data.chrome.grabbed() {
            // El sitio VIEJO hay que borrarlo antes de mover. Si no, la
            // ventana deja un rastro de copias de si misma: aqui no hay
            // recorte ni compositor que repinte lo de debajo solo.
            //
            // Al ESTIRAR pasa lo mismo pero solo al encoger; borrar el
            // rectangulo viejo entero cubre los dos casos con una regla
            // en vez de con dos.
            let (vx, vy, va, vl) = (
                dsk.win.data.x(), dsk.win.data.y(),
                dsk.win.data.width(), dsk.win.data.height(),
            );
            if dsk.win.data.chrome.follow_pointer(&p, pos.x, pos.y) {
                erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                dsk.win.data.relayout();
                scene::data::paint(&p, &dsk.win.data);
                dsk.win.top_before = W_DATA;
            }
        }
    }

    // -- ** EL ARRASTRE DE LAS OTRAS DOS VENTANAS --
    //
    // CABINA y Sonido nacieron con `Chrome` --que trae `grab`,
    // `follow_pointer` y `release`-- y **nadie las llamaba**. El
    // resultado en el Ryzen: dos ventanas con barra de titulo, con sus
    // tres botones pintados, y clavadas en el sitio.
    //
    // Es el patron 24 de `BITACORA.md` calcado: la politica escrita y
    // sin lector. Alli fue `es_para` del foco, que existia entera con
    // tests y no se llamaba ni una vez; aqui es el arrastre. Dar el
    // mecanismo NO es cablearlo, y la unica forma de notar la
    // diferencia es ejecutandolo -- por eso salio en metal y no antes.
    //
    // * Y el cableado nacio ANIDADO dentro del `if data_open`, que
    // es el mismo patron una vuelta mas: el lector existia y solo se le
    // daba corriente cuando ESTRATOS estaba abierta y sin minimizar.
    // Con Datos cerrada, las dos ventanas volvian a estar clavadas. El
    // arrastre de una ventana no depende de otra ventana, asi que va
    // aqui, al nivel de las demas.
    if dsk.win.cabina_open && !dsk.win.cabina.chrome.minimized {
        if button && !dsk.win.cabina.chrome.grabbed() && dsk.win.focus.es_para(W_CABINA)
            && dsk.win.cabina.chrome.on_the_grip(pos.x, pos.y)
        {
            dsk.win.cabina.chrome.grab(pos.x, pos.y);
        } else if !button && dsk.win.cabina.chrome.grabbed() {
            dsk.win.cabina.chrome.release();
        } else if button && dsk.win.cabina.chrome.grabbed() {
            // El sitio VIEJO se borra antes de mover: aqui no hay
            // compositor que repinte lo de debajo, asi que sin esto
            // la ventana deja un rastro de copias de si misma.
            let (vx, vy, va, vl) = (
                dsk.win.cabina.chrome.x, dsk.win.cabina.chrome.y,
                dsk.win.cabina.chrome.width, dsk.win.cabina.chrome.height,
            );
            if dsk.win.cabina.chrome.follow_pointer(&p, pos.x, pos.y) {
                erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                scene::cabina::paint(&p, &dsk.win.cabina);
                dsk.win.top_before = W_CABINA;
            }
        }
    }

    if dsk.win.sound_open && !dsk.win.sound.chrome.minimized {
        if button && !dsk.win.sound.chrome.grabbed() && dsk.win.focus.es_para(W_SOUND)
            && dsk.win.sound.chrome.on_the_grip(pos.x, pos.y)
        {
            dsk.win.sound.chrome.grab(pos.x, pos.y);
        } else if !button && dsk.win.sound.chrome.grabbed() {
            dsk.win.sound.chrome.release();
        } else if button && dsk.win.sound.chrome.grabbed() {
            let (vx, vy, va, vl) = (
                dsk.win.sound.chrome.x, dsk.win.sound.chrome.y,
                dsk.win.sound.chrome.width, dsk.win.sound.chrome.height,
            );
            if dsk.win.sound.chrome.follow_pointer(&p, pos.x, pos.y) {
                erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                scene::sound::paint(
                    &p, &dsk.win.sound, dsk.snd.cap.is_some(),
                    dsk.snd.devices, dsk.snd.volume, dsk.snd.pressed,
                );
                dsk.win.top_before = W_SOUND;
            }
        }
    }

    // -- ** EL RATON SOBRE UNA CAJA DE APP --
    //
    // Los mismos tres gestos que las ventanas del sistema, y por eso son
    // ocho lineas: el marco ya sabe hacerlos. **Este es el cobro del
    // `chrome.rs`** -- se escribio para que la cuarta ventana saliera
    // gratis, y la cuarta ventana resulta ser un programa entero.
    //
    // Va DESPUES de las ventanas del sistema y antes de las fichas: una
    // app en su caja esta por delante de ellas, asi que su clic manda.
    {
        use scene::chrome::Button;

        if button && !dsk.tick.button_before {
            if let Some(i) = dsk.table.at(pos.x, pos.y) {
                // El realce se pone aunque no se pulse: si no, los tres
                // botones de una app serian los unicos del escritorio
                // que no se encienden al pasar por encima.
                let gesture = dsk.table.get_mut(i).and_then(|s| s.chrome.button_at(pos.x, pos.y));
                match gesture {
                    // ** CERRAR NO MATA A LA APP: le quita la caja.
                    //
                    // Matar un proceso ajeno por ser el DIRECTOR seria
                    // `root` con otro nombre, en el sistema cuya primera
                    // clausula dice que la autoridad no se hereda. Matar
                    // se hara con el handle que devolvio LANZARLA --
                    // paso 3 del plan-- y no desde aqui.
                    Some(Button::Close) => {
                        if let Some((vx, vy, va, vl)) = dsk.table.close(i) {
                            erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                            uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                            for s in dsk.table.iter_mut() {
                                s.repaint_all();
                            }
                        }
                    }
                    Some(Button::Minimize) => {
                        if let Some(s) = dsk.table.get_mut(i) {
                            let (vx, vy, va, vl) =
                                (s.chrome.x, s.chrome.y, s.chrome.width, s.chrome.height);
                            s.chrome.minimized = true;
                            erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                            uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                        }
                    }
                    // ** PANTALLA COMPLETA = QUE NO SE DIBUJE EL BORDE.
                    //
                    // Y aqui todavia no: maximizar da el hueco entero
                    // bajo la barra, que es lo que hacen las demas. Lo
                    // que NO pasa --ni pasara-- es entregarle el
                    // aparato: se sigue componiendo, asi que Alt+Tab
                    // sigue y `Ctrl+Alt+ESC` sigue. Un juego colgado se
                    // cierra con el teclado y no con el boton de reset.
                    Some(Button::Maximize) => {
                        if let Some(s) = dsk.table.get_mut(i) {
                            let (vx, vy, va, vl) = s.chrome.toggle_maximized(&p);
                            erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                            uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                            s.repaint_all();
                        }
                    }
                    None => {
                        if let Some(s) = dsk.table.get_mut(i) {
                            s.chrome.grab(pos.x, pos.y);
                        }
                    }
                }
            }
        }

        // Arrastrar y estirar. El sitio VIEJO se borra antes de mover:
        // aqui no hay nadie que repinte lo de debajo, asi que sin esto
        // la ventana deja un rastro de copias de si misma.
        for i in 0..scene::surface::MAX {
            let Some(s) = dsk.table.get_mut(i) else { continue };
            if !s.chrome.grabbed() {
                return;
            }
            if !button {
                s.chrome.release();
                return;
            }
            let (vx, vy, va, vl) = (s.chrome.x, s.chrome.y, s.chrome.width, s.chrome.height);
            if s.chrome.follow_pointer(&p, pos.x, pos.y) {
                s.repaint_all();
                erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
            }
        }
    }

    // -- Clic en una FICHA de la barra: traer esa ventana --
    //
    // Es la mitad que hace que minimizar signifique algo. Sin esto, el
    // boton de minimizar seria uno de "desaparece para siempre".
    // * Una ficha hace SIEMPRE lo mismo: **trae su ventana y le da el
    // foco**, este minimizada, escondida o simplemente detras.
    //
    // La primera version solo actuaba `si estaba minimized` o `si
    // estaba escondida`, y por eso pulsar la ficha de una ventana que
    // ya se veia no hacia nada. En el Ryzen eso se lee como *"la barra
    // se olvida de mis clics"*, y con razon: un control que a veces
    // responde y a veces no es peor que uno que no esta.
    if button && !dsk.tick.button_before && pos.y < TASKBAR_H {
        if let Some(i) = scene::chip_at(pos.x, pos.y, 2) {
            if i == 1 && dsk.win.data_open {
                // Estaba minimizada o no, da igual: acaba visible,
                // encajada, con el foco y delante.
                dsk.win.data.chrome.minimized = false;
                dsk.win.data.chrome.fit(&p);
                dsk.win.focus.open(W_DATA);
                dsk.win.focus.clic_en(W_DATA);
                dsk.win.data.relayout();
                scene::data::paint(&p, &dsk.win.data);
                dsk.win.top_before = W_DATA;
                dsk.win.taskbar_dirty = true;
            } else if i == 0 {
                if !dsk.win.visible {
                    dsk.win.visible = true;
                }
                dsk.win.focus.open(W_RUN);
                dsk.win.focus.clic_en(W_RUN);
                uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                dsk.win.top_before = W_RUN;
                dsk.win.taskbar_dirty = true;
            }
        }
    }
    dsk.tick.button_before = button;

    // -- * El foco arrastra el Z-order --
    //
    // Levantar una ventana no da el teclado --eso es mezclar dos cosas y
    // es el error clasico de un gestor de ventanas--, pero **al reves si
    // vale**: la que tiene el teclado tiene que verse. Aqui no hay
    // recorte, asi que "verse" es pintarse la ultima.
    //
    // Sin esto, Alt+Tab a Ejecutar con Datos delante dejaria el teclado
    // en una linea tapada: escribirias sin ver nada. Es exactamente el
    // fallo que se acaba de arreglar, del reves.
    let top = if dsk.win.cabina_open && dsk.win.focus.es_para(W_CABINA) {
        W_CABINA
    } else if dsk.win.data_open && dsk.win.focus.es_para(W_DATA) {
        W_DATA
    } else {
        W_RUN
    };
    if top != dsk.win.top_before {
        match top {
            W_CABINA => scene::cabina::paint(&p, &dsk.win.cabina),
            W_DATA => scene::data::paint(&p, &dsk.win.data),
            // Sin guarda de `visible`: `uncover` ya no hace nada si
            // la caja esta escondida, y una guarda repetida es una que
            // puede quedarse desincronizada de la funcion.
            _ => uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field),
        }
        dsk.win.top_before = top;
    }

    // El cursor ya no se borra aqui: se pone al final del fotograma y
    // se quita al principio del siguiente, con lo que habia debajo
    // guardado. Aqui solo se apunta donde esta.
    dsk.tick.ax = pos.x;
    dsk.tick.ay = pos.y;

    // * Aqui se pintaban el PULSOMETRO y el testigo de botones. Fuera
    // el 2026-08-04, con los seis parches de medida: contestaban
    // "llegan informes del raton?" y esa pregunta la contesta ya el
    // propio puntero moviendose. Ver la nota del escritorio.
}
