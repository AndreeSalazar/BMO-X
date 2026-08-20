//! **The pointer**: what a click, a drag and a wheel turn mean.
//!
//! 556 lines that lived inside `_start`, under the `if let Some(e)` that reads
//! the input. They come out whole because they need **five names**: the
//! desktop, the screen, where the pointer is, how far the wheel turned and
//! whether Ctrl is down. Measured before cutting, not guessed -- the keyboard
//! block next door needs ninety, which is why it is still in there.
//!
//! [!] The two `continue` in here used to belong to the `for &c in keys` loop.
//! Outside of it there is no loop to continue, so they are `return`. Rust does
//! not let that be wrong: a `continue` with nothing to continue is an error,
//! which is exactly what makes this cut mechanical instead of risky.

use bmo_userland as bmo;

use super::{calc, Desktop, Ventana};
use crate::scene::calc::paint_calc;
use crate::scene::{self, TASKBAR_H};
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
    // ** LOS DOS BOTONES ERAN EL MISMO, Y ESO ERA UN FALLO.
    //
    // Esto era `pos.botones != 0`, o sea que **pulsar con el derecho hacia lo
    // mismo que con el izquierdo**: el clic derecho sobre un icono lanzaba la
    // app. No daba error y por eso llevaba ahi desde que hay raton.
    //
    // La mascara viene del informe HID tal cual (`uhid::raton`): bit 0 el
    // izquierdo, bit 1 el DERECHO. Hay prueba que lo fija --
    // `mover_y_pulsar_a_la_vez_son_dos_eventos` espera un `2`-- asi que el
    // numero no es una suposicion de este lado.
    const IZQUIERDO: u8 = 0b001;
    const DERECHO: u8 = 0b010;
    let button = pos.botones & IZQUIERDO != 0;
    let derecho = pos.botones & DERECHO != 0;

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
            // ** Lo que hacia esta tecla estaba escrito AQUI, y ahora vive en
            // `desktop::calc`. Desde que el teclado tambien pulsa, tenerlo en
            // los dos sitios seria la misma cuenta escrita dos veces -- que es
            // exactamente lo que MAQUETA acaba de borrar del pintado, y no se
            // arregla en un aparato para reinventarlo en el de al lado.
            calc::pulsar(dsk, p, t);
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
    // ** SIN `_`, Y ESO CAMBIA LO QUE HACE.
    //
    // La ultima rama era `_ => dsk.win.visible && run_box.contains(...)`, o
    // sea: **cualquier id que este `match` no conociera se contestaba como si
    // fuera Ejecutar**. Y habia dos que no conocia --las vitales-- porque el
    // raton no las tiene dadas de alta.
    //
    // No era teorico. `top_before` SI puede valer `Cpu` o `Mem` --lo pone el
    // repintado de `keys/mod.rs`-- y la primera pregunta de aqui es
    // `at(top_before)`. Con la ventana de CPU arriba y el puntero sobre la
    // terminal, `at(Cpu)` contestaba "si" por la rama comodin, el clic se
    // apuntaba a la ventana de CPU y **el teclado no volvia a Ejecutar al
    // hacer clic en Ejecutar**. Un comodin en un `match` de identidades no es
    // un caso por defecto: es un caso equivocado con buena letra.
    let at = |v: Ventana| match v {
        Ventana::Data => dsk.win.data_open && dsk.win.data.contains(pos.x, pos.y),
        Ventana::Cabina => dsk.win.cabina_open && dsk.win.cabina.chrome.contains(pos.x, pos.y),
        Ventana::Sound => dsk.win.sound_open && dsk.win.sound.chrome.contains(pos.x, pos.y),
        // [!] LAS VITALES NO ESTAN EN EL RATON, y esto lo dice en voz alta en
        // vez de esconderlo detras de un comodin. No se arrastran, sus botones
        // no responden y un clic encima se lo lleva la ventana de DEBAJO --que
        // ademas se queda el foco--. Su propio pie anuncia "arrastra el
        // titulo", asi que hoy prometen algo que no hacen: es el pecado del
        // 2026-08-09 otra vez, y es un trabajo aparte de este.
        Ventana::Cpu | Ventana::Mem => false,
        Ventana::Run => dsk.win.visible && dsk.run_box.contains(pos.x, pos.y),
        // ** LAS APPS NO SE BUSCAN AQUI: este `match` recorre las ventanas
        // FIJAS del escritorio, y una superficie no es una de ellas -- vive
        // en `dsk.table`, que tiene su propio `at()` unas lineas mas abajo y
        // que ademas necesita saber en QUE pixel suyo cayo el clic.
        //
        // Contestar `false` aqui no es esconderlas: es decir que la pregunta
        // "esta el puntero sobre esta ventana fija?" no se le hace a una app.
        Ventana::App(_) => false,
    };
    // De arriba abajo, que es `TODAS` del reves: la de encima se lleva el clic
    // de la zona compartida. Antes era otra lista escrita a mano.
    let under_pointer = if at(dsk.win.top_before) {
        Some(dsk.win.top_before)
    } else {
        Ventana::TODAS
            .into_iter()
            .rev()
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
            Some(Ventana::Cabina) => {
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
            Some(Ventana::Run) => {
                // Tres filas por muesca: una sola se queda corta y una
                // pagina entera se pasa. Es el paso de un terminal.
                dsk.out.grid.scroll_view(wheel * 3);
            }
            // La rueda sobre el arbol de nodos mueve la seleccion. En la
            // pestana de numeros no hay nada que desplazar: cabe entera.
            Some(Ventana::Data) if dsk.win.data.view == scene::data::View::Obra => {
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
    let hover_now = if dsk.calc.visible && dsk.win.top_before == Ventana::Run {
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

    // == ** EL CLIC DERECHO: el menu de lo que se puede hacer =============
    //
    // Va ANTES del bloque de la ventana de Datos porque el menu, mientras esta
    // abierto, **se queda con el clic**: es lo que hay encima de todo, y lo que
    // hay encima manda. Sin esa regla, pulsar `borrar` seleccionaria ademas la
    // fila que hay debajo del menu.
    if dsk.win.data_open && !dsk.win.data.chrome.minimized && dsk.win.data.view == scene::data::View::Obra {
        // El menu abierto se queda con el clic izquierdo: o eliges una entrada,
        // o lo cierras pulsando fuera. Las dos cosas son "ya no hay menu".
        if dsk.win.data.menu.visible && button && !dsk.tick.button_before {
            let elegida = dsk.win.data.menu.entrada_en(pos.x, pos.y);
            let sobre = dsk.win.data.menu.sobre;
            dsk.win.data.menu.cerrar();
            if let Some(e) = elegida {
                usar_entrada(dsk, &p, e, sobre);
            }
            scene::data::paint(&p, &dsk.win.data);
            dsk.win.top_before = Ventana::Data;
            dsk.tick.button_before = pos.botones != 0;
            dsk.tick.ax = pos.x;
            dsk.tick.ay = pos.y;
            return;
        }
        // Y el derecho lo ABRE, sobre lo que haya debajo.
        if derecho && !dsk.tick.derecho_before {
            let sobre = match dsk.win.data.fila_rejilla_en(pos.x, pos.y) {
                Some(i) => {
                    // Senalar y abrir el menu son la misma pulsacion: si no, el
                    // menu hablaria de una fila y el realce estaria en otra.
                    dsk.win.data.sel = i;
                    dsk.win.data.verified = None;
                    scene::menu::Sobre::Hijo(i)
                }
                None => scene::menu::Sobre::Aqui,
            };
            dsk.win.data.menu.abrir(pos.x, pos.y, sobre);
            scene::data::paint(&p, &dsk.win.data);
            dsk.win.top_before = Ventana::Data;
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
        //
        // [!] **Y ese comentario describia lo que aqui NO se hacia.** Decia
        // 1.700 pixeles y llamaba a `data::paint`, que repinta la ventana
        // ENTERA: marco, pestanas, arbol, rejilla con iconos e historial. Pasar
        // el puntero por encima de los tres botones cambia el realce cinco o
        // seis veces --entrar y salir de cada uno-- asi que un gesto de medio
        // segundo eran seis repintados completos del panel. Es exactamente lo
        // que se siente como *"al pasar por cerrar va lento"*.
        //
        // La caja de Ejecutar ya lo hacia bien, con `paint_buttons`, treinta
        // lineas mas abajo. Dos ventanas con el mismo cromo y solo una tenia el
        // arreglo -- que es la regla que este arbol lleva repitiendo toda la
        // semana en otros sitios.
        let hover_now = dsk.win.data.chrome.button_at(pos.x, pos.y);
        if hover_now != dsk.win.data.chrome.hover {
            dsk.win.data.chrome.hover = hover_now;
            dsk.win.data.chrome.paint_buttons(&p, scene::data::DATA_TITLE_BG);
        }

        if button && !dsk.tick.button_before {
            // Un boton se dispara al PULSAR y no al soltar. Es lo que
            // hace todo el mundo, y con `close` importa: soltar fuera
            // para arrepentirse no funciona en ningun escritorio, asi
            // que fingirlo aqui seria inventarse una costumbre.
            match dsk.win.data.chrome.button_at(pos.x, pos.y) {
                Some(Button::Close) => {
                    dsk.win.data_open = false;
                    dsk.win.focus.close(Ventana::Data);
                    erase_window(
                        &p, &dsk.run_box, dsk.win.data.x(), dsk.win.data.y(),
                        dsk.win.data.width(), dsk.win.data.height(), dsk.win.visible,
                    );
                    dsk.win.top_before = Ventana::Run;
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
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
                    dsk.win.focus.close(Ventana::Data);
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    dsk.win.top_before = Ventana::Run;
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    dsk.win.taskbar_dirty = true;
                }
                Some(Button::Maximize) => {
                    let (vx, vy, va, vl) = dsk.win.data.chrome.toggle_maximized(&p);
                    // Al restaurar, el hueco que deja hay que
                    // devolverselo al escritorio; al maximizar no sobra
                    // nada, pero borrar el rectangulo viejo entero
                    // cubre los dos casos con una sola regla.
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    dsk.win.data.relayout();
                    scene::data::paint(&p, &dsk.win.data);
                    dsk.win.top_before = Ventana::Data;
                }
                None => {
                    // [!] `servido` y NO un `return`. Salir de `on_pointer`
                    // aqui se saltaria `dsk.tick.button_before = button` del
                    // final, y entonces el fotograma siguiente volveria a ver
                    // "boton pulsado y antes no": el clic se repetiria mientras
                    // mantienes pulsado, y mantener el raton sobre una fila del
                    // arbol te iria metiendo carpeta adentro sola.
                    let mut servido = false;
                    // -- ** CLIC EN EL PANEL DE ARBOL --
                    //
                    // Va ANTES que el del grafo porque los dos miran el mismo
                    // clic y solo uno puede quedarselo. El arbol tiene su
                    // rectangulo, asi que "cae dentro" es una pregunta exacta y
                    // no un orden de prioridad disfrazado.
                    //
                    // Un clic aqui SALTA: de `/a/b/c` a `/a/d` en un gesto, que
                    // es justo lo que un panel de arbol compra sobre la miga de
                    // pan.
                    let z = scene::zonas::Zonas::repartir(&dsk.win.data.chrome, dsk.win.data.consola.abierta);
                    if dsk.win.data.view == scene::data::View::Obra {
                        if let Some(fila) =
                            scene::arbol::fila_en(&z.arbol, dsk.win.data.arbol_from, pos.x, pos.y)
                        {
                            let movido = match fila {
                                // La raiz: se sube hasta arriba. Subiendo y no
                                // con `a_la_raiz`, que releeria el directorio
                                // entero para acabar donde subir deja gratis.
                                None => {
                                    scene::arbol::a_la_raiz_subiendo();
                                    true
                                }
                                Some(f) => scene::arbol::saltar_a(f.nivel, f.indice),
                            };
                            if movido {
                                dsk.win.data.to_top();
                                dsk.win.data.verified = None;
                                scene::data::paint(&p, &dsk.win.data);
                                dsk.win.top_before = Ventana::Data;
                            }
                            servido = true;
                        }
                    }
                    // -- * CLIC DENTRO DEL GRAFO --
                    //
                    // El gesto que faltaba: hasta ahora el raton solo
                    // servia para mover la ventana, y una ventana llena
                    // de cajas en la que no se puede pulsar ninguna es
                    // una ventana que parece interactiva y no lo es.
                    let how_many = bmo::estratos::hijos() as usize;
                    match if servido { None } else { dsk.win.data.box_at(pos.x, pos.y, how_many) } {
                        // La caja del PADRE: sube un nivel. Es el gesto
                        // que la mano busca sola cuando ya has bajado.
                        Some(i) if i == usize::MAX => {
                            if bmo::estratos::subir() {
                                dsk.win.data.to_top();
                                dsk.win.data.verified = None;
                                scene::data::paint(&p, &dsk.win.data);
                                dsk.win.top_before = Ventana::Data;
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
                            dsk.win.top_before = Ventana::Data;
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
            // Al ESTIRAR pasa lo mismo pero solo al encoger. Se borra la
            // RESTA del viejo menos el nuevo, que cubre los dos casos con una
            // regla y ademas no toca lo que la ventana sigue tapando.
            let (vx, vy, va, vl) = (
                dsk.win.data.x(), dsk.win.data.y(),
                dsk.win.data.width(), dsk.win.data.height(),
            );
            if dsk.win.data.chrome.follow_pointer(&p, pos.x, pos.y) {
                scene::erase_moved(
                    &p,
                    &dsk.run_box,
                    (vx, vy, va, vl),
                    (
                        dsk.win.data.x(),
                        dsk.win.data.y(),
                        dsk.win.data.width(),
                        dsk.win.data.height(),
                    ),
                    dsk.win.visible,
                );
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                dsk.win.data.relayout();
                scene::data::paint(&p, &dsk.win.data);
                dsk.win.top_before = Ventana::Data;
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
        if button && !dsk.win.cabina.chrome.grabbed() && dsk.win.focus.es_para(Ventana::Cabina)
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
                scene::erase_moved(
                    &p,
                    &dsk.run_box,
                    (vx, vy, va, vl),
                    (
                        dsk.win.cabina.chrome.x,
                        dsk.win.cabina.chrome.y,
                        dsk.win.cabina.chrome.width,
                        dsk.win.cabina.chrome.height,
                    ),
                    dsk.win.visible,
                );
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                scene::cabina::paint(&p, &dsk.win.cabina);
                dsk.win.top_before = Ventana::Cabina;
            }
        }
    }

    // -- ** EL RATON SOBRE LA TERMINAL --
    //
    // Va ANTES que las demas ventanas a proposito: la terminal esta
    // DEBAJO de todas, asi que si CABINA o Datos estan encima y el
    // puntero cae en la zona compartida, la de arriba tiene que ganar
    // -- y gana porque su bloque se evalua despues y el `grab` de esta
    // no se dispara si ya hay otro agarrado por el suyo.
    //
    // ** Y este bloque existe porque el marco NO se cablea solo. Este
    // mismo fichero ya cazo el fallo una vez, con estas palabras:
    // *"CABINA y Sonido nacieron con `Chrome` y nadie las llamaba. El
    // resultado en el Ryzen: dos ventanas con barra de titulo, con sus
    // tres botones pintados, y clavadas en el sitio."* Pintar los
    // botones no es tenerlos.
    if dsk.win.visible {
        use scene::chrome::Button;

        let hover_now = dsk.run_box.chrome.button_at(pos.x, pos.y);
        if hover_now != dsk.run_box.chrome.hover {
            dsk.run_box.chrome.hover = hover_now;
            dsk.run_box.chrome.paint_buttons(&p, scene::BOX_TITLE);
        }

        if button && !dsk.tick.button_before {
            match dsk.run_box.chrome.button_at(pos.x, pos.y) {
                // No hay aspa: `sin_cerrar()`. Ver la cabecera de
                // `Chrome::closable` -- cerrar la unica ventana donde se
                // escriben ordenes dejaria la maquina sin linea de
                // ordenes, y al shell de Ring 0 no se vuelve.
                Some(Button::Close) => {}
                // Minimizar es **lo que ya hacia Ctrl+Alt**: esconderla
                // entera. No se inventa un segundo estado escondido
                // --`chrome.minimized` ademas de `win.visible`-- porque
                // dos banderas para una sola pregunta acaban diciendo
                // cosas distintas. Vuelve por donde ya volvia.
                Some(Button::Minimize) => {
                    scene::erase_box(&p, &dsk.run_box);
                    dsk.win.visible = false;
                    dsk.win.taskbar_dirty = true;
                }
                Some(Button::Maximize) => {
                    let (vx, vy, va, vl) = dsk.run_box.chrome.toggle_maximized(&p);
                    dsk.run_relayout(&p);
                    scene::erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    dsk.win.top_before = Ventana::Run;
                }
                None => {}
            }
        }

        if button && !dsk.run_box.chrome.grabbed()
            && (dsk.run_box.chrome.on_the_grip(pos.x, pos.y)
                || dsk.run_box.chrome.on_the_corner(pos.x, pos.y))
        {
            dsk.run_box.chrome.grab(pos.x, pos.y);
        } else if !button && dsk.run_box.chrome.grabbed() {
            dsk.run_box.chrome.release();
        } else if button && dsk.run_box.chrome.grabbed() {
            let (vx, vy, va, vl) = (
                dsk.run_box.x, dsk.run_box.y,
                dsk.run_box.w(), dsk.run_box.h(),
            );
            if dsk.run_box.chrome.follow_pointer(&p, pos.x, pos.y) {
                // Primero recolocar, luego borrar el sitio viejo y
                // repintar: la que se movio es ESTA, asi que
                // `erase_window` tiene que preguntar por el color de
                // fondo con la geometria NUEVA o deja un rastro de
                // copias de si misma.
                dsk.run_relayout(&p);
                // ** Se borra la RESTA, no el rectangulo entero.
                //
                // Arrastrar movia la ventana unos pocos pixeles y borraba sus
                // ~325.000 pixeles viejos --4,33 ms, la cuarta parte de un
                // fotograma-- para descubrir una tira estrecha. El resto lo
                // volvia a tapar ella misma un instante despues.
                scene::erase_moved(
                    &p,
                    &dsk.run_box,
                    (vx, vy, va, vl),
                    (dsk.run_box.x, dsk.run_box.y, dsk.run_box.w(), dsk.run_box.h()),
                    dsk.win.visible,
                );
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                dsk.win.top_before = Ventana::Run;
            }
        }
    }

    if dsk.win.sound_open && !dsk.win.sound.chrome.minimized {
        if button && !dsk.win.sound.chrome.grabbed() && dsk.win.focus.es_para(Ventana::Sound)
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
                scene::erase_moved(
                    &p,
                    &dsk.run_box,
                    (vx, vy, va, vl),
                    (
                        dsk.win.sound.chrome.x,
                        dsk.win.sound.chrome.y,
                        dsk.win.sound.chrome.width,
                        dsk.win.sound.chrome.height,
                    ),
                    dsk.win.visible,
                );
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                scene::sound::paint(
                    &p, &dsk.win.sound, dsk.snd.cap.is_some(),
                    dsk.snd.devices, dsk.snd.volume, dsk.snd.pressed,
                );
                dsk.win.top_before = Ventana::Sound;
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
                    // ** CERRAR RETIRA LA CAJA **Y** CIERRA EL PROCESO --
                    // paso 3 del plan, hecho el 2026-08-19.
                    //
                    // Y sigue sin ser `root`: el DIRECTOR no cierra "porque
                    // es el DIRECTOR", cierra porque **tiene el handle de
                    // haberlo lanzado**. `Hijo::por_tid` solo encuentra lo
                    // que `EJECUTAR` concedio; sobre una app que lanzo otro,
                    // no hay nada que encontrar y este boton no hace nada.
                    //
                    // ** El orden importa: primero la caja, despues el
                    // proceso. Al reves, `revoke_all` correria mientras esta
                    // vuelta todavia puede leer su superficie.
                    Some(Button::Close) => {
                        // El tid ANTES de soltar: `close` se lleva la
                        // superficie, y con ella la unica forma de saber de
                        // quien era esa ventana.
                        let tid = dsk.table.get_mut(i).map(|s| s.tid);
                        if let Some((vx, vy, va, vl)) = dsk.table.close(i) {
                            erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                            for s in dsk.table.iter_mut() {
                                s.repaint_all();
                            }
                        }
                        // Una app en ventana puede no tener entrada --hoy
                        // ninguna la tiene-- asi que pedirle que se vaya
                        // seria pedirselo a alguien que no escucha. Sin
                        // esto, cerrar dejaba un proceso dibujando para
                        // nadie hasta reiniciar.
                        if let Some(tid) = tid {
                            if let Some(h) = bmo::Hijo::por_tid(tid) {
                                h.cerrar();
                            }
                        }
                        // Y el foco deja de conocerla. Sin esto, Alt+Tab
                        // seguiria parando en una caja que ya no existe.
                        dsk.win.focus.close(Ventana::App(i as u8));
                    }
                    Some(Button::Minimize) => {
                        if let Some(s) = dsk.table.get_mut(i) {
                            let (vx, vy, va, vl) =
                                (s.chrome.x, s.chrome.y, s.chrome.width, s.chrome.height);
                            s.chrome.minimized = true;
                            erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
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
                            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                            s.repaint_all();
                        }
                    }
                    None => {
                        if let Some(s) = dsk.table.get_mut(i) {
                            s.chrome.grab(pos.x, pos.y);
                        }
                        // ** EL CLIC LE DA EL FOCO, y el turno largo sale de
                        // ahi: lo aplica `turno_al_foco` una vez por vuelta,
                        // en un solo sitio. Pedirlo tambien aqui seria la
                        // misma regla en dos sitios -- y ademas Alt+Tab se
                        // quedaria fuera, porque por aqui no pasa.
                        dsk.win.focus.clic_en(Ventana::App(i as u8));
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
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
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
        if let Some(i) = scene::chip_at(pos.x, pos.y, 3) {
            if i == 1 && dsk.win.data_open {
                // Estaba minimizada o no, da igual: acaba visible,
                // encajada, con el foco y delante.
                dsk.win.data.chrome.minimized = false;
                dsk.win.data.chrome.fit(&p);
                dsk.win.focus.open(Ventana::Data);
                dsk.win.focus.clic_en(Ventana::Data);
                dsk.win.data.relayout();
                scene::data::paint(&p, &dsk.win.data);
                dsk.win.top_before = Ventana::Data;
                dsk.win.taskbar_dirty = true;
            } else if i == 0 {
                if !dsk.win.visible {
                    dsk.win.visible = true;
                }
                dsk.win.focus.open(Ventana::Run);
                dsk.win.focus.clic_en(Ventana::Run);
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                dsk.win.top_before = Ventana::Run;
                dsk.win.taskbar_dirty = true;
            } else if i == 2 {
                // ** CABINA CON EL RATON, que es lo que la hace util.
                //
                // Misma secuencia que F11 (`keys/windows.rs`) y no una
                // parecida: abrir por lo ultimo, dar el foco, pintar. Si
                // las dos puertas dejaran la ventana en estados distintos,
                // el que la abre con el raton veria otra cosa que el que la
                // abre con la tecla -- y una de las dos estaria mal sin que
                // nadie pudiera decir cual.
                dsk.win.cabina_open = !dsk.win.cabina_open;
                if dsk.win.cabina_open {
                    dsk.win.cabina.from = 0;
                    dsk.win.cabina.chrome.minimized = false;
                    dsk.win.focus.open(Ventana::Cabina);
                    dsk.win.focus.clic_en(Ventana::Cabina);
                    scene::cabina::paint(&p, &dsk.win.cabina);
                    dsk.win.top_before = Ventana::Cabina;
                } else {
                    dsk.win.focus.close(Ventana::Cabina);
                    erase_window(
                        &p, &dsk.run_box,
                        dsk.win.cabina.chrome.x, dsk.win.cabina.chrome.y,
                        dsk.win.cabina.chrome.width, dsk.win.cabina.chrome.height,
                        dsk.win.visible,
                    );
                    dsk.win.top_before = Ventana::Run;
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    if dsk.win.data_open {
                        scene::data::paint(&p, &dsk.win.data);
                    }
                }
                dsk.win.taskbar_dirty = true;
            }
        }
    }
    dsk.tick.button_before = button;
    // El flanco del derecho, aparte: sin el, mantenerlo pulsado reabriria el
    // menu en cada fotograma -- el mismo fallo que ya se evito con el arbol.
    dsk.tick.derecho_before = derecho;

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
    let top = if dsk.win.cabina_open && dsk.win.focus.es_para(Ventana::Cabina) {
        Ventana::Cabina
    } else if dsk.win.data_open && dsk.win.focus.es_para(Ventana::Data) {
        Ventana::Data
    } else {
        Ventana::Run
    };
    if top != dsk.win.top_before {
        match top {
            Ventana::Cabina => scene::cabina::paint(&p, &dsk.win.cabina),
            Ventana::Data => scene::data::paint(&p, &dsk.win.data),
            // Sin guarda de `visible`: `uncover` ya no hace nada si
            // la caja esta escondida, y una guarda repetida es una que
            // puede quedarse desincronizada de la funcion.
            _ => uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field),
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

/// **Lo que hace una entrada del menu.**
///
/// * Casi todas escriben la orden en la CONSOLA en vez de hacerla por su
/// cuenta, y esa es la decision del menu entero (ver `scene::menu`): no hay un
/// segundo sitio donde se escriba en el disco, se ve la orden que se ejecuto, y
/// se aprende el terminal usando el raton.
///
/// Las dos que no pasan por ahi --entrar y subir-- es porque no escriben nada:
/// mueven el cursor, que es lo mismo que hace una flecha.
fn usar_entrada(
    dsk: &mut Desktop,
    p: &bmo::Pantalla,
    e: scene::menu::Entrada,
    sobre: scene::menu::Sobre,
) {
    use scene::menu::{Hace, Sobre};
    // El nombre de lo senalado, que es lo que la orden necesita.
    let mut nom = [0u8; 64];
    let n = match sobre {
        Sobre::Hijo(i) => bmo::estratos::hijo_nombre(i as u64, &mut nom),
        Sobre::Aqui => 0,
    };
    match e.hace {
        Hace::Entrar => {
            if let Sobre::Hijo(i) = sobre {
                if bmo::estratos::entrar(i as u64) {
                    dsk.win.data.to_top();
                    dsk.win.data.verified = None;
                }
            }
        }
        Hace::Subir => {
            if bmo::estratos::subir() {
                dsk.win.data.to_top();
                dsk.win.data.verified = None;
            }
        }
        Hace::Verificar => {
            if let Sobre::Hijo(i) = sobre {
                dsk.win.data.verified = Some(bmo::estratos::verificar(i as u64));
            }
        }
        Hace::Orden => dsk.win.data.consola.poner_orden(e.verbo, &nom[..n], true),
        Hace::Empezar => dsk.win.data.consola.poner_orden(e.verbo, &nom[..n], false),
    }
    let _ = p;
}
