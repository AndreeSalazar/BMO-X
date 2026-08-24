//! **El puntero**: que significa un clic, un arrastre y una vuelta de rueda.
//!
//! Empezo siendo 556 lineas dentro de `_start`, salio entero a un fichero
//! porque necesitaba **cinco nombres** --el escritorio, la pantalla, donde esta
//! el puntero, cuanto giro la rueda y si Ctrl esta pulsado-- y desde entonces
//! crecio hasta 995. El 2026-08-23 llego a **cinco lineas** de que L6a lo
//! rechazara, y se partio antes de tocarlo otra vez.
//!
//! # El corte, y por que es por QUIEN RECIBE y no por tamano
//!
//! El censo lo clasificaba como `GIGANTE` --dos funciones, media de 497
//! lineas-- que es la especie cara: *"el estado local tiene que volverse un
//! struct primero, y eso es diseno"*. Ese struct es [`Golpe`], y una vez
//! escrito el reparto sale solo, porque un puntero **siempre esta sobre algo**:
//!
//! ```text
//!    iconos      la rejilla del escritorio
//!    datos       la ventana de ESTRATOS y su menu contextual
//!    ventanas    CABINA, la terminal y el sonido -- las tres con marco
//!    apps        la caja de una app
//!    barra       las fichas de la barra de tareas
//! ```
//!
//! Es el mismo corte que `desktop::keys`, y no por simetria: **las dos preguntas
//! son la misma pregunta** --de quien es esta pulsacion-- hecha con el dedo en
//! un sitio distinto.
//!
//! ## Lo que se queda aqui, y es lo unico que no tiene dueno
//!
//! El reparto: los dos botones, `under_pointer` --que decide sobre QUE ventana
//! esta el raton-- la rueda, el realce de la calculadora, y el Z-order del
//! final. La calculadora se queda porque **no vive en una caja**: se dibuja
//! sobre el escritorio, asi que su pulsacion es del escritorio.
//!
//! ## [!] Los dos que devuelven `bool`, y por que importa
//!
//! `datos` y `apps` tenian `return` dentro. Ahi dentro un `return` cortaba **la
//! vuelta entera del puntero**, no su bloque: dejarlos como `return` al sacarlos
//! los habria convertido en "sal de esta funcion y sigue con lo de abajo", que
//! es otro programa. Devuelven `true` y quien llama corta.

use bmo_userland as bmo;

use super::{calc, Desktop, Ventana};
use crate::scene::calc::paint_calc;
use crate::scene::{self};
use crate::uncover;

/// One frame's worth of pointer.
///
/// Order inside is not free: the wheel is served AFTER working out which
/// window the pointer is over, because before that it always scrolled the
/// output history -- with the kernel console open on top, turning the wheel
/// scrolled a grid that was not even visible.

/// La rejilla de iconos del escritorio.
pub(crate) mod iconos;
/// La ventana de DATOS (ESTRATOS) y su menu contextual.
pub(crate) mod datos;
/// CABINA, la terminal y el sonido: las tres con marco.
pub(crate) mod ventanas;
/// La caja de una app.
pub(crate) mod apps;
/// Las fichas de la barra de tareas.
pub(crate) mod barra;

/// **Lo que una vuelta del puntero sabe antes de repartirla.**
///
/// Es el struct que el censo predijo que haria falta para partir este fichero:
/// *"el estado local tiene que volverse un struct primero"*. No lleva
/// `under_pointer` a proposito -- eso se calcula a mitad de la vuelta, despues
/// de que los iconos hayan tenido su turno, y meterlo aqui obligaria a mover ese
/// calculo hacia arriba. Reordenar para que un struct quede bonito es cambiar el
/// programa por una razon que no es del programa.
pub(crate) struct Golpe {
    pub pos: bmo::Punto,
    /// El boton IZQUIERDO, tal cual. El FLANCO --"acaba de bajar"-- lo calcula
    /// cada bloque con `dsk.tick.button_before`, igual que antes.
    pub button: bool,
    pub derecho: bool,
    pub ctrl: bool,
}

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

    let g = Golpe { pos, button, derecho, ctrl };
    iconos::on_pointer(dsk, p, &g);

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


    if datos::on_pointer(dsk, p, &g) {
        return;
    }
    ventanas::on_pointer(dsk, p, &g);
    if apps::on_pointer(dsk, p, &g) {
        return;
    }
    barra::on_pointer(dsk, p, &g);

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
pub(super) fn usar_entrada(
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
