//! **EL ARCHIVO QUE SE ESTA TRAYENDO** -- la carga por trozos de una ranura.
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! === Por que es un fichero y no un trozo de `file.rs` ===
//!
//! Por L6a: `obj/file.rs` estaba en 1.026 lineas, en la linea base del censo, y
//! **no podia crecer ni una**. Antes de anadirle la puerta de ESTRATOS habia
//! que hacerle sitio.
//!
//! Pero el corte no se eligio por tamano. Se eligio porque esto es un
//! **ciclo de vida** --un archivo que todavia no ha llegado entero-- y no un
//! camino de datos: sus dos `static` no los mira nadie mas, tenia ya su propio
//! comentario de seccion, y ninguna de sus funciones se llama desde fuera del
//! modulo. Nombres libres, que es como se eligen los cortes en esta casa.
//!
//! === Que resuelve ===
//!
//! `open` lee el fichero ENTERO antes de devolver el handle. Para un `.txt` no
//! se nota; para un `.bex` de 813 KB, el que lo pidio no existe durante toda la
//! lectura -- y si el que lo pidio es el escritorio, el escritorio no pinta.
//!
//! `abrir_asinc` --que se queda en `file.rs`, porque lo suyo es repartir
//! ranuras-- devuelve el handle **en cuanto sabe que el archivo existe**, y los
//! bytes van llegando en trozos por aqui.
//!
//! === Por que el avance lo empuja el que PREGUNTA ===
//!
//! La alternativa era seguir la cadena desde el manejador de interrupcion. No:
//! seguir una cadena FAT es leer mas disco --la propia tabla-- asi que seria
//! pedir E/S desde dentro de la interrupcion de E/S. Empujando desde la
//! pregunta, el trabajo ocurre en el turno de quien lo quiere, que es de quien
//! es.
//!
//! [!] Esto es FAT32 y solo FAT32, y esta bien que lo sea: un fichero de
//! ESTRATOS no se trae por trozos de una cadena, se reconstruye por su arbol de
//! atributos. Cuando ESTRATOS entre por la puerta de `file.rs` no pasara por
//! aqui.

use super::file::{buf, LARGO, MAX_ABIERTOS};

/// Cluster por el que va la carga. `0` = no hay carga en curso.
pub(super) static mut LOAD_CLUSTER: [u32; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// Lo que mide el archivo entero, mientras se trae.
///
/// * Aqui NO hay un contador de trozos. Se escribio, para que el `wait` supiera
/// si habia habido progreso desde la ultima mirada -- y como ese `wait` no
/// llego a existir (ver `syscall.rs`), el contador se quedaba subiendo para
/// nadie. Lo que hace falta saber de fuera --cuanto ha llegado-- ya lo dice
/// `LARGO`, y lo contesta `ARCH_OP_LISTO`.
pub(super) static mut LOAD_TOTAL: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];

/// Cuanto se trae de una vez.
///
/// 128 KiB: bastante para que un archivo normal llegue en una o dos vueltas, y
/// poco para que el kernel no se quede dentro mas de lo que dura un turno. El
/// numero correcto se sabra midiendo en metal; este es el que no estorba.
const TROZO: usize = 128 * 1024;

/// Se esta trayendo todavia?
pub(super) fn hay(i: usize) -> bool {
    unsafe { i < MAX_ABIERTOS && LOAD_CLUSTER[i] != 0 }
}

/// **Trae el siguiente trozo.** `true` si el archivo ya esta entero.
///
/// Lo llama cualquier operacion sobre el handle: preguntar por el archivo ES lo
/// que lo hace avanzar. Si no habia carga en curso, contesta que si -- un
/// archivo que ya estaba entero lo esta igual despues de preguntar.
pub(super) fn avanzar(i: usize) -> bool {
    if !hay(i) {
        return true;
    }
    unsafe {
        let cluster = LOAD_CLUSTER[i];
        let total = LOAD_TOTAL[i];
        let ya = LARGO[i];
        let dst = buf(i);
        let (leidos, siguiente) =
            crate::ring0::fsys::fs::leer_trozo(cluster, ya, total as u32, dst, TROZO);
        LARGO[i] = ya + leidos;
        // `siguiente == 0` es fin -- de la cadena o del archivo. Y `leidos == 0`
        // tambien corta: un tramo que no avanza dos veces seguidas seria un
        // bucle infinito en el que pregunta, y prefiero un archivo corto que se
        // nota a una maquina que no vuelve.
        if siguiente == 0 || leidos == 0 {
            LOAD_CLUSTER[i] = 0;
            crate::ring0::cabina::info("arch", "archivo completo", LARGO[i] as u64);
            return true;
        }
        LOAD_CLUSTER[i] = siguiente;
        false
    }
}

