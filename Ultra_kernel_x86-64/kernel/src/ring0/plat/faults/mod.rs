//! On-screen CPU fault reporter.
//!
//! [carril]  VERDE     el reparto de los tres carriles de al lado
//!
//! The faggin `s1_cpu` stage installs exception handlers that print to COM1
//! serial and halt. On a headless machine (no serial cable) a Ring 3 fault
//! therefore freezes the display with no clue why. This module patches the
//! live IDT (`ctx.idt_ptr`, same table `timer::init` patches for vector 48)
//! so the most common faults paint their vector / error code / faulting RIP
//! / CR2 into the dashboard log before halting -- making a CPL3 crash visible.
//!
//! The handlers are terminal: they gather state, draw, and `hlt` forever. No
//! register save/restore is needed because control never returns.


//! # ** LOS TRES CARRILES (L6g)
//!
//! ```text
//!    roja.rs       la IDT, los stubs y el reparto. Si falla, no hay pantalla
//!                  que lo cuente: es un triple fault
//!    amarilla.rs   el informe y la pantalla azul. No es peligroso de
//!                  EJECUTAR: es peligroso de CREER, y cambia cada semana
//!    verde.rs      `Line`, los colores y el plazo. Equivocarse pinta feo
//! ```
//!
//! *** El corte que mas dice es el de en medio. `fault_report` "solo imprime",
//! y aun asi es el que mas caro ha salido: tres veces en una semana enseno algo
//! que no era, y cada una mando a mirar donde no estaba el fallo.
//!
//! [!] Fuera no cambia nada: `pub use` deja el modulo con la misma cara.

mod amarilla;
mod roja;
mod verde;

pub use roja::{init, contexto_podrido, PODRIDO_CABECERA, PODRIDO_CS, PODRIDO_SELLO};

