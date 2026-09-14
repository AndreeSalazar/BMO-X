//! **BMO PUERTA RED** -- el GATE RED: la puerta que se paga UNA vez, el buzon
//! por el que despues no hay puerta, y el radar que mira desde fuera.
//!
//! generacion: nieto -- la forma del buzon y los veredictos; no sabe que es una tarjeta
//! capa: puro -- ni un `unsafe`, ni un aparato: lo usan el kernel y Ring 3
//!
//! # E3 de `docs/plan/PLAN_RED_TX.md` (2026-09-13)
//!
//! Eddi: *"burocratico en sentido de firmas pero si es valido, y que no juegue
//! con sorpresas ... una sola vez paga y luego sin burocracia, pero viene con
//! radar de 4 ms"*. Son los cuatro tiempos de `docs/identidad/LA_RUTA.md`,
//! aplicados a la red:
//!
//! ```text
//!    1. DECLARAR   INVOKE(RED_OP_ABRIR, ms, cupo)               [`pase`]
//!    2. JUZGAR     el kernel pregunta en orden, UNA vez, y cada NO tiene nombre
//!    3. CORRER     tramas por el buzon mapeado, CERO syscalls   [`buzon`]
//!                  y WAIT sobre el handle para dormir hasta que llegue algo
//!    4. VIGILAR    cada 4 ms, desde fuera: indices, grifo y plazo   [`radar`]
//!                  -> si no cuadra, se REVOCA: desmapeo en caliente
//! ```
//!
//! # *** Lo que Ring 3 NUNCA ve: el corral de la tarjeta
//!
//! El proceso escribe en SU buzon; el kernel COPIA la trama a memoria suya y
//! **solo esa copia** pasa por el grifo y llega al descriptor. Si la tarjeta
//! leyera del buzon, el proceso podria cambiar la trama DESPUES de juzgada y
//! antes de salir (TOCTOU), y el grifo seria decoracion.
//!
//! # [!] Por que vive aqui y no en `bmo-net`
//!
//! Porque lo leen los DOS anillos con la misma aritmetica, y L8 no deja que
//! Ring 3 enlace un driver. Es el mismo argumento que `bmo-foco`.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod buzon;
pub mod pase;
pub mod radar;
