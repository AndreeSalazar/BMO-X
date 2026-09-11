//! **EL RELOJ** -- el TSC, y lo unico que el dibujo necesita de el.
//!
//! [carril]  VERDE     lo unico que el dibujo necesita del TSC
//! [consumo] NADA      solo corre en el arranque
//!
//! ## Por que es un modulo y no dos lineas sueltas en medio del dibujo
//!
//! Porque no es dibujo. Estaba en mitad de `splash.rs` entre `fill_rect` y la
//! fuente, y el resultado es que quien buscaba "por que la intro dura lo que
//! dura" tenia que leer mil quinientas lineas de pintar rectangulos para
//! encontrar dos funciones de veinte.
//!
//! ## ** LA DECISION QUE HAY AQUI DENTRO: la animacion se guia por el RELOJ
//!
//! [`ms_desde`] es lo que separa una animacion de una secuencia de dibujos. Un
//! bucle que avanza un paso fijo por vuelta dura **lo que tarde en pintar**: en
//! un panel de 1080p la ciudad son ~8 MB por fotograma a memoria
//! write-combining, o sea decenas de milisegundos que no son los mismos en 720p
//! que en 4K. Preguntandole al reloj, la animacion dura lo que dice durar y lo
//! unico que cambia con el panel es cuantos fotogramas caben dentro.

/// TSC-based busy-wait. Reads TSC directly.
#[inline]
pub(crate) fn tsc_read() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi); }
    ((hi as u64) << 32) | lo as u64
}

#[inline]
pub(crate) fn tsc_wait(cycles: u64) {
    let start = tsc_read();
    while tsc_read() - start < cycles {
        core::hint::spin_loop();
    }
}

/// Milisegundos desde `origen`, contados con el TSC.
///
/// El bucle de la animacion se guia por ESTO y no por contar fotogramas, y es la
/// diferencia entre una animacion y una secuencia de dibujos.
///
/// Un bucle que avanza un paso fijo por vuelta dura lo que tarde en pintar: en
/// un panel de 1080p la ciudad son ~8 MB por fotograma a memoria
/// write-combining, o sea decenas de milisegundos que **no son los mismos** en
/// 720p que en 4K. Preguntandole al reloj, la animacion dura lo que dice durar y
/// lo unico que cambia con el panel es cuantos fotogramas caben dentro.
pub(crate) fn ms_desde(origen: u64) -> u32 {
    let f = crate::ring0::task::scheduler::tsc_freq();
    if f == 0 {
        return 0;
    }
    (tsc_read().wrapping_sub(origen) / (f / 1000)) as u32
}

// ** AQUI VIVIA `hold_ms`, y era el motor de la siesta (retirado 08-09).
//
// Un `spin_loop` que se comia el CPU los milisegundos que le pidieran. Lo mato
// el truco de Santa Monica --la intro dejo de esperar y paso a taparse con el
// trabajo de verdad-- y el mismo dia se retiro `GATO_MS`, que era su unico
// argumento: 1.600 ms de sostener el gato.
//
// Se borra la funcion y no solo la llamada: **una espera que gira, viva en el
// arbol y sin llamador, es una invitacion**. El dia que alguien quiera esperar
// tiene que encontrarse con `WAIT` y con `park_until`, que ceden el turno --no
// con esto, que se lo queda. Ver `docs/plan/PLAN_EL_PLAZO.md`, P2.
