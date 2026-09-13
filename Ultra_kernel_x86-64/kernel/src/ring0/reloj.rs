//! **EL RELOJ DEL KERNEL** -- el contador del tick, y nada mas.
//!
//! [familia] reloj  nivel 0 -- cuantos ticks van desde el arranque; no llama a nadie
//! [conecta] -
//!
//! [carril]  VERDE     un atomico que sube; leerlo no puede romper nada
//! [consumo] NADA      no corre: lo avanza el tick del LAPIC y lo lee quien quiera
//!
//! ## Por que es una familia de nivel 0 (L8b, 2026-09-13)
//!
//! Vivia dentro de `plat/timer.rs`. Y CABINA --que todos llaman para apuntar lo
//! que pasa-- tenia que sellar cada evento con la hora, asi que el registro mas
//! bajo del sistema importaba la plataforma: uno de los hilos del nudo de 13
//! subsistemas del kernel.
//!
//! La hora no es de la plataforma: es un dato que la plataforma PRODUCE. Aqui
//! vive el dato; `plat::timer` lo avanza (arriba importa abajo) y
//! `plat::timer::ticks()` sigue existiendo y le delega, para que los que ya lo
//! leian no cambien.

use core::sync::atomic::{AtomicU64, Ordering};

static TICKS: AtomicU64 = AtomicU64::new(0);

/// Ticks del LAPIC desde el arranque.
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Avanza un tick y devuelve el nuevo valor. Solo lo llama el manejador del
/// tick (`plat::timer`).
pub(crate) fn avanzar() -> u64 {
    TICKS.fetch_add(1, Ordering::Relaxed) + 1
}
