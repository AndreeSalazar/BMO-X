//! **EL ATRIL: donde se deja el encargo antes de decir *tocad*.**
//!
//! [carril]  ROJO      publica direcciones FISICAS que doce nucleos van a
//!                     escribir sin preguntar nada mas
//! [consumo] NADA      corre cuando se levantan o se reparten nucleos
//!
//! [cuesta]  MAQUINA -- un encargo mal publicado no falla: **escribe**. Doce
//!           obreros escribiendo por el espejo en una fisica equivocada no dan
//!           un fallo de pagina --el espejo alcanza los 16 GiB enteros-- dan
//!           memoria de otro pisada, y el sintoma tres arranques despues.
//!
//! [riesgo]  ESPEJO SILENCIO
//!           ESPEJO   -- el catalogo de partes vive en `bmo-orquesta` y las
//!                       funciones que las ejecutan viven AQUI. Son dos listas
//!                       sobre los mismos numeros, y el guardian del final las
//!                       cuenta a las dos.
//!           SILENCIO -- un atril a medio llenar es indistinguible de uno lleno:
//!                       todos los campos son numeros y el cero es legal en tres
//!                       de los cuatro. Por eso el juez de `bmo-orquesta` corre
//!                       ANTES de repartir y por eso rechaza por MOTIVO.
//!
//! # *** POR QUE ESTE FICHERO ES LA PUERTA, Y QUE NO DEJA PASAR
//!
//! `crew::repartir` sabe cortar una faena en n partes desde el 08-08, y en
//! metal arranco 12 de 12. **Y nadie le habia dado nunca una faena de verdad**:
//! su unico llamador era un banco de pruebas. Esto es lo que faltaba, y lo que
//! faltaba no era el reparto: era **como se le dicen los numeros**.
//!
//! Una `Faena` es `fn(u32, u32)` -- mi parte y cuantas hay. No caben
//! parametros. Asi que el encargo se publica aqui, en atomicas que los obreros
//! leen, con la MISMA disciplina que `crew` usa para la faena: **todo antes de
//! la ronda**, porque la ronda es la senal.
//!
//! ## Lo que NO cruza esta puerta
//!
//! ```text
//!    un puntero a funcion de Ring 3     JAMAS
//!    una direccion virtual sin traducir  JAMAS
//!    una fisica que no sea del que llama JAMAS
//! ```
//!
//! Los tres se cierran en el mismo sitio y con lo que ya habia:
//!
//! * la parte es un **numero de catalogo**, y el catalogo lo resuelve
//!   `bmo_orquesta::Parte::de_numero`, que dice `None` a lo que no conoce
//! * la traduccion la hace `obj::memory::fisica_de(pid, va, len)`, que **solo
//!   traduce dentro de los bloques del propio pid** -- una app no puede nombrar
//!   memoria ajena porque la funcion que traduce no sabe hacerlo
//! * y el `len` va en la traduccion, asi que un rango que se sale del bloque no
//!   devuelve una fisica recortada: devuelve `None`
//!
//! ## Por que por el ESPEJO y no cambiando de CR3
//!
//! Los obreros no tienen GS por-CPU ni TSS propia --lo dice la cabecera de
//! `crew`, y es lo que mantiene ese modulo en cien lineas--. Cambiarles el CR3
//! seria empezar por ahi.
//!
//! No hace falta: un bloque de `obj::memory` es **contiguo y con su fisica ya
//! guardada** (`Bloque { base, fisica, bytes }`), y el espejo del kernel alcanza
//! los 16 GiB. Los obreros escriben por el espejo, bajo el CR3 del kernel, sin
//! tocar el espacio de nadie.
//!
//! ** Y el que pidio la faena esta BLOQUEADO dentro del syscall mientras dura,
//! asi que su espacio no puede cambiar debajo. Esa es la razon de que la puerta
//! bloquee, y no la comodidad.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use bmo_orquesta::{
    bytes_de, conviene_despertar, cuantos_atriles, se_puede_tocar, Encargo, Escala, Parte, Rango,
};

/// Si la orquesta ya intento despertar nucleos por su cuenta en este arranque.
/// Ver `bmo_orquesta::conviene_despertar`: se intenta UNA vez, y parar a mano
/// con `smp stop` gana siempre.
static YA_SE_INTENTO: AtomicBool = AtomicBool::new(false);

// == EL ATRIL, publicado antes de la ronda =================================
static DESTINO: AtomicU64 = AtomicU64::new(0);
static ORIGEN: AtomicU64 = AtomicU64::new(0);
static TOTAL: AtomicU64 = AtomicU64::new(0);
static DATO: AtomicU64 = AtomicU64::new(0);

/// Cuantos campos tiene un encargo. El indice llega por la puerta, asi que se
/// comprueba: un campo de mas seria escribir fuera de esta lista.
pub const CAMPOS: u64 = 4;

/// **Poner un numero en el atril.** `campo` es el indice, `valor` el numero.
///
/// [!] Guarda la VIRTUAL tal cual. La traduccion a fisica se hace en `tocar`, y
/// no aqui, por una razon: traducir necesita el `pid`, y el pid del que pone el
/// numero y el del que dice *tocad* **tienen que ser el mismo**. Traducir al
/// final es lo que hace que esa igualdad se compruebe una vez y no cuatro.
pub fn poner(campo: u64, valor: u64) -> bool {
    match campo {
        0 => DESTINO.store(valor, Ordering::SeqCst),
        1 => ORIGEN.store(valor, Ordering::SeqCst),
        2 => TOTAL.store(valor, Ordering::SeqCst),
        3 => DATO.store(valor, Ordering::SeqCst),
        _ => return false,
    }
    true
}

/// Lo que hay escrito, con las direcciones todavia VIRTUALES.
fn recoger() -> Encargo {
    Encargo {
        destino: DESTINO.load(Ordering::SeqCst),
        origen: ORIGEN.load(Ordering::SeqCst),
        total: TOTAL.load(Ordering::SeqCst),
        dato: DATO.load(Ordering::SeqCst),
    }
}

/// El atril se vacia SIEMPRE al terminar, salga bien o mal.
///
/// ** Un atril que conserva el encargo anterior es la trampa exacta que el
/// `RONDA` de `crew` existe para evitar un piso mas abajo: un `tocar` al que se
/// le olvide un campo tocaria con el numero de la vez pasada, y eso no falla --
/// escribe en el sitio de antes.
fn vaciar() {
    DESTINO.store(0, Ordering::SeqCst);
    ORIGEN.store(0, Ordering::SeqCst);
    TOTAL.store(0, Ordering::SeqCst);
    DATO.store(0, Ordering::SeqCst);
}

// == LO QUE LOS OBREROS LEEN ===============================================
//
// Copias en fisica, publicadas por `tocar` ANTES de llamar a `repartir` -- que
// a su vez publica la ronda al final. El orden es el mismo de siempre: los
// datos primero, la senal la ultima.
static F_DESTINO: AtomicU64 = AtomicU64::new(0);
static F_ORIGEN: AtomicU64 = AtomicU64::new(0);
static F_TOTAL: AtomicU64 = AtomicU64::new(0);
static F_DATO: AtomicU64 = AtomicU64::new(0);
/// Cuantos atriles se rechazaron por pedir mas de los que hay. Sale en CABINA.
static RECORTES: AtomicU32 = AtomicU32::new(0);

/// El puntero del espejo para una fisica. Es la unica forma que tiene un obrero
/// de tocar la memoria del que pidio la faena.
#[inline]
fn espejo(fisica: u64) -> *mut u32 {
    crate::ring0::mm::phys_to_virt(fisica) as *mut u32
}

/// **LLENAR**: el mismo valor de 32 bits en todo el rango.
///
/// La parte mas simple que existe, y esta a proposito: el resultado no depende
/// de nada mas que del reparto, asi que si sale mal el reparto es el culpable.
/// Es el instrumento que puede MATAR la hipotesis, no confirmarla.
fn llenar(mia: u32, partes: u32) {
    let base = F_DESTINO.load(Ordering::SeqCst);
    let total = F_TOTAL.load(Ordering::SeqCst);
    let v = F_DATO.load(Ordering::SeqCst) as u32;
    let r = Rango::de(mia as u64, partes as u64, total);
    let d = espejo(base);
    for i in r.desde..r.hasta {
        unsafe { d.add(i as usize).write_volatile(v) };
    }
}

/// **EXPANDIR**: cada pixel de origen, `escala` veces en el destino.
///
/// La primera faena REAL, y la que motivo la puerta: en DOOM son 10.590 us por
/// fotograma, y las 200 filas **no se tocan entre ellas**. `total` son los
/// pixeles de origen; cada uno escribe `escala` en el destino.
///
/// [!] El rango se reparte sobre el ORIGEN y el destino se calcula, no al reves.
/// Repartir el destino haria que un atril empezara a mitad de un pixel expandido
/// -- y eso es un solape que la aritmetica de `Rango` no puede ver, porque para
/// ella todos los elementos miden igual.
fn expandir(mia: u32, partes: u32) {
    let dst = F_DESTINO.load(Ordering::SeqCst);
    let src = F_ORIGEN.load(Ordering::SeqCst);
    let total = F_TOTAL.load(Ordering::SeqCst);
    let escala = F_DATO.load(Ordering::SeqCst);
    let r = Rango::de(mia as u64, partes as u64, total);
    let s = espejo(src);
    let d = espejo(dst);
    for i in r.desde..r.hasta {
        let p = unsafe { s.add(i as usize).read_volatile() };
        let mut j = i * escala;
        let fin = j + escala;
        while j < fin {
            unsafe { d.add(j as usize).write_volatile(p) };
            j += 1;
        }
    }
}

/// **ESCALAR**: la imagen entera, al factor entero que cabe, centrada y con
/// negro alrededor. La geometria NO se calcula aqui: es `bmo_orquesta::Escala`,
/// probada en el anfitrion con los numeros del cubo del Ryzen.
///
/// Se reparten las FILAS del destino. Cada una sabe de que fila de la imagen
/// sale sin mirar a las demas, asi que dos atriles no escriben nunca la misma.
///
/// [!] Si `Escala::de` dijera que no aqui, el obrero no escribe: el juez ya lo
/// acepto en `tocar` con estos mismos numeros, asi que solo pasaria si el atril
/// cambiara a mitad -- y un obrero no escribe con numeros que no cuadran.
fn escalar(mia: u32, partes: u32) {
    let dst = F_DESTINO.load(Ordering::SeqCst);
    let src = F_ORIGEN.load(Ordering::SeqCst);
    let alto = F_TOTAL.load(Ordering::SeqCst);
    let dato = F_DATO.load(Ordering::SeqCst);
    let Ok(g) = Escala::de(dato, alto) else { return };
    let r = Rango::de(mia as u64, partes as u64, alto);
    let s = espejo(src);
    let d = espejo(dst);
    for y in r.desde..r.hasta {
        let fila = y * g.dst_ancho;
        let fuente = g.fila_fuente(y).map(|sy| sy * g.src_ancho);
        for x in 0..g.dst_ancho {
            let p = match (fuente, g.columna_fuente(x)) {
                (Some(f), Some(sx)) => unsafe { s.add((f + sx) as usize).read_volatile() },
                _ => 0,
            };
            unsafe { d.add((fila + x) as usize).write_volatile(p) };
        }
    }
}

/// Por que no se pudo tocar. Sale por la puerta como un numero.
pub const NO_HAY_ORQUESTA: u64 = u64::MAX;

/// **TOCAD.** Traduce, juzga, publica y reparte. Devuelve cuantos atriles
/// tocaron, o `NO_HAY_ORQUESTA` si el encargo no pasa.
///
/// El orden NO es negociable y es el mismo de `crew::repartir` un piso mas
/// abajo: **primero se comprueba todo, despues se publica, y la senal va la
/// ultima**. Publicar antes de juzgar dejaria un encargo malo visible para un
/// obrero que ya estuviera mirando.
pub fn tocar(pid: u32, parte_num: u64, pedidos: u64) -> u64 {
    let e = recoger();
    let parte = match Parte::de_numero(parte_num) {
        Some(p) => p,
        None => {
            vaciar();
            crate::ring0::cabina::fault("orquesta", "parte que no existe", parte_num);
            return NO_HAY_ORQUESTA;
        }
    };
    if let Err(por_que) = se_puede_tocar(parte, &e) {
        vaciar();
        crate::ring0::cabina::fault("orquesta", parte.nombre(), por_que as u64);
        return NO_HAY_ORQUESTA;
    }

    // == LA TRADUCCION, que es donde se cierra la puerta de verdad ==========
    //
    // `fisica_de` solo traduce dentro de los bloques del propio `pid`, y lleva
    // el `len` dentro: un rango que se sale del bloque no devuelve una fisica
    // recortada, devuelve `None`. Por eso una app no puede nombrar memoria
    // ajena -- no es que se le prohiba, es que **la funcion que traduce no sabe
    // hacerlo**.
    // ** Los tamanos los dice `bmo_orquesta::bytes_de`, probado en el
    // anfitrion, y no una rama por parte aqui: una parte nueva que se olvidara
    // de la suya traduciria de menos, y eso no falla -- deja escribir mas alla
    // de lo comprobado. Y ya no SATURA: un tamano que no cabe en 64 bits se
    // rechaza en vez de convertirse en `u64::MAX`.
    let Some((bytes_dst, bytes_src)) = bytes_de(parte, &e) else {
        vaciar();
        crate::ring0::cabina::fault("orquesta", "el encargo no cabe en 64 bits", parte_num);
        return NO_HAY_ORQUESTA;
    };
    let f_dst = match crate::ring0::obj::memory::fisica_de(pid, e.destino, bytes_dst) {
        Some(f) => f,
        None => {
            vaciar();
            crate::ring0::cabina::fault("orquesta", "el destino no es tuyo", e.destino);
            return NO_HAY_ORQUESTA;
        }
    };
    let f_src = if e.origen == 0 {
        0
    } else {
        match crate::ring0::obj::memory::fisica_de(pid, e.origen, bytes_src) {
            Some(f) => f,
            None => {
                vaciar();
                crate::ring0::cabina::fault("orquesta", "el origen no es tuyo", e.origen);
                return NO_HAY_ORQUESTA;
            }
        }
    };

    // == CUANTOS ATRILES, que lo decide el PERFIL y no el que llama =========
    //
    // ** Quien pide puede sugerir; quien manda es la maquina. Un `.bex` que
    // pida cien atriles en una maquina de seis nucleos no recibe cien: recibe
    // los que hay, y el recorte se apunta. Es la ley 24 --el hardware se
    // PERFILA-- aplicada a un reparto.
    let (mut vivos, _) = super::alive();
    // ** SI LA APP PIDE Y NO HAY NADIE EN PIE, SE DESPIERTA -- una vez.
    //
    // Politica del dueno (2026-09-12): los nucleos se activan solos cuando una
    // app pide una parte que merece repartirse. El juicio vive en
    // `bmo_orquesta::conviene_despertar`, probado en el anfitrion; aqui solo se
    // obedece y se DICE en CABINA, porque despertar cambia el estado del
    // hardware y eso no puede ser silencioso ni cuando sale bien.
    if conviene_despertar(e.total, vivos as u64, pedidos, YA_SE_INTENTO.load(Ordering::SeqCst)) {
        YA_SE_INTENTO.store(true, Ordering::SeqCst);
        let (despiertos, esperados) = super::despertar(u32::MAX, |_| {});
        crate::ring0::cabina::info("orquesta", "una app pidio: nucleos despertados", despiertos as u64);
        if despiertos == 0 {
            // ** El "nel" del orquestador, con su motivo: se reparte con el BSP
            // solo y no se vuelve a intentar hasta reiniciar.
            crate::ring0::cabina::warn(
                "orquesta",
                "desperte y no contesto ningun nucleo: toca el BSP solo",
                esperados as u64,
            );
        }
        vivos = super::alive().0;
    }
    // ** `+ 1` porque `alive()` cuenta los OBREROS y `cuantos_atriles` cuenta al
    // BSP (su prueba lo dice: *"con un solo nucleo en pie, uno"*). Sin el, con
    // once obreros en pie se repartia entre once y uno se quedaba quieto.
    let en_pie = vivos as u64 + 1;
    let conviene = cuantos_atriles(e.total, en_pie);
    let atriles = if pedidos == 0 {
        conviene
    } else {
        if pedidos > conviene {
            RECORTES.fetch_add(1, Ordering::Relaxed);
        }
        if pedidos < conviene { pedidos } else { conviene }
    };

    F_DESTINO.store(f_dst, Ordering::SeqCst);
    F_ORIGEN.store(f_src, Ordering::SeqCst);
    F_TOTAL.store(e.total, Ordering::SeqCst);
    F_DATO.store(e.dato, Ordering::SeqCst);

    let faena: super::crew::Faena = match parte {
        Parte::Llenar => llenar,
        Parte::Expandir => expandir,
        Parte::Escalar => escalar,
        Parte::Nada => unreachable!("el juez ya lo rechazo"),
    };
    // `repartir` cuenta al BSP como una parte, asi que los obreros son uno menos.
    let ok = super::crew::repartir(faena, (atriles - 1) as u32);
    vaciar();
    if !ok {
        // ** Una barrera que no se cierra deja el DATO A MEDIAS, no lento. Falta
        // el trozo de alguien, y devolverlo como si nada seria entregar un
        // buffer con un agujero. Ver `crew::repartir`.
        crate::ring0::cabina::fault("orquesta", "falto un atril: el dato NO vale", atriles);
        return NO_HAY_ORQUESTA;
    }
    atriles
}

/// **EL GUARDIAN DEL ESPEJO**, y corre en compilacion.
///
/// El catalogo de `bmo-orquesta` y el despacho de aqui arriba son dos listas
/// sobre los mismos numeros. Si alguien anade una parte alla y no la escribe
/// aqui, el `match` de `tocar` no compila --Rust obliga-- pero al reves si
/// colaria. Esto lo cierra: el numero de partes escritas es contrato.
const _: () = {
    assert!(bmo_orquesta::PARTES_ESCRITAS == 4, "el catalogo crecio y el despacho no");
    assert!(CAMPOS == 4, "un campo mas en el encargo es un campo mas en `poner`");
};
