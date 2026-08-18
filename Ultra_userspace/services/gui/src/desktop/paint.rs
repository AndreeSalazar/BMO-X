//! **Closing the frame**: drain the child, the taskbar chips, the caret, the
//! apps, the vitals, and the mouse cursor on top of everything.
//!
//! 245 lines, and they need **three names**: the desktop, the screen and how
//! many app windows died this turn. That is the smallest signature of any
//! block in the loop, and the reason is the order itself -- everything in here
//! happens once the decisions are already made.
//!
//! ## The order is the design
//!
//! - The dead windows' holes are given back BEFORE composing the live ones.
//!   Erasing afterwards would cover a window that is still there.
//! - The apps go LAST, for the same reason the cursor does: what is painted
//!   last is what ends up on top.
//! - The vitals repaint before the cursor, because anything painted after the
//!   cursor eats its save-under.
//! - `p.vaciar()` is the final instruction. The framebuffer is mapped
//!   write-combining, so without it what was painted this frame sits waiting
//!   for someone to write more -- the exact symptom seen on the Ryzen, where
//!   typing painted nothing until the mouse moved.

use bmo_userland as bmo;

use super::{BLINK, Desktop, Ventana};
use crate::scene::calc::paint_calc;
use crate::scene::output::paint_output;
use crate::scene::{self, paint_field, ACCENT, TASKBAR};
use crate::{erase_window, uncover};

/// Everything that happens after the input has been read and understood.
pub(crate) fn compose(dsk: &mut Desktop, p: &bmo::Pantalla, dead: usize) {
    // -- Drenar la salida de los hijos --
    //
    // Con tope por fotograma. Un programa que escupe sin parar podria
    // quedarse con el bucle entero y congelar el cursor: es preferible que
    // la salida vaya un poco por detras a que el escritorio deje de
    // responder. Lo que no se lea ahora sigue en el anillo del kernel.
    if let Some(c) = dsk.out.console.as_ref() {
        let mut buf = [0u8; 8];
        let mut drained = 0;
        while drained < 64 {
            let read_bytes = c.read(&mut buf);
            if read_bytes == 0 {
                break;
            }
            if dsk.calc.waiting {
                // Todo lo que escriba el motor es la respuesta: el
                // programa no imprime prompts a proposito.
                for &b in &buf[..read_bytes] {
                    if b == b'\n' {
                        // ** LA PRIMERA LINEA ES EL ESTADO, NO LA RESPUESTA.
                        //
                        // El motor contesta `estado \n valor \n`. Antes mandaba
                        // una sola linea y ESTE bucle daba por hecho que era un
                        // numero -- asi que un "codigo no valido" del motor se
                        // pintaba en la pantallita como si fuera una cifra.
                        //
                        // Con la tecla `$` eso deja de poder arreglarse
                        // mirando: `$12,345.67` es una respuesta BUENA y no
                        // parece un numero. Quien sabe si contesto es el motor,
                        // y por eso lo dice el en vez de adivinarlo nosotros.
                        if !dsk.calc.respondio {
                            dsk.calc.respondio = true;
                            dsk.calc.bien = dsk.resp_n > 0 && dsk.resp[0] == b'0';
                            dsk.resp_n = 0;
                            continue;
                        }
                        if dsk.resp_n > 0 {
                            dsk.calc.input = [0; 20];
                            let k = dsk.resp_n.min(dsk.calc.input.len());
                            dsk.calc.input[..k].copy_from_slice(&dsk.resp[..k]);
                            dsk.calc.n = k;
                            dsk.calc.saved_n = 0;
                            dsk.calc.op = 0;
                            dsk.calc.waiting = false;
                            dsk.calc.respondio = false;
                            // Lo que no es una cifra es una PRESENTACION (`$`)
                            // o un motivo: en los dos casos, teclear encima
                            // empieza de cero en vez de anadir al final.
                            dsk.calc.presentado = !dsk.calc.bien
                                || dsk.calc.input[..k].iter().any(|c| {
                                    !c.is_ascii_digit() && *c != b'.' && *c != b'-'
                                });
                            // * El cursor SE APARTA antes de pintar aqui.
                            //
                            // Este es el unico pintado del bucle que no
                            // dispara la ENTRADA: lo dispara el HIJO al
                            // contestar. Asi que puede caer en un fotograma
                            // con `will_paint` en falso -- o sea con el
                            // puntero todavia en pantalla y lo que hay
                            // debajo ya guardado. Pintar encima **caduca**
                            // ese guardado, y el `lift` de la vuelta
                            // siguiente devolveria los pixeles viejos
                            // encima del resultado recien escrito: un
                            // rectangulo fantasma sobre la calculadora.
                            //
                            // `lift` es idempotente --si no esta puesto no
                            // hace nada--, asi que llamarlo aqui no cuesta
                            // nada en los fotogramas que ya lo apartaron.
                            dsk.save_under.lift(&p);
                            paint_calc(&p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
                        }
                    } else if dsk.resp_n < dsk.resp.len() && b >= 0x20 {
                        dsk.resp[dsk.resp_n] = b;
                        dsk.resp_n += 1;
                    }
                }
            } else {
                dsk.out.grid.text(&buf[..read_bytes]);
            }
            drained += 1;
        }
    }
    // * Y solo en un fotograma que haya apartado el cursor. Un hijo que
    // escribe no es motivo suficiente: pintar aqui dejaria el puntero
    // enterrado bajo la rejilla y, al quitarlo, devolveria pixeles viejos
    // encima de lo recien escrito. `dirty` se queda puesto y la vuelta
    // siguiente ya empieza sabiendo que hay que pintar.
    if dsk.out.grid.dirty && dsk.tick.will_paint {
        // Se pinta solo si se ve; el contenido sigue acumulandose oculto,
        // asi que al invocar la ventana esta todo lo que paso mientras.
        //
        // * Y NO si la consola de datos esta ARRIBA. Sin este guardia, el
        // fotograma siguiente repintaria la rejilla POR DEBAJO y la
        // dibujaria encima de la ventana de datos, dejandola a trozos. La
        // salida no se pierde: `dirty` se queda puesto y se pinta entera
        // cuando esta ventana vuelva a estar arriba.
        //
        // Y es ARRIBA, no ABIERTA: con Datos abierta pero detras, la
        // rejilla se ve y tiene que seguir escribiendose.
        if dsk.win.visible && dsk.win.top_before != Ventana::Data && !dsk.win.switcher_painted {
            paint_output(&p, &dsk.run_box, &dsk.out.grid);
            dsk.out.grid.dirty = false;
        } else if !dsk.win.visible {
            dsk.out.grid.dirty = false;
        }
    }

    // -- Las FICHAS de la barra --
    //
    // Se repintan solo cuando algo cambia de estado. Son la lista de lo que
    // hay abierto, y la unica forma de volver a una ventana minimizada.
    //
    // * Lo que las ensucia se calcula AQUI, comparando el estado con el del
    // fotograma anterior, en vez de poner `taskbar_dirty = true` en los seis
    // sitios que cambian algo. Un `sucio` que hay que acordarse de poner es
    // un `sucio` que un dia no se pone, y entonces la barra ensena un
    // estado viejo sin que nada falle -- el peor tipo de fallo de interfaz.
    let taskbar_state = (
        dsk.win.visible,
        dsk.win.top_before,
        dsk.win.data_open,
        dsk.win.data.chrome.minimized,
        dsk.win.cabina_open,
    );
    if taskbar_state != dsk.win.taskbar_state_before {
        dsk.win.taskbar_state_before = taskbar_state;
        dsk.win.taskbar_dirty = true;
    }
    if dsk.win.taskbar_dirty && dsk.tick.will_paint {
        scene::paint_chip(&p, 0, "Ejecutar", ACCENT, dsk.win.visible && dsk.win.top_before == Ventana::Run, !dsk.win.visible);
        if dsk.win.data_open {
            scene::paint_chip(
                &p, 1, "ESTRATOS", 0x0034_D399,
                dsk.win.top_before == Ventana::Data, dsk.win.data.chrome.minimized,
            );
        } else {
            // Cerrada: su hueco vuelve al color de la barra. Una ficha que
            // se queda tras cerrar la ventana promete algo que ya no esta.
            let (fx, fy, fw, fh) = scene::chip_box(1);
            p.rect(fx, fy, fw, fh, TASKBAR);
        }
        // -- ** CABINA: LA UNICA FICHA QUE ESTA SIEMPRE --
        //
        // Las otras dos aparecen cuando su ventana existe. Esta no, y el
        // motivo es el dia que la puso: **el teclado dejo de escribir y con
        // el se fue la unica forma de diagnosticarlo**. CABINA vivia detras
        // de F11, `guarda` detras de escribir, y el raton --que seguia
        // funcionando perfectamente-- no podia abrir nada.
        //
        // Un panel de diagnostico al que solo se llega con el aparato que
        // puede estar roto no es un panel de diagnostico. Asi que esta ficha
        // se pinta aunque la ventana este cerrada: es la puerta, no el
        // recordatorio.
        scene::paint_chip(
            &p, 2, "CABINA", 0x00F5_9E0B,
            dsk.win.cabina_open && dsk.win.top_before == Ventana::Cabina,
            !dsk.win.cabina_open,
        );
        // El testigo del USB vive en la misma barra, en la ranura siguiente a
        // CABINA. Repintar las fichas no lo toca --esta despues-- pero SI lo
        // tapa lo que repinta la barra entera, y de ahi se vuelve por aqui:
        // `taskbar_dirty` es la senal comun de "la barra se ha vuelto a
        // pintar". Olvidando lo pintado, la luz se dibuja en la vuelta
        // siguiente.
        //
        // Un hueco vacio donde estaba la luz se lee como "no hay problema", que
        // es la peor cosa que puede decir un instrumento que se borro.
        scene::testigo::olvidar();
        dsk.win.taskbar_dirty = false;
    }

    // El parpadeo del cursor de escritura. Solo repinta cuando cambia de
    // estado -- repintar el campo cada vuelta seria reescribir la ruta
    // miles de veces por segundo para que se vea igual.
    //
    // * El contador se REINICIA con cada tecla (ver el manejador). Antes
    // era `frames % BLINK`, un reloj que corria solo: si te ponias a
    // escribir justo cuando tocaba apagarlo, el cursor desaparecia a mitad
    // de la palabra y no volvia hasta la siguiente vuelta entera. Un
    // cursor que se esconde mientras escribes es lo contrario de lo que
    // un cursor existe para decir.
    dsk.field.since_key = dsk.field.since_key.wrapping_add(1);
    if dsk.field.since_key >= BLINK {
        dsk.field.since_key = 0;
        dsk.field.caret = !dsk.field.caret;
        dsk.tick.repaint_field = true;
    }
    if dsk.tick.repaint_field
        && dsk.tick.will_paint
        && dsk.win.visible
        && dsk.win.top_before != Ventana::Data
        && !dsk.win.switcher_painted
    {
        paint_field(&p, &dsk.run_box, dsk.field.line(), dsk.field.cur, dsk.field.caret);
    }

    // * UNA sola vez, al cerrar el primer fotograma entero. Con esto, las
    // ultimas palabras que guarda el kernel dicen DONDE murio sin tener que
    // adivinarlo:
    //
    //   "reclamo pantalla y entrada"  -> murio en el arranque o en la intro
    //   "escritorio pintado"          -> murio sin cerrar el primer cuadro
    //   "primer fotograma completo"   -> murio ya en el bucle
    //
    // Tres mensajes que ya existian mas este, y el diagnostico deja de ser
    // una teoria. Cuesta una linea en el log y se dice una vez en la vida
    // del proceso.
    // -- ** LAS APPS, COMPUESTAS --
    //
    // Va **al final** y por el mismo motivo que el cursor va detras: lo que
    // se pinta al final es lo que queda encima. Una app en su caja esta por
    // delante de las ventanas del sistema, y el unico que se le pone encima
    // es el puntero del raton.
    //
    // El hueco de las que murieron se devuelve ANTES de componer las vivas:
    // borrar despues taparia a una ventana que si esta.
    if dsk.tick.will_paint {
        for &(vx, vy, va, vl) in dsk.tick.dead_boxes[..dead].iter() {
            erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
        }
        if dead > 0 {
            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
            // Lo que quedara debajo de la que se fue tiene que volver a
            // pintarse: `erase_window` devuelve el FONDO, no las ventanas.
            for s in dsk.table.iter_mut() {
                s.repaint_all();
            }
        }
        dsk.table.compose(&p);
    }

    if dsk.tick.frames == 1 {
        bmo::consola("primer fotograma completo\n");
    }

    // -- ** LAS VITALES SE REPINTAN SOLAS, y eso es lo que las hace vistas
    //
    // Aqui, al final del fotograma y ANTES del cursor: lo que se pinte
    // despues del cursor le come el save-under.
    //
    // Cada 15 vueltas y no cada una, por dos razones que van juntas:
    //
    //   * Un panel de ~500x400 repintado a 60 fps son megabytes por segundo
    //     de volcado por unos numeros que cambian despacio. Es justo el
    //     derroche que el troceado por regiones acaba de quitar en otro
    //     sitio.
    //   * Y los numeros del CPU son MEDIDAS POR DIFERENCIA: con intervalos
    //     de 16 ms la ventana es tan corta que el resultado tiembla. A 15
    //     fotogramas son ~250 ms, que es donde un vatio se lee quieto.
    //
    // O sea que refrescar mas no daria mas informacion: daria la misma
    // temblando.
    if (dsk.win.cpu_open || dsk.win.mem_open) && dsk.tick.frames % 15 == 0 {
        if dsk.win.cpu_open {
            scene::vitals::paint(&p, &dsk.win.cpu);
        }
        if dsk.win.mem_open {
            scene::vitals::paint(&p, &dsk.win.mem);
        }
    }

    // -- ** EL TESTIGO DEL BUS USB: la luz que no hay que abrir --
    //
    // Aqui, con las vitales y antes del cursor, por el mismo motivo: lo que se
    // pinta despues del cursor le come el save-under.
    //
    // ** Y SOLO EN FOTOGRAMAS QUE VAYAN A PINTAR, que es lo que lo hace seguro
    // sin apartar el puntero a mano. La duda razonable es si eso lo puede dejar
    // sin repintar justo el dia malo --si el teclado esta muerto y el raton
    // quieto, no hay entrada que dispare nada--, y no: el parpadeo del cursor
    // de escritura pone `will_paint` cada `BLINK` vueltas **sin que nadie toque
    // un aparato**. La luz llega tarde como mucho medio parpadeo.
    //
    // Cada 15 fotogramas, como las vitales: es un estado que cambia despacio y
    // mirarlo mas rapido no da mas informacion.
    if dsk.tick.will_paint {
        scene::testigo::refrescar(&p, dsk.tick.frames, 15);
    }

    // -- El cursor del raton, ENCIMA de todo y lo ultimo --
    //
    // Aqui ya no queda nada por pintar en este fotograma, asi que lo que se
    // guarda debajo es lo definitivo. Ponerlo antes obligaria a que cada
    // ventana supiera esquivarlo -- que es justo lo que no se puede pedir a
    // una ventana que todavia no existe.
    if dsk.tick.ax != u32::MAX {
        // * QUE ESTA DICIENDO EL PUNTERO.
        //
        // Se decide aqui, al final del fotograma, porque es aqui donde ya
        // se sabe todo lo que paso en el: que ventana quedo arriba, donde
        // acabo el raton y si la calculadora esta abierta.
        //
        // El orden de las preguntas es el Z-order otra vez: lo que esta
        // encima manda. Un boton de la calculadora tapado por la consola
        // del kernel no puede pedir la mano -- senalaria algo que no se
        // puede pulsar, que es peor que no senalar nada.
        let shape = if dsk.calc.visible
            && dsk.win.top_before == Ventana::Run
            && dsk.calc_pad.key_at(dsk.tick.ax, dsk.tick.ay).is_some()
        {
            scene::cursor::Shape::Hand
        } else if dsk.win.visible && dsk.win.top_before == Ventana::Run && dsk.run_box.on_field(dsk.tick.ax, dsk.tick.ay) {
            scene::cursor::Shape::Beam
        } else {
            scene::cursor::Shape::Arrow
        };
        dsk.save_under.place(&p, dsk.tick.ax, dsk.tick.ay, shape);
    }

    // * Y ahora EMPUJARLO a la pantalla.
    //
    // El framebuffer esta mapeado en write-combining: el CPU acumula las
    // escrituras y las suelta cuando el bufer se llena. Sin esta linea, lo
    // pintado en este fotograma se queda esperando a que alguien escriba
    // mas -- y el sintoma es exactamente el que aparecio en el Ryzen:
    // teclear no pintaba nada hasta que se movia el raton, porque mover el
    // raton era lo que llenaba el bufer.
    //
    // Una instruccion, una vez por fotograma, al final de todo. Ver
    // `Pantalla::vaciar`.
    p.vaciar();
}
