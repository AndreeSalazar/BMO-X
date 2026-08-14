//! **El vigilante de la corrida**: saber que el programa lanzado termino, y
//! guardar lo que dijo.
//!
//! ## Por que esto sale de `_start`
//!
//! El `_start` del compositor son **1960 lineas en una sola funcion**, y no es
//! un problema de longitud: es un problema de ESTADO. Treinta y cinco variables
//! viven la vuelta entera, y el bloque de teclado toca veintiseis de ellas. Por
//! eso no se parte extrayendo funciones a ojo -- la mayoria de los bloques se
//! llevarian veinte parametros, que es mover el problema a la firma.
//!
//! Este no. Toca **tres**: la corrida, la consola del hijo y la salida. Es el
//! corte que se puede hacer sin inventar nada, y por eso es el primero.
//!
//! * Y se lleva algo mas: dentro de este bloque vivian las **dos sombras** del
//! fichero -- un `let n` y un `let mut frames` que tapaban a las variables del
//! mismo nombre del prologo. Con el bloque fuera, `n` y `frames` dejan de estar
//! ensombrecidas en `_start`, que es justo lo que hacia insegura una renombrada
//! mecanica del estado. Esto no es limpieza: es **desbloquear el corte
//! siguiente**.

use crate::scene::output::{Output, INK_GOOD, INK_ERR, INK_PLAIN};
use crate::commands::complete::file_error_reason;
use bmo_userland as bmo;

/// Un programa lanzado del que todavia se espera el final.
pub(crate) struct Run {
    pub mark: usize,
    /// Cuantos fotogramas han pasado desde que se lanzo.
    ///
    /// ** Esto era una bandera `visto` que exigia **haber visto al hijo
    /// vivo** antes de volcar, y ahi estaba el fallo que costo tres
    /// sesiones: un programa que arranca y termina ENTRE DOS FOTOGRAMAS no
    /// se le ve vivo nunca, asi que no volcaba jamas.
    ///
    /// La evidencia lo dijo entera desde el disco: de todo lo que se corrio,
    /// **el unico que dejo su `.txt` fue `c/pregc.bex`** -- el unico
    /// interactivo, el unico que se pasa segundos esperando a que teclees.
    /// Los diez de COBOL y `memc` duran milisegundos y no volcaron ni uno.
    ///
    /// Con un contador no hace falta ver nada: se espera un par de vueltas
    /// --el margen que necesita el lanzamiento para registrarse-- y a partir
    /// de ahi, *no hay hijo* significa *termino*.
    pub waits: u32,
    /// `data/<programa>.txt`, ya montada al lanzar.
    pub dest: [u8; 32],
    pub dest_n: usize,
}

/// Cuantas lecturas de 8 bytes como mucho al drenar. Alto, pero existe: un
/// programa que muere dejando megabytes no puede quedarse con el bucle.
const DRAIN_MAX: u32 = 8192;

/// Termino el programa que se lanzo? Entonces, a guardarlo.
///
/// Se llama una vez por fotograma, lo primero.
pub(crate) fn watch_run(
    run: &mut Option<Run>,
    child_console: &Option<bmo::Consola>,
    output: &mut Output,
) {
    let Some(c) = run.as_mut() else { return };

    c.waits = c.waits.saturating_add(1);
    let alive_one = child_console.as_ref().map(|cc| cc.has_child()).unwrap_or(false);
    // Mientras haya hijo, no ha terminado. Y las dos primeras vueltas
    // no cuentan: son el margen que necesita `ejecutar_en` para que el
    // kernel registre al hijo en la tabla de la consola. Sin ese
    // margen se volcaria un archivo vacio en el acto.
    if alive_one || c.waits <= 2 {
        return;
    }

    // ** SE DRENA ANTES DE GUARDAR, y esto lo enseno el disco.
    //
    // El primer `output.txt` que llego a Windows tenia las cuatro
    // lineas del ECO y **ni una del programa**. El motivo: este
    // vigilante corre al principio del fotograma y el drenado de la
    // consola del hijo esta mucho mas abajo. Cuando `has_child()`
    // dice que no, lo ultimo que escribio el programa **sigue en el
    // anillo del kernel** -- se guardaba un archivo de lo que el
    // terminal habia dicho, no de lo que habia contestado.
    //
    // Aqui se vacia el anillo entero antes de tocar el disco.
    if let Some(cc) = child_console.as_ref() {
        let mut buf = [0u8; 8];
        let mut reads = 0u32;
        while reads < DRAIN_MAX {
            let read_bytes = cc.read(&mut buf);
            if read_bytes == 0 {
                break;
            }
            output.text(&buf[..read_bytes]);
            reads += 1;
        }
    }

    let path_n = c.dest_n;
    let path = c.dest;
    let (from, to) = output.rows_since(c.mark);
    match crate::dump_output(output, &path[..path_n], from, to) {
        Ok(_) => {
            output.with_ink(INK_GOOD);
            output.text(b"  [salida guardada en ");
            output.text(&path[..path_n]);
            output.text(b"]\n");
            output.with_ink(INK_PLAIN);
        }
        // Se dice y no se calla, **y se dice QUE y POR QUE**. La
        // primera version ponia "no se pudo guardar, F11 dice por
        // que", y eso obliga a abrir otra ventana para saber lo que
        // el mensaje ya tenia en la mano. Un error que manda a otro
        // sitio a buscar el motivo es medio error.
        Err(e) => {
            output.with_ink(INK_ERR);
            output.text(b"  [NO se guardo ");
            output.text(&path[..path_n]);
            output.text(b": ");
            if e == 0 {
                // El cero es el `close` que contesta que no. El
                // kernel no dice mas, y eso tambien se dice.
                output.text(b"el cierre fallo (disco lleno? no cabe?)");
            } else {
                output.text(file_error_reason(e));
            }
            output.text(b"]\n");
            output.with_ink(INK_PLAIN);
        }
    }

    *run = None;
}
