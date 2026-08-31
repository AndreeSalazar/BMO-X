//! **EL BARRIDO: lo que NO se puede escapar de ningun filtro.**
//!
//! [carril]  AMARILLO  los filtros; uno de mas y el evento que importaba no sale
//!
//! # Los dos agujeros de un anillo con filtro, y ninguno se ve
//!
//! CABINA graba en un anillo de **48** eventos y `cabina fallos` filtra lo que
//! queda dentro. Eso deja dos formas de que un evento desaparezca **sin que
//! nadie se entere**:
//!
//! ```text
//!    1. SE CAE DEL ANILLO   el evento 49 empuja al 1 fuera. Un FAULT del
//!                           arranque ya no existe cuando llegas a mirar
//!    2. REENTRANCIA         si `record` se llama desde dentro de `record`
//!                           --una IRQ encima de un fault-- el segundo se
//!                           descarta para no partir el anillo
//! ```
//!
//! *** **Y filtrar no arregla ninguno de los dos: un filtro solo puede mirar lo
//! que sobrevivio.** Preguntar *"ensename los fallos"* sobre un anillo que ya
//! perdio el fallo contesta *"ninguno"* -- que es la respuesta mas cara que
//! puede dar un sistema de vigilancia, porque **es indistinguible de estar
//! bien**.
//!
//! > Un radar que pierde un contacto y dibuja la pantalla vacia no es un radar
//! > con menos alcance. Es una pantalla.
//!
//! # La respuesta: contar en el ORIGEN, antes de que se pueda perder
//!
//! Aqui hay una cuenta por **capa x severidad** --8 x 5 = 40-- y **ninguna
//! gira**. Se incrementa en `record` **antes** del cerrojo del anillo, asi que:
//!
//! ```text
//!    el evento se cae del anillo   -> la cuenta lo sigue teniendo
//!    el evento se pierde por BUSY  -> la cuenta lo sigue teniendo
//! ```
//!
//! *** Y eso parte la vigilancia en dos cosas que ya no se estorban:
//!
//! ```text
//!    el ANILLO       el DETALLE de lo reciente. 48, y gira
//!    el BARRIDO      CUANTOS hubo de cada clase. No gira NUNCA
//! ```
//!
//! **Lo que se pierde del anillo no se pierde de la cuenta**, y esa es toda la
//! diferencia entre una ventana y un radar.
//!
//! # * Y ademas dice SI TODAVIA SE PUEDE VER
//!
//! Cada clase guarda el `seq` de su **ultimo** evento. Con la ventana del anillo
//! --el `seq` mas bajo que sigue dentro-- se contesta la pregunta que de verdad
//! importa cuando algo va mal:
//!
//! ```text
//!    hubo 3 FAULT de `sec`, y el ultimo es el #412
//!    la ventana del anillo empieza en el #480
//!    -> los TRES estan fuera. Sabes que pasaron y NO puedes leerlos
//! ```
//!
//! *** Saber que hay tres fallos que **ya no se pueden leer** es un dato
//! accionable --vuelca antes, o sube el anillo--. Creer que no hubo ninguno no
//! lo es.
//!
//! # Por que atomicos y no el mismo cerrojo que el anillo
//!
//! Porque el cerrojo es exactamente lo que hace que la reentrancia pierda. Un
//! `fetch_add` no necesita cerrojo, asi que **puede correr dentro de la
//! reentrancia** -- que es el unico sitio donde el anillo no puede.
//!
//! [!] Y no se hace `Ordering::SeqCst`: son contadores para mirar, no para
//! sincronizar nada. `Relaxed` al sumar y `Relaxed` al leer es lo correcto y lo
//! barato, y esto corre en el camino de CADA evento del kernel.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use cabina_core::{Layer, Severity};

/// Cuantas severidades hay: `Info`, `Trace`, `Warning`, `Fault`, `Panic`.
pub const SEVERIDADES: usize = 5;
/// Cuantas capas. Lo declara `cabina_core::Layer`.
pub const CAPAS: usize = Layer::COUNT;

#[allow(clippy::declare_interior_mutable_const)]
const C32: AtomicU32 = AtomicU32::new(0);
#[allow(clippy::declare_interior_mutable_const)]
const C64: AtomicU64 = AtomicU64::new(0);
#[allow(clippy::declare_interior_mutable_const)]
const FILA32: [AtomicU32; SEVERIDADES] = [C32; SEVERIDADES];
#[allow(clippy::declare_interior_mutable_const)]
const FILA64: [AtomicU64; SEVERIDADES] = [C64; SEVERIDADES];

/// **Cuantos hubo de cada clase. Nunca baja y nunca gira.**
static CUENTA: [[AtomicU32; SEVERIDADES]; CAPAS] = [FILA32; CAPAS];
/// El `seq` del ultimo de cada clase. `0` = no hubo ninguno.
static ULTIMO: [[AtomicU64; SEVERIDADES]; CAPAS] = [FILA64; CAPAS];

// -- *** EL RITMO: cuantos POR SEGUNDO, y no cuantos desde el arranque -----
//
// # El agujero que esto tapa, y no se ve mirando el barrido
//
// `CUENTA` no gira nunca, y esa es su virtud: lo que paso sigue contado. Pero
// tiene el defecto exacto de esa virtud -- **un total no puede decir "esta
// pasando AHORA"**:
//
// ```text
//    cuenta = 400 FAULT de `vmm`     puede ser
//                                      - una tanda de hace media hora, resuelta
//                                      - cuatrocientos por segundo, ahora mismo
// ```
//
// *** Y esas dos cosas piden lo contrario: la primera es forense --se mira
// despues-- y la segunda es una emergencia. **Un numero que no distingue una
// emergencia de un recuerdo no sirve para actuar**, solo para contarlo luego.
//
// Es literalmente el limite que tenia la patada: sabe reaccionar a UN suceso
// (`vmm::caminable` falla), y no sabe ver una tormenta.
//
// # Como se saca sin poner nada en el camino
//
// Restando. Una vez por segundo se copia `CUENTA` a `ANTERIOR` y la diferencia
// es `RITMO`. **Son 40 restas por segundo**: al lado del `fetch_add` que ya se
// paga en cada evento, esto es gratis.
//
// [!] Y lo hace el hilo del bus, no el camino del evento. Es la misma regla de
// siempre: quien ve el hecho lo APUNTA; quien puede pensar lo recoge en su
// turno. Ver `dev/usb/bus.rs`.

/// Lo que valia `CUENTA` en el ultimo cierre de ventana.
static ANTERIOR: [[AtomicU32; SEVERIDADES]; CAPAS] = [FILA32; CAPAS];
/// Cuantos hubo de cada clase en el ULTIMO segundo cerrado.
static RITMO: [[AtomicU32; SEVERIDADES]; CAPAS] = [FILA32; CAPAS];
/// El TSC del ultimo cierre. `0` = todavia no se ha cerrado ninguna ventana.
static ULTIMO_CIERRE: AtomicU64 = AtomicU64::new(0);
/// Cuantas ventanas se han cerrado. Sin esto, un ritmo de cero no distingue
/// *"no paso nada"* de *"la ventana no ha llegado a cerrarse todavia"*.
static VENTANAS: AtomicU32 = AtomicU32::new(0);

/// **Cierra la ventana del segundo, si ha pasado uno.** Devuelve `true` si la
/// cerro.
///
/// `hz` es la frecuencia del TSC. **Si vale cero no se cierra nada**: sin reloj
/// medido no hay segundo que medir, y un ritmo calculado sobre un intervalo
/// desconocido es un numero inventado con cara de dato. Es la regla de los
/// jueces de esta casa -- cuando falta el dato, la respuesta es la que no asume.
pub fn cerrar_ventana(ahora: u64, hz: u64) -> bool {
    if hz == 0 {
        return false;
    }
    let previo = ULTIMO_CIERRE.load(Ordering::Relaxed);
    if previo == 0 {
        // La primera vez solo se marca el instante: no hay intervalo anterior
        // contra el que restar, y publicar el total del arranque como si fuera
        // un ritmo diria que la maquina esta ardiendo en su primer segundo.
        ULTIMO_CIERRE.store(ahora, Ordering::Relaxed);
        return false;
    }
    if ahora.wrapping_sub(previo) < hz {
        return false;
    }
    for c in 0..CAPAS {
        for s in 0..SEVERIDADES {
            let total = CUENTA[c][s].load(Ordering::Relaxed);
            let antes = ANTERIOR[c][s].swap(total, Ordering::Relaxed);
            RITMO[c][s].store(total.wrapping_sub(antes), Ordering::Relaxed);
        }
    }
    ULTIMO_CIERRE.store(ahora, Ordering::Relaxed);
    VENTANAS.fetch_add(1, Ordering::Relaxed);
    true
}

/// Cuantos eventos de esta clase hubo en el **ultimo segundo cerrado**.
pub fn ritmo(capa: usize, sev: usize) -> u64 {
    match RITMO.get(capa).and_then(|f| f.get(sev)) {
        Some(c) => c.load(Ordering::Relaxed) as u64,
        None => 0,
    }
}

/// Cuantas ventanas de un segundo se han cerrado desde el arranque.
///
/// ** Se publica porque **`ritmo() == 0` es ambiguo sin esto**: puede ser "no
/// paso nada en el ultimo segundo" o "todavia no ha pasado un segundo entero".
/// La primera es tranquilizadora y la segunda no dice nada, y un panel que las
/// pinte igual esta mintiendo la mitad de las veces.
pub fn ventanas() -> u64 {
    VENTANAS.load(Ordering::Relaxed) as u64
}

/// **Apunta un evento en el barrido.** Se llama desde `record` **antes** del
/// cerrojo del anillo, y por eso cuenta tambien lo que el anillo va a perder.
///
/// `seq` es el numero de secuencia que el anillo le va a dar. Se pasa en vez de
/// leerlo aqui porque **el que lo genera es el anillo** y dos sitios generando
/// numeros de secuencia son dos series que se separan.
pub fn apunta(sev: Severity, capa: Layer, seq: u64) {
    let (c, s) = (capa as usize, sev as usize);
    if c >= CAPAS || s >= SEVERIDADES {
        return;
    }
    CUENTA[c][s].fetch_add(1, Ordering::Relaxed);
    ULTIMO[c][s].store(seq, Ordering::Relaxed);
}

/// Cuantos eventos de esta clase hubo **desde el arranque**.
pub fn cuenta(capa: usize, sev: usize) -> u64 {
    match CUENTA.get(capa).and_then(|f| f.get(sev)) {
        Some(c) => c.load(Ordering::Relaxed) as u64,
        None => 0,
    }
}

/// El `seq` del ultimo de esta clase, o `0` si no hubo ninguno.
///
/// Con esto y la ventana del anillo se sabe si **todavia se puede leer**.
pub fn ultimo(capa: usize, sev: usize) -> u64 {
    match ULTIMO.get(capa).and_then(|f| f.get(sev)) {
        Some(c) => c.load(Ordering::Relaxed),
        None => 0,
    }
}

/// **Cuantas clases tienen algo que ya no se puede leer.**
///
/// Es el numero que resume el barrido en una linea: si es `0`, todo lo que paso
/// sigue en el anillo. Si no, hay sucesos de los que solo queda la cuenta.
pub fn clases_fuera_de_ventana(primer_seq_visible: u64) -> u64 {
    let mut n = 0;
    for c in 0..CAPAS {
        for s in 0..SEVERIDADES {
            let u = ultimo(c, s);
            // `u != 0` -> hubo alguno. `u < ventana` -> el ULTIMO ya se cayo,
            // asi que todos los de esa clase se cayeron.
            if u != 0 && u < primer_seq_visible {
                n += 1;
            }
        }
    }
    n
}
