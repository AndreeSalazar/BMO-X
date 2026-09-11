//! **LA CAIDA** -- lo ultimo que dijo la maquina, en RAM que sobrevive al reinicio.
//!
//! [carril]  AMARILLO  es un instrumento: si miente no falla, CONVENCE. Un
//!                     fichero de caida con las lineas equivocadas manda la
//!                     investigacion al sitio que no es
//! [cuesta]  NADA      un byte y un incremento por cada byte que sale a pantalla.
//!                     Ni disco, ni bloqueo, ni interrupcion
//! [riesgo]  AJENO     depende de que la PLACA conserve la DRAM en un reinicio
//!                     en caliente, y eso no lo decide este kernel: lo decide el
//!                     firmware. Es una hipotesis hasta que el Ryzen conteste
//!
//! # La pregunta del dueno (2026-09-11)
//!
//! > *"podemos hacer que el arranque escriba en tiempo real para saber?"*
//!
//! Y la pregunta de fondo, que es mejor: **hoy lo que se sabe es lo que `save`
//! alcanzo a guardar, y `save` necesita que la maquina siga viva.** Si se cuelga
//! en el minuto tres, lo del minuto tres no existe. `blackbox.rs` lo tiene
//! escrito: es la pieza que puede tener que correr *mientras lo que graba se
//! esta muriendo*, que es justo cuando el disco es lo menos fiable.
//!
//! # La idea, que no es de esta casa: `ramoops`
//!
//! Los kernels de verdad lo resuelven asi. **La DRAM no se borra en un reinicio
//! en caliente** --el reset no corta la alimentacion--, asi que una region fija
//! de RAM escrita durante la sesion sigue ahi cuando el kernel vuelve a
//! arrancar. Se lee ANTES de que nadie la pise, se guarda en el disco con calma
//! --ahora que el disco es fiable-- y se vuelve a preparar para esta sesion.
//!
//! ```text
//!    durante la sesion    cada byte que sale a pantalla -> tambien aqui
//!                         coste: un `mov` y un `inc`. Cero disco.
//!    la maquina muere     la RAM se queda como estaba
//!    reinicio             se lee la region, se escribe CAIDA.TXT, se limpia
//! ```
//!
//! *** Es "en tiempo real" en el unico sentido que importa: **nada de lo que
//! salio por pantalla se pierde**, y no cuesta nada mientras la maquina vive.
//!
//! # [!] Y ES UNA HIPOTESIS SOBRE LA PLACA, no una certeza -- LEY 24
//!
//! Dos cosas pueden borrarla y este kernel no manda sobre ninguna:
//!
//! ```text
//!    1. el firmware limpia la DRAM al reiniciar   (los AMI de AM4 no suelen)
//!    2. el firmware o el cargador ESCRIBEN encima  antes de que esto la lea
//! ```
//!
//! Contra la 2 se elige una direccion que ni las etapas ni el kernel tocan
//! --64 MiB, lejos de 0x100000/0x200000/0x400000-- y se comprueba al arrancar
//! que cae dentro de un tramo de RAM usable. Contra la 1 no hay defensa: solo
//! medirlo. **El primer arranque con esto dentro es el perfil.** Si tras un
//! reinicio en caliente aparece `CAIDA.TXT` con las lineas de la sesion
//! anterior, la placa lo conserva. Si no, se dice, y el plan B es un volcado
//! periodico al disco.
//!
//! # La forma en RAM
//!
//! ```text
//!    +0    magia        u64   "vale, esto es mio"
//!    +8    version      u32   por si la forma cambia
//!    +12   generacion   u32   cuantos arranques ha visto esta region
//!    +16   cursor       u64   bytes escritos en total; posicion = cursor % CAP
//!    +24   suma         u64   de magia|version|generacion. NO del cursor
//!    +32   texto        CAP bytes, anillo
//! ```
//!
//! ** La suma no cubre el cursor a proposito: recalcularla en cada byte seria
//! pagar una decena de operaciones por byte para proteger un numero que ya se
//! valida por rango. La cabecera fija se firma una vez; el cursor se comprueba
//! con `< 2^40`, que es lo que separa un cursor de basura.
//!
//! # Lo que NO hace
//!
//! No toma ningun cerrojo. Se escribe desde interrupciones y desde el camino de
//! la muerte, y un cerrojo ahi es la forma de que la ultima linea no se escriba
//! nunca. Una linea partida por otra en el fichero de caida es un precio; un
//! fichero vacio no lo es.

use crate::ring0::mm;

/// Donde vive. 64 MiB: lejos de las etapas (1, 2 y 4 MiB) y del kernel.
pub const BASE: u64 = 0x0400_0000;
/// Cuanto: 256 KiB de texto, que son unas cinco mil lineas.
pub const BYTES: u64 = 256 * 1024;
const CAB: u64 = 32;
const CAP: u64 = BYTES - CAB;
/// Lo que esta sesion puede escribir ANTES de que el disco este montado sin
/// pisar lo recuperado. Un arranque hasta el montaje son unas decenas de KiB
/// de lineas; con 64 KiB de margen, lo recuperado se recorta a los ultimos
/// 192 KiB y el arranque nuevo escribe detras sin tocarlo.
const MARGEN: u64 = 64 * 1024;
const MAGIA: u64 = 0x4341_4944_4142_4D4F; // "CAIDABMO" al reves, como se lee en memoria
const VERSION: u32 = 1;

/// `true` desde que la region esta comprobada y preparada para esta sesion.
static mut LISTA: bool = false;
/// Lo que se dijo ANTES de que la region estuviera lista, para no perderlo.
const ANTES_MAX: usize = 4096;
static mut ANTES: [u8; ANTES_MAX] = [0; ANTES_MAX];
static mut ANTES_N: usize = 0;
/// Lo que se recupero del arranque anterior: `(bytes, generacion)`.
static mut RECUPERADO: usize = 0;
static mut GENERACION: u32 = 0;

#[inline(always)]
fn ptr(off: u64) -> *mut u8 {
    mm::phys_to_virt(BASE + off) as *mut u8
}

unsafe fn lee64(off: u64) -> u64 { core::ptr::read_volatile(ptr(off) as *const u64) }
unsafe fn lee32(off: u64) -> u32 { core::ptr::read_volatile(ptr(off) as *const u32) }
unsafe fn pon64(off: u64, v: u64) { core::ptr::write_volatile(ptr(off) as *mut u64, v) }
unsafe fn pon32(off: u64, v: u32) { core::ptr::write_volatile(ptr(off) as *mut u32, v) }

fn suma(magia: u64, version: u32, generacion: u32) -> u64 {
    // Una mezcla barata, no criptografia: separa "cabecera de una sesion" de
    // "bytes cualesquiera", que es lo unico que hace falta.
    let mut s = magia ^ ((version as u64) << 32 | generacion as u64);
    s ^= s.rotate_left(29);
    s = s.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    s ^ s.rotate_left(17)
}

/// **Un byte que salio a pantalla.** Se llama desde `serial_write_byte`, que
/// es el embudo por el que pasa todo lo que la maquina dice.
#[inline]
pub fn anotar(b: u8) {
    unsafe {
        if !LISTA {
            // Todavia no hay region: se guarda aparte y se copia al abrir.
            if ANTES_N < ANTES_MAX {
                ANTES[ANTES_N] = b;
                ANTES_N += 1;
            }
            return;
        }
        let cursor = lee64(16);
        core::ptr::write_volatile(ptr(CAB + cursor % CAP), b);
        pon64(16, cursor + 1);
    }
}

/// **Abrir la region: recuperar lo del arranque anterior y preparar esta.**
///
/// Se llama desde `mm::phys::init`, justo despues de reservar el rango y con
/// el physmap ya en pie. `dentro_de_ram` lo comprueba quien llama, que es
/// quien tiene el mapa: si la region no cae en RAM usable, esto no se abre y
/// se dice.
pub fn abrir(dentro_de_ram: bool) {
    if !dentro_de_ram {
        crate::ring0::cabina::warn(
            "caida",
            "la region de 64 MiB no cae en RAM usable: sin caja negra en RAM",
            BASE,
        );
        return;
    }
    let habia_magia;
    unsafe {
        let magia = lee64(0);
        habia_magia = magia == MAGIA;
        let version = lee32(8);
        let generacion = lee32(12);
        let cursor = lee64(16);
        let firma = lee64(24);
        let cabecera_ok = magia == MAGIA
            && version == VERSION
            && firma == suma(magia, version, generacion)
            && cursor < (1u64 << 40);
        if cabecera_ok && cursor > 0 {
            // *** HAY RASTRO DEL ARRANQUE ANTERIOR. No se mueve de sitio: se
            // apunta cuanto es y se deja donde esta hasta que el disco pueda
            // recibirlo. `volcar` lo lee de aqui.
            let total = if cursor > CAP { CAP } else { cursor };
            GENERACION = generacion.wrapping_add(1);
            // Lo recuperado tiene que sobrevivir a esta apertura: los bytes
            // nuevos de esta sesion empiezan DESPUES de reservarlo. Se
            // empaqueta al principio del anillo en orden cronologico, para que
            // `volcar` lo lea de un tiron y esta sesion escriba detras.
            enderezar(cursor);
            // ** Y SE RECORTA A LOS ULTIMOS `CAP - MARGEN`. Si se guardara
            // todo el anillo lleno, el cursor de esta sesion arrancaria en
            // CAP, o sea en 0: la primera linea del arranque nuevo pisaria
            // la primera linea recuperada, y el disco no esta montado hasta
            // bastante despues. Lo mas viejo es lo que menos vale.
            let k = if total > CAP - MARGEN { CAP - MARGEN } else { total };
            if k < total {
                core::ptr::copy(ptr(CAB + (total - k)), ptr(CAB), k as usize);
            }
            RECUPERADO = k as usize;
            pon64(16, k);
        } else {
            RECUPERADO = 0;
            GENERACION = if cabecera_ok { generacion.wrapping_add(1) } else { 1 };
            pon64(16, 0);
        }
        pon64(0, MAGIA);
        pon32(8, VERSION);
        pon32(12, GENERACION);
        pon64(24, suma(MAGIA, VERSION, GENERACION));
        LISTA = true;
        // Y lo que se dijo antes de abrir, ahora dentro.
        let n = ANTES_N;
        let mut i = 0;
        while i < n {
            anotar(ANTES[i]);
            i += 1;
        }
        ANTES_N = 0;
    }
    // *** TRES RESPUESTAS DISTINTAS, porque mandan a sitios distintos. "No hay
    // rastro" y "hay rastro pisado" se leerian igual con un solo mensaje, y son
    // las dos hipotesis de la cabecera: la placa BORRA, o alguien ESCRIBE encima.
    match (unsafe { RECUPERADO }, habia_magia) {
        (n, _) if n > 0 => crate::ring0::cabina::info(
            "caida",
            "la RAM CONSERVO lo ultimo que dijo el arranque anterior: bytes",
            n as u64,
        ),
        (_, true) => crate::ring0::cabina::info(
            "caida",
            "la RAM conservo la cabecera y nada dentro: la sesion anterior no escribio (o es esta misma)",
            unsafe { GENERACION } as u64,
        ),
        (_, false) => crate::ring0::cabina::info(
            "caida",
            "la RAM no traia rastro: primer arranque, o la placa la borra al reiniciar",
            unsafe { GENERACION } as u64,
        ),
    }
}

/// Deja los ultimos `min(cursor, CAP)` bytes del anillo en orden cronologico
/// desde el offset 0 del texto. Con un anillo que dio la vuelta hay que rotar;
/// sin vuelta ya estan en orden.
unsafe fn enderezar(cursor: u64) {
    if cursor <= CAP {
        return;
    }
    // Rotacion in situ por inversiones: tres `reverse` sin memoria extra.
    let inicio = (cursor % CAP) as usize;
    let texto = core::slice::from_raw_parts_mut(ptr(CAB), CAP as usize);
    texto[..inicio].reverse();
    texto[inicio..].reverse();
    texto.reverse();
}

/// **Volcar lo recuperado a `CAIDA.TXT`**, cuando el disco ya esta montado.
///
/// Devuelve los bytes escritos. Cero si no habia nada, o si el disco dijo que
/// no -- y en ese caso CABINA ya lo conto.
pub fn volcar() -> usize {
    let n = unsafe { RECUPERADO };
    if n == 0 {
        return 0;
    }
    let texto = unsafe { core::slice::from_raw_parts(ptr(CAB) as *const u8, n) };
    match crate::ring0::fsys::fs::create(b"CAIDA   TXT", texto) {
        Ok(()) => {
            crate::ring0::cabina::info("caida", "guardado en CAIDA.TXT: bytes", n as u64);
            n
        }
        Err(_) => {
            crate::ring0::cabina::warn("caida", "el disco no acepto CAIDA.TXT; sigue en RAM", n as u64);
            0
        }
    }
}

/// Cuantos bytes se recuperaron del arranque anterior (para el panel).
pub fn recuperado() -> u64 {
    unsafe { RECUPERADO as u64 }
}

/// La generacion de esta region: cuantos arranques ha visto.
pub fn generacion() -> u64 {
    unsafe { GENERACION as u64 }
}
