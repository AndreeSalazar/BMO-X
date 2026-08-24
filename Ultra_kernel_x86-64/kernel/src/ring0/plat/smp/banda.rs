//! **El ancho de banda de la memoria.** La casilla vacia del tablero, y la que
//! decide el tamano del modelo que esta maquina puede correr.
//!
//! # Por que este numero y no otro
//!
//! Un modelo de lenguaje generando texto **lee sus pesos enteros por cada
//! token**. No una parte: todos. Asi que:
//!
//! ```text
//!     tokens/s  ~=  ancho de banda  /  bytes del modelo
//! ```
//!
//! *** No es una aproximacion util, es la ecuacion. Un 7B en 4 bits son ~3,7 GB
//! y a 45 GB/s salen ~12 tokens/s -- **y el CPU esta casi parado**. Por eso
//! esto no se optimiza haciendo mas cuentas: **la maquina no esta calculando,
//! esta esperando a la RAM.**
//!
//! ** Y por eso se mide aqui antes de escribir una sola linea de motor. LEY 24:
//! el hardware se PERFILA. Una cifra de hoja de datos --"DDR4-3200 en dos
//! canales son 51,2 GB/s"-- es la cifra de OTRA maquina con los mismos
//! numeros, no de esta.
//!
//! # Un BARRIDO, no un numero
//!
//! Esto no contesta "cuanto ancho de banda tiene la maquina". Contesta **con
//! cuantos obreros deja de subir**, que es una cosa distinta y mas util:
//!
//! ```text
//!    1 obrero    un nucleo de Zen 3 NO SATURA la DDR4 el solo: tiene un
//!                numero limitado de fallos de cache en vuelo a la vez
//!    N obreros   mas nucleos = mas fallos en vuelo = mas ancho, HASTA que
//!                el bus se llena. Ahi la curva se aplana
//!
//!    donde se aplana ES el ancho de banda de la maquina
//! ```
//!
//! *** Y ese aplanamiento es la respuesta honesta a una pregunta que se hizo
//! mal antes: se escribio que *"el decode es memory-bound, asi que el SMP no
//! ayuda"*. Eso vale para dos HILOS en un nucleo --comparten los mismos buffers
//! de relleno, no hay nada que ganar-- y **es falso para nucleos distintos**,
//! que tienen los suyos. El barrido separa las dos cosas sin que haya que
//! creerse ninguna: se ve donde para.
//!
//! # Lo que esto NO mide, dicho antes de que alguien lo suponga
//!
//! - **Escritura.** Solo lectura. Escribir en x86 lee primero la linea para
//!   apropiarsela, asi que el numero de escritura es otro y no sale de aqui.
//! - **Nucleos exactos.** `crew::repartir` toma *cuantos* obreros, no *cuales*.
//!   Con 6 obreros no se sabe si son 6 nucleos o 3 con sus dos hilos, porque
//!   eso depende del orden en que la MADT enumera los APIC. **La curva se lee
//!   igual** --donde se aplana es donde se aplana-- pero atribuir el codo a
//!   "los 6 nucleos fisicos" seria afirmar lo que no se ha comprobado.
//! - **Latencia.** Ancho y latencia son cosas distintas y esta faena esta
//!   escrita a proposito para que la latencia no se note: lecturas secuenciales
//!   que el prefetcher del L2 puede adelantar.

use core::sync::atomic::{AtomicU64, Ordering};

use super::crew;
use crate::ring0::mm::phys;

/// Bytes que se piden para el banco. Ocho veces el L3 del 5600X.
const BANCO_PEDIDO: u64 = 256 * 1024 * 1024;

/// **El banco tiene que ser MUCHO mas grande que el L3 o esto es un fraude.**
///
/// Si los datos caben en cache, la medida sale magnifica y no mide la RAM:
/// mide el L3, que va mas de diez veces mas rapido. Cuatro veces el L3 es el
/// minimo con el que el numero significa algo; por debajo, esto **se niega a
/// contestar** en vez de dar una cifra bonita.
const VECES_EL_L3: u64 = 4;

/// **Techo de lo fisicamente posible**, en MB/s.
///
/// Ninguna DDR4 en dos canales pasa de ~51 GB/s ni de lejos. Un resultado por
/// encima de 100 GB/s no es un record: es que el banco cabia en cache, o que el
/// cronometro miente. Mismo principio que `crew::medida_creible`:
///
/// > Un numero que no puede ser cierto tiene que decirlo el, no el que mira.
const TECHO_IMPOSIBLE_MB_S: u64 = 100_000;

/// Cuantas veces se corre cada punto del barrido. Se queda **la mas rapida**.
///
/// No es hacer trampa: se busca de lo que la maquina ES CAPAZ, y cualquier
/// interferencia --una interrupcion del timer, el refresco de la DRAM-- solo
/// puede hacer una pasada mas lenta, nunca mas rapida. La mejor es la que menos
/// ruido lleva encima.
const PASADAS: u32 = 3;

/// Direccion **virtual del kernel** del banco. `0` = sin preparar.
static BANCO: AtomicU64 = AtomicU64::new(0);
/// Bytes utiles del banco.
static BYTES: AtomicU64 = AtomicU64::new(0);
/// El testigo de cada obrero. **Cero = ese obrero no leyo nada.**
static SUMAS: [AtomicU64; 16] = [const { AtomicU64::new(0) }; 16];

/// El patron con el que se llena el banco: cada palabra **distinta**.
#[inline]
fn patron(i: u64) -> u64 {
    i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5555_5555_5555_5555
}

/// **Reserva el banco y lo llena.** Idempotente.
///
/// Devuelve `(bytes, veces_el_l3)` o el motivo por el que no se pudo.
///
/// [!] LLENARLO NO ES OPCIONAL, y no es por calentar nada --el banco no cabe en
/// ninguna cache-- sino porque un marco recien reservado viene a ceros, y el
/// testigo de un obrero que lee ceros vale cero: **igual que el de un obrero
/// que no llego a ejecutarse**. Un instrumento que no distingue "leyo" de "no
/// corrio" no sirve para nada.
pub fn preparar() -> Result<(u64, u64), &'static str> {
    if BANCO.load(Ordering::SeqCst) != 0 {
        let b = BYTES.load(Ordering::SeqCst);
        return Ok((b, b / l3_bytes().max(1)));
    }

    let l3 = l3_bytes();
    if l3 == 0 {
        return Err("el perfil no dice cuanto L3 hay: no se puede saber si el banco es honesto");
    }
    let minimo = l3 * VECES_EL_L3;

    // Se pide grande y se va bajando: en una maquina con la memoria repartida
    // puede no haber 256 MiB seguidos. Lo que NO se hace es bajar del minimo.
    let mut quiero = BANCO_PEDIDO;
    let fisica = loop {
        if quiero < minimo {
            return Err("no hay un hueco contiguo de 4x el L3: la medida seria de cache");
        }
        if let Some(p) = phys::alloc_frames_contig(quiero / crate::ring0::mm::PAGE) {
            break p;
        }
        quiero /= 2;
    };

    let base = crate::ring0::mm::phys_to_virt(fisica);
    let n = (quiero / 8) as usize;
    let p = base as *mut u64;
    for i in 0..n {
        unsafe { core::ptr::write_volatile(p.add(i), patron(i as u64)) };
    }

    // ** Y SE COMPRUEBA, porque `alloc_frames_contig` promete marcos seguidos y
    // el physmap promete que seguidos en fisico es seguido en virtual. Si
    // cualquiera de las dos cosas fallara, dos trozos del banco taparian el
    // mismo marco -- y la medida saldria *mejor*, porque estaria releyendo algo
    // que ya esta en cache. Un fallo que se disfraza de buen resultado hay que
    // buscarlo a proposito.
    let mut i = 0usize;
    while i < n {
        if unsafe { core::ptr::read_volatile(p.add(i)) } != patron(i as u64) {
            return Err("el banco no se lee como se escribio: marcos solapados");
        }
        i += 4096; // una muestra por pagina
    }

    BANCO.store(base, Ordering::SeqCst);
    BYTES.store(quiero, Ordering::SeqCst);
    Ok((quiero, quiero / l3))
}

/// El L3 que declara el perfil, en bytes. `0` si no lo declara.
fn l3_bytes() -> u64 {
    let c = crate::ring0::cpu_vendor::ryzen_5_5600x::cache::detect_5600x();
    match c.l3 {
        Some(i) => i.size_kb as u64 * 1024,
        None => 0,
    }
}

/// Bytes que lee **cada** obrero cuando son `de` en total.
///
/// Alineado a linea de cache y hacia abajo, asi que `por(de) * de` puede ser un
/// poco menos que el banco. Ese --y no el tamano del banco-- es el numero que
/// se divide por el tiempo.
#[inline]
fn por(de: u32) -> u64 {
    if de == 0 {
        return 0;
    }
    (BYTES.load(Ordering::Relaxed) / de as u64) & !63
}

/// **La faena: leer su rebanada, y nada mas.**
///
/// ## Ocho acumuladores independientes, otra vez, y por otro motivo
///
/// En `crew::faena_ancho` eran para llenar las unidades de calculo. Aqui son
/// para que **haya varios fallos de cache en vuelo a la vez**: con un solo
/// acumulador el nucleo pediria una linea, esperaria, pediria la siguiente...
/// y eso mide LATENCIA, que es el numero equivocado.
///
/// ** Ocho palabras seguidas son exactamente una linea de 64 bytes. El
/// prefetcher del L2 ve el patron secuencial y va por delante, que es
/// justamente lo que hace un motor de inferencia leyendo una matriz de pesos.
///
/// [!] `read_volatile` y no una lectura normal: el compilador ve un bucle que
/// suma cosas y no usa el resultado, y **se lo lleva entero**. Sin esto la
/// prueba mediria lo rapido que es no leer nada, que es infinito.
fn faena_banda(parte: u32, de: u32) {
    let base = BANCO.load(Ordering::Relaxed);
    let trozo = por(de);
    if base == 0 || trozo == 0 {
        return;
    }
    let p = (base + trozo * parte as u64) as *const u64;
    let n = (trozo / 8) as usize;

    let mut a: [u64; 8] = [0; 8];
    let mut i = 0usize;
    while i + 8 <= n {
        unsafe {
            a[0] ^= core::ptr::read_volatile(p.add(i));
            a[1] ^= core::ptr::read_volatile(p.add(i + 1));
            a[2] ^= core::ptr::read_volatile(p.add(i + 2));
            a[3] ^= core::ptr::read_volatile(p.add(i + 3));
            a[4] ^= core::ptr::read_volatile(p.add(i + 4));
            a[5] ^= core::ptr::read_volatile(p.add(i + 5));
            a[6] ^= core::ptr::read_volatile(p.add(i + 6));
            a[7] ^= core::ptr::read_volatile(p.add(i + 7));
        }
        i += 8;
    }
    let mut h = 0u64;
    for x in a.iter() {
        h ^= *x;
    }
    if (parte as usize) < SUMAS.len() {
        SUMAS[parte as usize].store(h, Ordering::SeqCst);
    }
}

/// **Un punto del barrido.** `obreros` son los APs; el BSP trabaja siempre.
///
/// Devuelve `(ticks_mejores, bytes_leidos, llegaron_todos)`.
pub fn medir(obreros: u32) -> (u64, u64, bool) {
    use crate::ring0::task::scheduler::rdtsc_serial as reloj;
    let partes = obreros + 1;
    let bytes = por(partes) * partes as u64;

    let mut mejor = u64::MAX;
    let mut ok = true;
    for _ in 0..PASADAS {
        let t0 = reloj();
        let llego = crew::repartir(faena_banda, obreros);
        let t = reloj().wrapping_sub(t0);
        ok &= llego;
        if t < mejor {
            mejor = t;
        }
    }
    (mejor, bytes, ok)
}

/// Pasa de ticks a **MB/s**, o `None` si no se puede saber.
///
/// [!] Depende de `tsc_freq_hz()`, que es lo unico aqui que no se mide en el
/// sitio. Si el perfil no la sabe, esto devuelve `None` y quien pinte ensena
/// los ticks pelados -- **nunca una cifra en MB/s calculada con una frecuencia
/// inventada**, que es exactamente el tipo de numero que parece una medida y no
/// lo es.
pub fn mb_por_segundo(bytes: u64, ticks: u64) -> Option<u64> {
    let hz = crate::ring0::cpu_vendor::ryzen_5_5600x::bmo_cpu::tsc_freq_hz();
    if hz == 0 || ticks == 0 {
        return None;
    }
    Some(bytes.checked_mul(hz)? / ticks / 1_000_000)
}

/// **Puede ser cierto este numero?** Ver [`TECHO_IMPOSIBLE_MB_S`].
pub fn creible(mb_s: u64) -> bool {
    mb_s > 0 && mb_s < TECHO_IMPOSIBLE_MB_S
}

/// El testigo del obrero 0. **Cero = no leyo.**
pub fn testigo() -> u64 {
    SUMAS[0].load(Ordering::SeqCst)
}

/// Los puntos del barrido, en obreros **extra** (el BSP va siempre aparte).
///
/// `0, 1, 3, 5, 7, 11` = 1, 2, 4, 6, 8 y 12 partes. Los 6 y los 12 son los que
/// importan en un 5600X --nucleos e hilos-- y los de en medio son los que
/// dibujan la curva entre ellos.
pub const PUNTOS: [u32; 6] = [0, 1, 3, 5, 7, 11];

/// **Cuantos tokens por segundo daria un modelo de `gb` gigabytes.**
///
/// x100 para no necesitar decimales: `1250` son 12,50 tokens/s.
///
/// ** Es un TECHO, no una prediccion. Supone que el motor lee los pesos una vez
/// por token y a la velocidad maxima de la maquina; un motor de verdad se queda
/// entre el 60% y el 80% de esto. Lo que si es cierto es que **no puede
/// pasarlo**, y por eso sirve para elegir el tamano del modelo antes de
/// escribir el motor.
pub fn techo_tokens_x100(mb_s: u64, mb_modelo: u64) -> u64 {
    if mb_modelo == 0 {
        return 0;
    }
    mb_s * 100 / mb_modelo
}
