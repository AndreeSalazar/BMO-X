//! **Commands that talk to the DISK**: `ls`, `lee`, `escribe`, `guarda`.
//!
//! They are together because they share the failure they can hit -- a
//! capability that is not granted, a FAT32 name that does not fit in 8.3, a
//! close that fails after every write succeeded. `file_error_reason` says
//! which one, and it is next door on purpose.

use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::commands::complete::file_error_reason;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_BAD, INK_DIM, INK_OK};
use crate::text::{decimal, is_dot_entry};
use crate::{dump_output, DEFAULT_DUMP};

pub(crate) fn list(dsk: &mut Desktop, p: &bmo::Pantalla, dir_path: &[u8]) -> After {
    match bmo::Directorio::open(dir_path) {
        Ok(d) => {
            let mut count = 0u32;
            // Tope por si un directorio enorme se
            // comiera el fotograma entero.
            while count < 256 {
                let e = match d.next() {
                    Some(e) => e,
                    None => break,
                };
                let mut nom = [0u8; 12];
                let length = e.legible(&mut nom);
                // `.` y `..` no se ensenan: aqui
                // no hay carpeta actual a la que
                // volver, asi que son ruido.
                if is_dot_entry(&nom[..length]) { return After::NextKey; }
                dsk.out.grid.text(b"  ");
                dsk.out.grid.text(&nom[..length]);
                // Alinear la columna del tamano.
                let mut k = length;
                while k < 14 { dsk.out.grid.byte(b' '); k += 1; }
                if e.es_dir {
                    dsk.out.grid.text(b"<DIR>");
                } else {
                    let mut d10 = [0u8; 10];
                    let n10 = decimal(e.bytes as u64, &mut d10);
                    dsk.out.grid.text(&d10[..n10]);
                }
                dsk.out.grid.byte(b'\n');
                count += 1;
            }
            if count == 0 {
                dsk.out.grid.text(b"  (vacio)
");
            }
            paint_status(&p, &dsk.run_box, "listo", INK_DIM);
        }
        // * El MOTIVO, no un "no pude" para todo.
        //
        // Esto tiraba el codigo con `Err(_)` y
        // decia siempre "no puedo abrir esa
        // carpeta". Cuando la tabla de directorios
        // del kernel se lleno, eso fue una mentira
        // exacta: la carpeta estaba ahi, lo que no
        // habia era ranura. Y mando a buscar el
        // fallo al disco, que estaba perfecto.
        //
        // Un error que no distingue sus causas es
        // un error que manda a mirar donde no es.
        Err(cod) => {
            // 25 = sin hueco, 26 = no esta. Ver
            // `ring0/obj/directorio.rs`.
            let (line, estado): (&[u8], &str) = if cod == 25 {
                (
                    b"  no queda slot de directorio en el kernel.\n",
                    "sin ranura libre",
                )
            } else {
                (
                    b"  no puedo open esa carpeta.\n",
                    "carpeta no encontrada",
                )
            };
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(line);
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, estado, INK_BAD);
        }
    }
    dsk.field.n = 0;
    After::Settle
}

/// -- Leer un archivo --
///
/// El hermano de `ls`: aquel dice QUE hay, este
/// ensena lo de DENTRO. Es la primera vez que un
/// programa de Ring 3 abre un archivo del disco.
pub(crate) fn read(dsk: &mut Desktop, p: &bmo::Pantalla, file_path: &[u8]) -> After {
    match bmo::Archivo::leer_de(file_path) {
        Ok(a) => {
            let mut chunk = [0u8; 256];
            let mut total = 0usize;
            // El ultimo byte se guarda segun pasa:
            // reconstruirlo al final obligaria a
            // saber en que trozo cayo, y el buffer
            // ya se ha reutilizado.
            let mut last = 0u8;
            // De 256 en 256 y con tope: un archivo
            // que no sea texto llenaria la rejilla
            // de basura y se comeria el fotograma.
            loop {
                let got = a.read(&mut chunk);
                if got == 0 { break; }
                dsk.out.grid.text(&chunk[..got]);
                last = chunk[dsk.field.n - 1];
                total += dsk.field.n;
                if total >= 2048 {
                    dsk.out.grid.text(b"\n  ...(cortado)\n");
                    last = b'\n';
                    break;
                }
            }
            if total == 0 {
                dsk.out.grid.text(b"  (vacio)\n");
            } else if last != b'\n' {
                // Sin esto, el proximo mensaje se
                // pega al final del archivo.
                dsk.out.grid.byte(b'\n');
            }
            a.close();
            paint_status(&p, &dsk.run_box, "listo", INK_DIM);
        }
        Err(e) => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  ");
            dsk.out.grid.text(file_error_reason(e));
            dsk.out.grid.byte(b'\n');
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "no se pudo leer", INK_BAD);
        }
    }
    dsk.field.n = 0;
    After::Settle
}

/// -- Escribir un archivo --
///
/// Lo que NUNCA habia pasado: un programa de Ring 3
/// dejando algo en el disco. Hasta hoy todo lo que
/// habia ahi lo puso el anfitrion al flashear o el
/// kernel con su caja negra.
pub(crate) fn write(dsk: &mut Desktop, p: &bmo::Pantalla, file_path: &[u8], text: &[u8]) -> After {
    match bmo::Archivo::create(file_path) {
        Ok(a) => {
            let placed = a.write(text);
            // El salto final: un archivo de texto
            // sin el ultimo salto es el clasico
            // que descuadra al siguiente que lo lee.
            a.write(b"\n");
            // * Aqui es donde llega al disco. Antes
            // de esto no hay nada escrito.
            if a.close() {
                dsk.out.grid.text(b"  guardado: ");
                let mut d10 = [0u8; 10];
                let n10 = decimal(placed as u64 + 1, &mut d10);
                dsk.out.grid.text(&d10[..n10]);
                dsk.out.grid.text(b" bytes\n");
                paint_status(&p, &dsk.run_box, "guardado", INK_OK);
            } else {
                dsk.out.grid.text(b"  no se guardo nada.\n");
                paint_status(&p, &dsk.run_box, "no se pudo guardar", INK_BAD);
            }
        }
        Err(e) => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  ");
            dsk.out.grid.text(file_error_reason(e));
            dsk.out.grid.byte(b'\n');
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "no se pudo crear", INK_BAD);
        }
    }
    dsk.field.n = 0;
    After::Settle
}

/// -- Volcar el historial a un .txt --
///
/// El hermano manual del volcado automatico: aquel
/// guarda lo de UNA corrida, este guarda todo lo que
/// quede en el historial, que es lo que hace falta
/// cuando lo interesante son tres comandos juntos.
pub(crate) fn save(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
    let dest = if arg.is_empty() { DEFAULT_DUMP } else { arg };
    // ** LA TABLA DE CONSUMO VA DENTRO DEL VOLCADO, y va ANTES de tomar el
    // rango para que entre en el fichero.
    //
    // El motivo es el bucle de trabajo real: este `.txt` es lo unico que cruza
    // del Ryzen al otro lado, y hasta hoy llegaba contando lo que hizo el
    // programa **sin decir en que estado estaba la maquina**. Cada vez que hacia
    // falta esa mitad --cuanta RAM quedaba, a que reloj iba, cuantos nucleos en
    // pie-- habia que pedir otro arranque con un `info` puesto a mano.
    //
    // Ahora todo volcado la lleva. Cuesta veinte lineas de texto y ahorra un
    // viaje entero, que en esta maquina se mide en reinicios.
    //
    // [!] Y por eso se pinta aqui y no en `dump_output`: lo que se guarda es el
    // historial de la PANTALLA, asi que para que algo salga en el fichero tiene
    // que estar antes en la pantalla. Escribirlo solo al fichero seria tener dos
    // caminos de salida que pueden decir cosas distintas.
    super::reports::report_consumo(&mut dsk.out.grid);
    // El rango se toma ANTES de escribir nada:
    // los mensajes de abajo son de esta orden, no
    // de lo que se estaba guardando, y colarlos
    // dentro haria que el archivo hablara de si
    // mismo.
    let (from, to) = dsk.out.grid.all_rows();
    match dump_output(&dsk.out.grid, dest, from, to) {
        Ok(bytes) => {
            dsk.out.grid.with_ink(INK_GOOD);
            dsk.out.grid.text(b"  guardado en ");
            dsk.out.grid.text(dest);
            dsk.out.grid.text(b": ");
            let mut d = [0u8; 10];
            let k = decimal(bytes as u64, &mut d);
            dsk.out.grid.text(&d[..k]);
            dsk.out.grid.text(b" bytes, ");
            let k = decimal((to - from + 1) as u64, &mut d);
            dsk.out.grid.text(&d[..k]);
            dsk.out.grid.text(b" lineas\n");
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "volcado", INK_OK);
        }
        Err(0) => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  no se guardo nada. el motivo esta en F11.\n");
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "no se pudo guardar", INK_BAD);
        }
        Err(e) => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  ");
            dsk.out.grid.text(file_error_reason(e));
            dsk.out.grid.byte(b'\n');
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "no se pudo crear", INK_BAD);
        }
    }
    dsk.field.n = 0;
    After::Settle
}
