//! **LA FICHA DE CADA OBRERO: quien es, y que lleva hecho AHORA MISMO.**
//!
//! [carril]  AMARILLO  aqui escriben ONCE nucleos a la vez y lee el BSP
//!
//! [cuesta]  NADA -- de aqui no sale ni un cambio de propiedad, ni un mapeo, ni
//!           una decision. Solo se contesta quien es cada obrero y cuanto
//!           lleva. Equivocarse pinta un panel raro, no rompe una maquina.
//!
//! [riesgo]  SILENCIO ESPEJO
//!           SILENCIO -- un contador que no se mueve y un nucleo que no
//!                       trabaja se ven EXACTAMENTE IGUAL desde el panel. Por
//!                       eso el estado se escribe antes y despues de la faena
//!                       y no solo al acabar: un obrero atascado se queda en
//!                       TRABAJANDO, que es distinto de ESPERANDO.
//!           ESPEJO   -- **y este riesgo se cerro antes de nacer.** La primera
//!                       version guardaba aqui una copia de `hilos_por_nucleo`
//!                       para poder dividir el APIC en nucleo y hilo. Al ir a
//!                       cablearla aparecio que `smp::tipo_de` YA contestaba
//!                       CORE/THREAD con el perfil delante: habrian sido dos
//!                       jueces del mismo numero, que es el patron 55 y el que
//!                       mas caro ha salido en este arbol. Ahora la ficha
//!                       guarda el APIC --un numero suyo-- y la identidad la
//!                       resuelve `smp::donde_vive`, que es el unico que juzga.
//!
//! # *** POR QUE ESTE FICHERO EXISTE
//!
//! Lo pidio el dueno el 2026-09-03, y con la observacion correcta delante:
//!
//! > *"los 12 no son correcto [...] empezar a construir asi como core | thread"*
//!
//! El 5600X tiene **6 nucleos fisicos y 12 hilos logicos**. `crew` repartia el
//! trabajo en `n` partes iguales numeradas `0..n-1`, y ese numero **no dice
//! nada de donde vive el obrero**: las partes 2 y 3 pueden ser los dos hilos
//! del mismo nucleo, compartiendo las mismas unidades de carga y almacen.
//!
//! Para una faena que va a memoria --un blit, por ejemplo-- doce hilos no
//! rinden el doble que seis: rinden lo que da la memoria, y el doble de
//! nucleos girando. **Repartir en 12 partes iguales es mentirle al reparto.**
//!
//! ## Y lo que hace falta para poder decidirlo es SABER QUIEN ES QUIEN
//!
//! Esa es toda la ambicion de este fichero, y por eso no hace nada mas. La
//! politica --cuantos obreros y cuales-- se decide en `crew`, con esto delante.
//!
//! # [!] EL CONTRATO QUE NO SE ROMPE
//!
//! `tramp.rs` lo dice: *"un obrero que no comparte estado no puede correr una
//! carrera, y por eso esto es seguro con los 236 `static mut` que hay ahi
//! fuera"*. Aqui **cada obrero escribe SOLO en su ranura** y todo es atomico.
//! No se toca ni CABINA, ni el planificador, ni un driver. `rdtsc` va inline a
//! proposito: llamar al que ya existe en `task::scheduler` haria que un AP
//! entrase en un modulo del que no debe saber nada.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Tope de obreros con ficha. El 5600X trae 12; se deja sitio para un Threadripper
/// sin que nadie tenga que acordarse de subirlo.
pub const MAX_OBREROS: usize = 32;

// -- QUE ESTA HACIENDO -------------------------------------------------------
/// Ni ha llegado. Es el valor inicial, y distinguirlo de ESPERANDO importa: un
/// nucleo que no arranco y uno que arranco y no tiene faena se parecen mucho.
pub const DORMIDO: u32 = 0;
/// Vivo y girando, sin encargo.
pub const ESPERANDO: u32 = 1;
/// Dentro de una faena. Si se queda aqui, se quedo colgado.
pub const TRABAJANDO: u32 = 2;
/// Se le mando parar: `cli; hlt` y de ahi no vuelve sin INIT+SIPI.
pub const PARADO: u32 = 3;

static APIC: [AtomicU32; MAX_OBREROS] = [const { AtomicU32::new(u32::MAX) }; MAX_OBREROS];
static ESTADO: [AtomicU32; MAX_OBREROS] = [const { AtomicU32::new(DORMIDO) }; MAX_OBREROS];
static ENCARGOS: [AtomicU32; MAX_OBREROS] = [const { AtomicU32::new(0) }; MAX_OBREROS];
static CICLOS: [AtomicU64; MAX_OBREROS] = [const { AtomicU64::new(0) }; MAX_OBREROS];

/// Lo que se puede contar de un obrero sin pararlo.
#[derive(Clone, Copy)]
pub struct Retrato {
    pub apic: u32,
    /// `None` cuando el perfil no permite saberlo. Ver `smp::donde_vive`.
    pub nucleo: Option<u8>,
    pub hilo: Option<u8>,
    pub estado: u32,
    pub encargos: u32,
    pub ciclos: u64,
}

/// **Un obrero se presenta.** Lo primero que hace al entrar en su bucle.
pub fn alta(indice: u32, apic: u32) {
    let i = indice as usize;
    if i >= MAX_OBREROS {
        return;
    }
    APIC[i].store(apic, Ordering::SeqCst);
    ESTADO[i].store(ESPERANDO, Ordering::SeqCst);
}

/// Cambiar de estado. Barato a proposito: se llama dos veces por faena.
pub fn marcar(indice: u32, estado: u32) {
    if let Some(r) = ESTADO.get(indice as usize) {
        r.store(estado, Ordering::SeqCst);
    }
}

/// **Un encargo terminado**, con lo que costo.
pub fn apuntar(indice: u32, ciclos: u64) {
    let i = indice as usize;
    if i >= MAX_OBREROS {
        return;
    }
    ENCARGOS[i].fetch_add(1, Ordering::SeqCst);
    CICLOS[i].fetch_add(ciclos, Ordering::SeqCst);
    ESTADO[i].store(ESPERANDO, Ordering::SeqCst);
}

/// `rdtsc`, inline. Ver la nota del contrato en la cabecera.
pub fn ciclos() -> u64 {
    let (alto, bajo): (u32, u32);
    unsafe {
        core::arch::asm!("rdtsc", out("eax") bajo, out("edx") alto,
                         options(nomem, nostack, preserves_flags));
    }
    ((alto as u64) << 32) | (bajo as u64)
}

/// El retrato de un obrero. Se lee mientras trabaja: ninguna de las cuatro
/// lecturas lo para ni lo espera.
pub fn retrato(indice: u32) -> Retrato {
    let i = indice as usize;
    if i >= MAX_OBREROS {
        return Retrato { apic: u32::MAX, nucleo: None, hilo: None,
                         estado: DORMIDO, encargos: 0, ciclos: 0 };
    }
    let apic = APIC[i].load(Ordering::SeqCst);
    // ** Se pregunta al UNICO juez, y aqui se puede porque esto lo llama el
    // BSP: `donde_vive` lee el perfil, y el contrato dice que un AP no toca
    // estado del kernel. El obrero solo guardo su APIC.
    let (nucleo, hilo) = match super::donde_vive(apic) {
        Some((n, h)) => (Some(n), Some(h)),
        None => (None, None),
    };
    Retrato {
        apic,
        nucleo,
        hilo,
        estado: ESTADO[i].load(Ordering::SeqCst),
        encargos: ENCARGOS[i].load(Ordering::SeqCst),
        ciclos: CICLOS[i].load(Ordering::SeqCst),
    }
}

/// **La ficha de un APIC ID concreto**, si ese nucleo llego a presentarse.
///
/// [!] Hace falta porque las fichas se numeran por ORDEN DE LLEGADA y la tabla
/// del shell recorre APIC IDs. Los dos numeros coinciden casi siempre y **casi
/// no es siempre**: el orden de llegada depende de quien gane el `fetch_add`.
pub fn por_apic(apic: u32) -> Option<Retrato> {
    for i in 0..MAX_OBREROS {
        if APIC[i].load(Ordering::SeqCst) == apic {
            return Some(retrato(i as u32));
        }
    }
    None
}

/// El nombre del estado, para pintarlo.
pub fn nombre_estado(e: u32) -> &'static str {
    match e {
        ESPERANDO => "esperando",
        TRABAJANDO => "TRABAJANDO",
        PARADO => "parado",
        _ => "dormido",
    }
}

/// Cuantas fichas se han dado de alta.
pub fn dados_de_alta() -> u32 {
    let mut n = 0;
    for i in 0..MAX_OBREROS {
        if APIC[i].load(Ordering::SeqCst) != u32::MAX {
            n += 1;
        }
    }
    n
}

/// **UN OBRERO POR NUCLEO FISICO**, que es la pregunta que abrio este fichero.
///
/// Devuelve cuantos indices se escribieron en `fuera`, con el PRIMER hilo de
/// cada nucleo distinto. Para una faena que va a memoria, esta es la lista que
/// hay que usar: el segundo hilo de un nucleo comparte sus unidades de carga y
/// almacen, asi que sumarlo cuesta calor y no da ancho.
///
/// [!] Si el perfil no deja saber quien comparte nucleo con quien, cada ficha
/// contesta `None` y **aqui sale una lista VACIA, no una inventada**. Quien
/// pregunte tendra que repartir por hilos --que es lo que se hacia antes-- pero
/// sabiendolo.
pub fn primeros_de_cada_nucleo(fuera: &mut [u32]) -> u32 {
    let mut vistos = [false; MAX_OBREROS];
    let mut n = 0usize;
    for i in 0..MAX_OBREROS {
        if n >= fuera.len() {
            break;
        }
        let r = retrato(i as u32);
        let Some(nucleo) = r.nucleo else { continue };
        let nucleo = nucleo as usize;
        if nucleo >= MAX_OBREROS || vistos[nucleo] {
            continue;
        }
        vistos[nucleo] = true;
        fuera[n] = i as u32;
        n += 1;
    }
    n as u32
}
