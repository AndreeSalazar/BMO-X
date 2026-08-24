//! **El raton sobre la ventana de DATOS** (ESTRATOS), y su menu contextual.
//!
//! Es el bloque mas grande de los que reparte `super::on_pointer`, y por eso
//! sale el primero: dentro hay cuatro cosas que no se parecen --las pestanas,
//! la rejilla, el menu del clic derecho y la consola del pie-- y todas quieren
//! la misma pulsacion.
//!
//! ** DEVUELVE `true` CUANDO YA ESTA ATENDIDO. El `return` que tenia dentro
//! cortaba la vuelta ENTERA del puntero, no solo este bloque: convertirlo en un
//! `bool` es lo que conserva ese significado al sacarlo de la funcion grande.
//! Si se hubiera dejado como `return`, la barra de tareas y el Z-order
//! seguirian corriendo detras de un clic que ya tenia dueno.

use bmo_userland as bmo;

use super::{usar_entrada, Golpe};
use crate::desktop::{Desktop, Ventana};
use crate::scene;
use crate::{erase_window, uncover};

pub(crate) fn on_pointer(dsk: &mut Desktop, p: &bmo::Pantalla, g: &Golpe) -> bool {
    let pos = g.pos;
    let button = g.button;
    let derecho = g.derecho;
    let ctrl = g.ctrl;

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
            return true;
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
                        // -- ** CLIC EN LA REJILLA: senalar, y abrir al SEGUNDO
                        //
                        // Faltaba entero. `fila_rejilla_en` existia desde que
                        // hay menu contextual, pero solo lo miraba el boton
                        // DERECHO: con el izquierdo se podia abrir un menu
                        // sobre un archivo y no se podia senalar ese archivo.
                        // Una lista en la que se pulsa y no pasa nada parece
                        // rota aunque el teclado la recorra bien.
                        //
                        // ** El primero SOLO senala. Abrir con un clic suelto
                        // en una lista donde tambien se arrastra la ventana
                        // seria entrar en carpetas sin querer.
                        //
                        // Y el segundo hace **lo mismo que ENTRAR**, llamando a
                        // lo mismo: `entrar` dice que no cuando es un archivo,
                        // y entonces no pasa nada -- que es correcto, porque un
                        // archivo no tiene dentro. El dia que haya con que
                        // abrirlo, se anade en `entrar` y las dos formas lo
                        // heredan a la vez.
                        else if let Some(i) = dsk.win.data.fila_rejilla_en(pos.x, pos.y) {
                            let doble = dsk.win.data.clic_rejilla(i);
                            if doble {
                                // Lo MISMO que ENTRAR, y llamando a lo mismo:
                                // baja si es carpeta, y si es archivo lo abre
                                // en el visor. Dos gestos, una regla.
                                if bmo::estratos::entrar(i as u64) {
                                    dsk.win.data.to_top();
                                } else {
                                    dsk.win.data.ver_senalado();
                                }
                            }
                            scene::data::paint(&p, &dsk.win.data);
                            dsk.win.top_before = Ventana::Data;
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

    false
}
