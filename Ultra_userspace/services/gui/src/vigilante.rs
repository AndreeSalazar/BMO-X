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
//! fichero -- un `let n` y un `let mut vueltas` que tapaban a las variables del
//! mismo nombre del prologo. Con el bloque fuera, `n` y `vueltas` dejan de estar
//! ensombrecidas en `_start`, que es justo lo que hacia insegura una renombrada
//! mecanica del estado. Esto no es limpieza: es **desbloquear el corte
//! siguiente**.

use crate::escena::salida::{Salida, TINTA_BIEN, TINTA_MAL, TINTA_NORMAL};
use crate::ordenes::complete::motivo_archivo;
use bmo_userland as bmo;

/// Un programa lanzado del que todavia se espera el final.
pub(crate) struct Corrida {
    pub marca: usize,
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
    pub esperas: u32,
    /// `datos/<programa>.txt`, ya montada al lanzar.
    pub destino: [u8; 32],
    pub destino_n: usize,
}

/// Cuantas lecturas de 8 bytes como mucho al drenar. Alto, pero existe: un
/// programa que muere dejando megabytes no puede quedarse con el bucle.
const DRENADO_MAX: u32 = 8192;

/// Termino el programa que se lanzo? Entonces, a guardarlo.
///
/// Se llama una vez por fotograma, lo primero.
pub(crate) fn vigilar_corrida(
    corrida: &mut Option<Corrida>,
    salida_cap: &Option<bmo::Consola>,
    salida: &mut Salida,
) {
    let Some(c) = corrida.as_mut() else { return };

    c.esperas = c.esperas.saturating_add(1);
    let vivo = salida_cap.as_ref().map(|cc| cc.has_child()).unwrap_or(false);
    // Mientras haya hijo, no ha terminado. Y las dos primeras vueltas
    // no cuentan: son el margen que necesita `ejecutar_en` para que el
    // kernel registre al hijo en la tabla de la consola. Sin ese
    // margen se volcaria un archivo vacio en el acto.
    if vivo || c.esperas <= 2 {
        return;
    }

    // ** SE DRENA ANTES DE GUARDAR, y esto lo enseno el disco.
    //
    // El primer `salida.txt` que llego a Windows tenia las cuatro
    // lineas del ECO y **ni una del programa**. El motivo: este
    // vigilante corre al principio del fotograma y el drenado de la
    // consola del hijo esta mucho mas abajo. Cuando `has_child()`
    // dice que no, lo ultimo que escribio el programa **sigue en el
    // anillo del kernel** -- se guardaba un archivo de lo que el
    // terminal habia dicho, no de lo que habia contestado.
    //
    // Aqui se vacia el anillo entero antes de tocar el disco.
    if let Some(cc) = salida_cap.as_ref() {
        let mut buf = [0u8; 8];
        let mut leidas = 0u32;
        while leidas < DRENADO_MAX {
            let leidos = cc.read(&mut buf);
            if leidos == 0 {
                break;
            }
            salida.texto(&buf[..leidos]);
            leidas += 1;
        }
    }

    let ruta_n = c.destino_n;
    let ruta = c.destino;
    let (desde, hasta) = salida.filas_desde(c.marca);
    match crate::volcar_salida(salida, &ruta[..ruta_n], desde, hasta) {
        Ok(_) => {
            salida.con_tinta(TINTA_BIEN);
            salida.texto(b"  [salida guardada en ");
            salida.texto(&ruta[..ruta_n]);
            salida.texto(b"]\n");
            salida.con_tinta(TINTA_NORMAL);
        }
        // Se dice y no se calla, **y se dice QUE y POR QUE**. La
        // primera version ponia "no se pudo guardar, F11 dice por
        // que", y eso obliga a abrir otra ventana para saber lo que
        // el mensaje ya tenia en la mano. Un error que manda a otro
        // sitio a buscar el motivo es medio error.
        Err(e) => {
            salida.con_tinta(TINTA_MAL);
            salida.texto(b"  [NO se guardo ");
            salida.texto(&ruta[..ruta_n]);
            salida.texto(b": ");
            if e == 0 {
                // El cero es el `close` que contesta que no. El
                // kernel no dice mas, y eso tambien se dice.
                salida.texto(b"el cierre fallo (disco lleno? no cabe?)");
            } else {
                salida.texto(motivo_archivo(e));
            }
            salida.texto(b"]\n");
            salida.con_tinta(TINTA_NORMAL);
        }
    }

    *corrida = None;
}
