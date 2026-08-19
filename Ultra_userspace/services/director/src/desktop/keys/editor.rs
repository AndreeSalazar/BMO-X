//! **The one line of the Run box**: type, move, cut, paste, complete, recall,
//! and Enter.
//!
//! ## Why this returns a value for `run`
//!
//! Every key is handled here except one outcome: `run` hands the machine to
//! the child. `lend_screen` takes the screen and the input capability **by
//! value** and gives them back, and those two are bindings of `_start`, not
//! fields of `Desktop`. So the editor DECIDES and `_start` EXECUTES -- the
//! target comes back in [`Edit::Launch`].

use bmo_userland as bmo;

use super::Edit;
use crate::commands::complete::complete;
use crate::commands::{dispatch, parse, After, Command};
use crate::desktop::Desktop;
use crate::scene::output::{INK_ECHO, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};
use crate::PATH_MAX;

pub(crate) fn on_key(dsk: &mut Desktop, p: &bmo::Pantalla, c: u8, ctrl: bool) -> Edit {
    debug_assert!(dsk.win.visible, "el foco de una ventana escondida es un bug");
    // Cualquier tecla enciende el cursor y reinicia el parpadeo.
    dsk.field.caret = true;
    dsk.field.since_key = 0;
    dsk.tick.repaint_field = true;
// Cualquier tecla enciende el cursor y reinicia el parpadeo.
dsk.field.caret = true;
dsk.field.since_key = 0;
dsk.tick.repaint_field = true;
match c {
    b'\r' | b'\n' => {
        // Eco SIEMPRE, tambien de lo que no se entiende: un
        // terminal que se traga lo que escribiste deja al
        // usuario sin saber que llego.
        // El eco lleva un punto medio (0xB7) y no `>`. El `>`
        // es la marca de Unix y este sistema no es Unix; el
        // punto medio separa igual de bien y no arrastra la
        // convencion de otro. Esta en la tabla de extras del
        // font, asi que se dibuja sin tocar nada mas.
        // El eco en su tinta y la respuesta en la normal: al
        // mirar la rejilla, los comandos son las anclas y todo
        // lo de debajo es lo que contestaron.
        dsk.out.grid.with_ink(INK_ECHO);
        dsk.out.grid.byte(0xB7);
        dsk.out.grid.byte(b' ');
        dsk.out.grid.text(dsk.field.line());
        dsk.out.grid.byte(b'\n');
        dsk.out.grid.with_ink(INK_PLAIN);

        // Hay un programa vivo escuchando en esta consola?
        // Entonces la linea NO es un comando: es SUYA. Es lo
        // que hace cualquier shell, y sin esto un `ACCEPT` de
        // COBOL no puede recibir nada nunca -- el terminal se
        // come la respuesta y contesta "no lo conozco".
        //
        // La calculadora se excluye a proposito: mientras
        // espera al motor, ese hijo es SUYO y ya recibio sus
        // tres lineas. Colar una mas ahi le cambiaria la
        // cuenta a alguien que no la pidio.
        let from_child = !dsk.calc.waiting
            && dsk.out.console.as_ref().map(|cc| cc.has_child()).unwrap_or(false);

        if from_child {
            if let Some(cc) = dsk.out.console.as_ref() {
                cc.write(dsk.field.line());
                // El salto va aparte y SIEMPRE: `read_line`
                // espera a verlo para dar la linea por
                // cerrada. Sin el, el programa sigue
                // esperando algo que ya escribiste.
                cc.write(b"\n");
            }
            paint_status(&p, &dsk.run_box, "para el programa", INK_DIM);
            dsk.field.n = 0;
            dsk.field.cur = 0;
            dsk.tick.repaint_field = true;
            return Edit::Taken;
        }

        // Al historial va lo que es un COMANDO. Un importe
        // tecleado para un `ACCEPT` es un dato, y mezclarlo
        // con las rutas ensucia la flecha arriba justo cuando
        // hace falta repetir el comando de verdad.
        dsk.field.history.push(&dsk.field.path[..dsk.field.n]);
        // ** LA LINEA SE COPIA ANTES DE INTERPRETARLA.
        //
        // `Command<'a>` toma prestado `dsk.field.path`, asi que
        // pasarle la orden a un `dispatch(&mut dsk, ..)` choca
        // con ese prestamo. Copiarla a la pila lo desata: 128
        // bytes una vez por Enter --no por fotograma-- y a
        // cambio las veintiuna ordenes salen del fichero.
        let mut line = [0u8; PATH_MAX];
        let ln = dsk.field.n;
        line[..ln].copy_from_slice(&dsk.field.path[..ln]);
        match parse(&line[..ln]) {
            // ** `run` SE DEVUELVE, no se ejecuta aqui.
            //
            // `lend_screen` se lleva la pantalla y la entrada POR VALOR y las
            // devuelve. Esos dos son bindings de `_start`, no campos del
            // `Desktop`, asi que el editor DECIDE y `_start` EJECUTA. La ruta
            // se copia porque `target` toma prestada la linea local.
            Command::Launch(target) => {
                let mut buf = [0u8; PATH_MAX];
                let tn = target.len().min(PATH_MAX);
                buf[..tn].copy_from_slice(&target[..tn]);
                return Edit::Launch(buf, tn);
            }
            // `run` NO baja a `commands/`: es la unica orden
            // que se lleva la pantalla y la entrada POR VALOR, y
            // esos dos son bindings de `_start`, no campos del
            // `Desktop`. Ver la cabecera de `commands/dispatch`.
            other => match dispatch(dsk, p, other) {
                After::NextKey => return Edit::Taken,
                After::Settle => {}
            },
        }
        // El cursor detras de la linea, SIEMPRE. Las ramas que
        // vacian el campo ponian `n = 0` y dejaban `cur` donde
        // estaba: la tecla siguiente se escribia en `path[cur]`
        // --fuera de lo que se dibuja-- y el campo ensenaba los
        // bytes VIEJOS del comando anterior. Escribir `2` tras
        // `run apps/calc.bex` mostraba una `r`. Las ramas de
        // error conservan la ruta a proposito para poder
        // corregirla, y ahi `cur` no se mueve: por eso es un
        // `min` y no un cero.
        dsk.field.cur = dsk.field.cur.min(dsk.field.n);
        dsk.tick.repaint_field = true;
    }
    // TAB: completar.
    b'\t' => {
        let antes = dsk.field.n;
        dsk.field.n = complete(&mut dsk.field.path, dsk.field.n, &mut dsk.out.grid);
        dsk.field.cur = dsk.field.n;
        if dsk.field.n == antes {
            paint_status(&p, &dsk.run_box, "nada que completar", INK_DIM);
        }
        dsk.tick.repaint_field = true;
    }
    // Retroceso.
    //
    // ** LA GUARDA ES `cur > 0 && n > 0`, Y LE FALTABA LA
    // SEGUNDA MITAD. Panico en el Ryzen el 2026-08-09:
    //
    //     range end index 18446744073709551615
    //     out of range for slice of length ...
    //     en services\gui\src\main.rs:2834
    //
    // Esa linea es `paint_field(..., &path[..dsk.field.n], ...)`, y el
    // indice es `usize::MAX`: **`n` se desbordo por abajo**.
    // Este `n -= 1` estaba guardado por `cur > 0` -- que es la
    // condicion del OTRO contador. Con `cur > 0` y `n == 0`, la
    // resta da la vuelta y el siguiente repintado revienta.
    //
    // ** Y para llegar ahi hacia falta romper `cur <= n`, que es
    // el invariante de este campo. Lo rompio el camino nuevo del
    // lanzador: pulsar el icono deja `n = cur = 17`, el `run` se
    // lanza, **falla la admision**, y en ese camino de fallo `n`
    // vuelve a 0 sin que `cur` le acompane. Un retroceso
    // despues, la maquina se lleva el escritorio por delante.
    //
    // Se arregla en los dos sitios: aqui la guarda correcta, y
    // arriba el invariante restaurado en cada vuelta -- que es
    // lo que impide que el proximo camino nuevo lo vuelva a
    // romper sin que nadie se entere.
    0x08 | 0x7F => {
        if dsk.field.cur > 0 && dsk.field.n > 0 {
            let mut k = dsk.field.cur;
            while k < dsk.field.n {
                dsk.field.path[k - 1] = dsk.field.path[k];
                k += 1;
            }
            dsk.field.cur -= 1;
            dsk.field.n -= 1;
            dsk.tick.repaint_field = true;
        }
    }
    // Escape: borrar la linea entera, igual que en el Win+R.
    0x1B => {
        dsk.field.n = 0;
        dsk.field.cur = 0;
        paint_status(&p, &dsk.run_box, "listo", INK_DIM);
        dsk.tick.repaint_field = true;
    }
    // -- El portapapeles --
    //
    // Ctrl+C copia la linea entera; Ctrl+V la pega donde este
    // el cursor. No es un lujo: la mitad de lo que se teclea en
    // un terminal es una variacion de lo anterior, y sin copiar
    // hay que reescribirlo todo.
    //
    // Ctrl+C para copiar y no para interrumpir, que es lo que
    // significa en Unix. Aqui no hay senales que mandar, y el
    // dedo que ya sabe Ctrl+C sabe copiar -- no interrumpir.
    0x03 => {
        dsk.field.clipboard_n = dsk.field.n;
        let upto = dsk.field.n;
        let (src, dst) = (&dsk.field.path[..upto], &mut dsk.field.clipboard[..upto]);
        dst.copy_from_slice(src);
        paint_status(&p, &dsk.run_box, "copiado", INK_DIM);
    }
    0x16 => {
        if dsk.field.clipboard_n > 0 && dsk.field.n + dsk.field.clipboard_n <= PATH_MAX {
            // Hueco del tamano del pegado, y meterlo.
            let mut k = dsk.field.n;
            while k > dsk.field.cur {
                dsk.field.path[k + dsk.field.clipboard_n - 1] = dsk.field.path[k - 1];
                k -= 1;
            }
            dsk.field.path[dsk.field.cur..dsk.field.cur + dsk.field.clipboard_n].copy_from_slice(&dsk.field.clipboard[..dsk.field.clipboard_n]);
            dsk.field.cur += dsk.field.clipboard_n;
            dsk.field.n += dsk.field.clipboard_n;
            dsk.tick.repaint_field = true;
        }
    }
    // Ctrl+U -- borra la linea. Ctrl+L -- borra la salida.
    // Los mismos que el shell de Ring 0, porque los dedos ya
    // los tienen y un atajo que cambia entre dos ventanas del
    // mismo sistema es peor que no tenerlo.
    0x15 => {
        dsk.field.n = 0;
        dsk.field.cur = 0;
        dsk.tick.repaint_field = true;
    }
    0x0C => {
        dsk.out.grid.clear();
        dsk.tick.repaint_field = true;
    }
    // FLECHA ARRIBA / ABAJO -- el historial. Llegan por la misma
    // cola que las letras, con bytes del rango C1 (0x80..0x9F)
    // que no tienen glifo: el driver los eligio justo para que
    // no puedan confundirse con texto.
    // Ctrl+ARRIBA copia, Ctrl+ABAJO pega. Lo mismo que
    // Ctrl+C / Ctrl+V, con las flechas -- porque los dedos que
    // ya andan por el historial no tienen que irse a buscar
    // otra tecla para copiar lo que acaban de recuperar.
    0x80 if ctrl => {
        dsk.field.clipboard_n = dsk.field.n;
        let upto = dsk.field.n;
        let (src, dst) = (&dsk.field.path[..upto], &mut dsk.field.clipboard[..upto]);
        dst.copy_from_slice(src);
        paint_status(&p, &dsk.run_box, "copiado", INK_DIM);
    }
    0x81 if ctrl => {
        if dsk.field.clipboard_n > 0 && dsk.field.n + dsk.field.clipboard_n <= PATH_MAX {
            let mut k = dsk.field.n;
            while k > dsk.field.cur {
                dsk.field.path[k + dsk.field.clipboard_n - 1] = dsk.field.path[k - 1];
                k -= 1;
            }
            dsk.field.path[dsk.field.cur..dsk.field.cur + dsk.field.clipboard_n].copy_from_slice(&dsk.field.clipboard[..dsk.field.clipboard_n]);
            dsk.field.cur += dsk.field.clipboard_n;
            dsk.field.n += dsk.field.clipboard_n;
            dsk.tick.repaint_field = true;
        }
    }
    0x80 => {
        if let Some(k) = dsk.field.history.back(&mut dsk.field.path) {
            dsk.field.n = k;
            dsk.field.cur = k;
            dsk.tick.repaint_field = true;
        }
    }
    0x81 => {
        if let Some(k) = dsk.field.history.forward(&mut dsk.field.path) {
            dsk.field.n = k;
            dsk.field.cur = k;
            dsk.tick.repaint_field = true;
        }
    }
    // IZQUIERDA / DERECHA -- mover el cursor.
    0x82 => {
        if dsk.field.cur > 0 { dsk.field.cur -= 1; dsk.tick.repaint_field = true; }
    }
    0x83 => {
        if dsk.field.cur < dsk.field.n { dsk.field.cur += 1; dsk.tick.repaint_field = true; }
    }
    // INICIO / FIN.
    0x84 => { dsk.field.cur = 0; dsk.tick.repaint_field = true; }
    0x85 => { dsk.field.cur = dsk.field.n; dsk.tick.repaint_field = true; }
    // -- Los atajos de edicion de linea --
    //
    // Los de toda la vida en una consola: Ctrl+A al principio,
    // Ctrl+E al final, Ctrl+K corta hasta el final, Ctrl+W
    // borra la palabra de atras. Van ADEMAS de Inicio/Fin, que
    // ya estaban: los dedos que vienen de un terminal buscan
    // estos, y los que vienen de Windows buscan aquellos.
    // Atender a los dos cuesta cuatro lineas.
    0x01 => { dsk.field.cur = 0; dsk.tick.repaint_field = true; }
    0x05 => { dsk.field.cur = dsk.field.n; dsk.tick.repaint_field = true; }
    // Ctrl+K: tirar lo que hay del cursor al final.
    0x0B => {
        dsk.field.n = dsk.field.cur;
        dsk.tick.repaint_field = true;
    }
    // Ctrl+W: borrar la palabra de atras. Primero se comen los
    // espacios y luego las letras, que es lo que espera
    // cualquiera que lo haya usado -- si no, borrar tras un
    // espacio no haria nada.
    0x17 => {
        // `cur - k` con `cur` pasado de `n` daria un `removed`
        // enorme y el `n -= removed` de abajo se desbordaria
        // igual que el retroceso. El invariante de arriba ya lo
        // impide; la guarda se queda porque esta resta no tiene
        // por que fiarse de que alguien lo mantenga.
        let limit = dsk.field.cur.min(dsk.field.n);
        let mut k = limit;
        while k > 0 && dsk.field.path[k - 1] == b' ' { k -= 1; }
        while k > 0 && dsk.field.path[k - 1] != b' ' { k -= 1; }
        let removed = limit - k;
        if removed > 0 {
            let mut i = limit;
            while i < dsk.field.n {
                dsk.field.path[i - removed] = dsk.field.path[i];
                i += 1;
            }
            dsk.field.n -= removed;
            dsk.field.cur = k;
            dsk.tick.repaint_field = true;
        }
    }
    // SUPRIMIR -- borra HACIA ADELANTE, al reves que el
    // retroceso. Son dos teclas porque son dos intenciones.
    0x86 => {
        if dsk.field.cur < dsk.field.n {
            let mut k = dsk.field.cur + 1;
            while k < dsk.field.n { dsk.field.path[k - 1] = dsk.field.path[k]; k += 1; }
            dsk.field.n -= 1;
            dsk.tick.repaint_field = true;
        }
    }
    // * PgUp / PgDn -- el historial de la salida.
    //
    // Estaban ignoradas "explicitamente", que era honesto pero
    // inutil: lo que salia por arriba se perdia para siempre, y
    // en una maquina donde depurar es fotografiar la pantalla,
    // perder la salida de un batch cuesta un arranque entero.
    // Ahora suben y bajan la ventana sobre 200 filas guardadas.
    0x87 => {
        // Una pagina es lo que SE VE, no el tope: con la constante, en una
        // ventana pequena RePag saltaria por encima de filas que nunca se
        // llegaron a leer.
        dsk.out.grid.scroll_view(dsk.run_box.out_rows() as i32 - 1);
    }
    0x88 => {
        dsk.out.grid.scroll_view(-(dsk.run_box.out_rows() as i32 - 1));
    }
    // * F12 (0x94) NO esta aqui: se atiende arriba, antes de
    // preguntar por el foco, porque es del sistema y no de esta
    // ventana. Ver la conmutacion de la consola de datos.
    //
    // El resto de navegacion se ignora, pero EXPLICITAMENTE:
    // dejarlas caer al comodin las dibujaria como basura.
    0x89..=0x9F => {}
    // Todo lo demas imprimible, incluido el Latin-1 alto: la
    // `n` llega como 0xF1 y la fuente la tiene.
    c if c >= 0x20 => {
        if dsk.field.n < PATH_MAX {
            // Hueco en el cursor y meter ahi: escribir en
            // medio de una linea es lo normal, no un caso raro.
            let mut k = dsk.field.n;
            while k > dsk.field.cur {
                dsk.field.path[k] = dsk.field.path[k - 1];
                k -= 1;
            }
            dsk.field.path[dsk.field.cur] = c;
            dsk.field.cur += 1;
            dsk.field.n += 1;
            dsk.tick.repaint_field = true;
        }
    }
    _ => {}
}
    Edit::Taken
}
