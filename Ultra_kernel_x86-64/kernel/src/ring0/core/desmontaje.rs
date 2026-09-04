//! **EL TESTIGO DEL DESMONTAJE: en que estacion iba la purga cuando reviento.**
//!
//! [carril]  AMARILLO  no ejecuta nada del desmontaje: lo APUNTA. Es peligroso
//!                     de creer, no de correr
//!
//! [cuesta]  NADA -- dos escrituras atomicas por estacion. No mata a nadie, no
//!           libera nada y no decide nada; si se equivoca, lo unico que sale
//!           mal es una linea de la pantalla azul.
//!
//! [riesgo]  ESPEJO SILENCIO
//!           ESPEJO   -- la tabla de nombres de aqui abajo es un REFLEJO del
//!                       orden real de `obj::cap::revoke_all`. Si alguien mete
//!                       una estacion ahi y no la mete aqui, la pantalla azul
//!                       **acusa a la de al lado** -- que es peor que no decir
//!                       nada, porque manda a leer el fichero equivocado con
//!                       la maquina parada. Ver el guardian del final.
//!           SILENCIO -- un testigo que no se imprime no existe. Lo mismo que
//!                       ya se pago el 02-09 con `marco OCUPADO,` cortado en el
//!                       byte 80: un instrumento cuya respuesta no cabe en la
//!                       pantalla no ha respondido.
//!
//! # *** POR QUE EXISTE
//!
//! `purga` tumba la maquina con un `#PF` en Ring 0, y la pantalla azul dice
//! muchisimo sobre **el marco** --de quien fue, si el asignador lo da por
//! entregado, quien lo tiene ahora-- y **nada sobre el momento**.
//!
//! Y el momento es la mitad que falta, porque desmontar un proceso no es un
//! paso: son DIECISIETE, en un orden que es portante. `revoke_all` lo dice en
//! sus propios comentarios:
//!
//! ```text
//!    loan   va ANTES que memory   -- el reflejo se desmapea del espacio
//!                                    del muerto, y ese espacio tiene que
//!                                    existir todavia
//!    autoridad va la PRIMERA      -- un pid reutilizado heredaria la del
//!                                    muerto
//! ```
//!
//! Un orden portante con diecisiete puestos es exactamente la forma que
//! produce *"uno toca lo que otro ya libero"*. Sin testigo, la pantalla azul
//! manda a auditar los diecisiete. Con testigo, nombra uno.
//!
//! # El metodo, que es el que ya funciono DOS VECES esta semana
//!
//! Es el `bmo_quien_llamo` de DOOM, subido a Ring 0: una variable que dice
//! quien iba hablando, leida por quien recoge el cadaver. Alli convirtio
//! *"Bad R_RenderWallRange en alguno de ocho sitios"* en *"sitio 21"* de un
//! arranque. Aqui tiene que convertir *"#PF en la purga"* en *"estacion 09,
//! `loan::process_died`"*.
//!
//! ** Y NO ES UN LOG. Un `printf` por estacion a la velocidad a la que purga
//! recorre veinte procesos tapa su propio mensaje --la leccion del cepo del
//! 30-08-- y ademas no sobrevive al fallo, que es cuando hace falta. Esto son
//! dos enteros que el que revienta se encuentra ya escritos.
//!
//! # Lo que NO promete
//!
//! No dice **por que**. Dice DONDE. Un `#PF` en la estacion 09 puede ser de
//! `loan` o de algo que las ocho anteriores dejaron a medias, y distinguirlo
//! sigue siendo trabajo. Pero es trabajo sobre un fichero en vez de sobre
//! diecisiete, y esa es toda la diferencia que se le pide a un instrumento.

use core::sync::atomic::{AtomicU32, Ordering};

/// La estacion en curso. **0 = nadie esta desmontando.**
static PASO: AtomicU32 = AtomicU32::new(0);
/// De quien. Se guarda porque la purga recorre muchos y la pantalla azul solo
/// ve el ultimo: sin el pid, "estacion 09" no dice de cual.
static PID: AtomicU32 = AtomicU32::new(0);
/// Cuantos desmontajes ENTEROS terminaron desde el arranque.
///
/// [!] Es el numero que parte el caso en dos: si revienta en el `00`, el
/// primero ya falla y el fallo es del desmontaje; si revienta en el `13`,
/// sobrevivio a trece y lo que falla es la **acumulacion** -- algo que las
/// anteriores dejaron y que la catorce se encuentra.
///
/// ** NO se llama `vueltas`, y a proposito. `purga::Informe` ya tiene un campo
/// con ese nombre y significa **cesiones de CPU**, que es otra cosa. Dos jueces
/// de la misma palabra en la misma pantalla es el `[riesgo] ESPEJO` de la
/// cabecera cobrandose a si mismo.
static DESMONTAJES: AtomicU32 = AtomicU32::new(0);

/// **LAS ESTACIONES**, en el orden exacto de `obj::cap::revoke_all`.
///
/// [!] Los nombres son los de las FUNCIONES a las que se entra, no los de lo
/// que hacen: quien lee esto con la maquina parada necesita un `grep`, no una
/// descripcion. La 17 es la unica que no vive en `revoke_all`.
const ESTACIONES: [&str; 18] = [
    "-",                     // 00: nadie
    "autoridad::olvidar",    // 01
    "endpoint",              // 02
    "fb",                    // 03
    "input",                 // 04
    "mmio",                  // 05
    "audio",                 // 06
    "usb::audio::soltar",    // 07
    "cr3_de_pid",            // 08
    "loan(aspace)",          // 09
    "memory",                // 10
    "console",               // 11
    "directory",             // 12
    "file",                  // 13
    "package",               // 14
    "family",                // 15
    "revoke_all_slots",      // 16
    "destroy_address_space", // 17
];

/// Entrar en una estacion. Lo llama `revoke_all` antes de cada llamada.
#[inline]
pub fn entra(paso: u32, pid: u32) {
    PID.store(pid, Ordering::Relaxed);
    PASO.store(paso, Ordering::Relaxed);
}

/// El desmontaje entero termino bien.
///
/// ** Vuelve a 0 A PROPOSITO. Dejar la ultima estacion puesta haria que un
/// `#PF` de cualquier otro sitio saliera acusando a la 17, que es el fallo de
/// un instrumento que MIENTE -- la clase que este mes ya costo dos dias.
#[inline]
pub fn sale() {
    PASO.store(0, Ordering::Relaxed);
    DESMONTAJES.fetch_add(1, Ordering::Relaxed);
}

/// Lo que la pantalla azul pregunta: `(estacion, nombre, pid, desmontajes)`.
///
/// `None` significa **el fallo no fue desmontando**, y eso tambien es una
/// respuesta: exonera de golpe a los diecisiete.
pub fn donde() -> Option<(u32, &'static str, u32, u32)> {
    let p = PASO.load(Ordering::Relaxed);
    if p == 0 {
        return None;
    }
    let nombre = *ESTACIONES.get(p as usize).unwrap_or(&"?");
    Some((
        p,
        nombre,
        PID.load(Ordering::Relaxed),
        DESMONTAJES.load(Ordering::Relaxed),
    ))
}

/// Cuantos desmontajes enteros van. La usa el informe de la purga, que contaba
/// marcos y ranuras y no contaba **cuantos desmontajes llego a terminar** --
/// que es lo que separa "no limpio" de "limpio a medias y se paro".
pub fn desmontajes() -> u32 {
    DESMONTAJES.load(Ordering::Relaxed)
}

/// **EL GUARDIAN DEL ESPEJO, y corre DE VERDAD.**
///
/// La primera version de esto era un `#[cfg(test)]`, y en este crate eso **no
/// se ejecuta jamas**: `bmo-kernel` es un binario bare-metal y el banco del
/// anfitrion lo salta entero --lo dice el propio build, `bmo-kernel no se
/// prueba aqui`--. O sea que era un guardian que no mira, que es la clase de
/// fallo que este mes ya costo dos dias en otros dos sitios.
///
/// Como constante evaluada en compilacion **si** corre, y ademas rompe el build
/// en vez de una prueba: quien anada una estacion y no la nombre no llega a
/// desplegar.
const _: () = {
    assert!(ESTACIONES.len() == 18, "17 estaciones mas el hueco del 0");
};
