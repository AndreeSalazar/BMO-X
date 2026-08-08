//! **Servicio de entrada -- NO EXISTE, y esto lo dice en vez de fingirlo.**
//!
//! === Lo que hay aqui ===
//!
//! Tres funciones que devuelven `None` y un `init()` vacio. Nadie las llama, y
//! **este crate no tiene un solo dependiente en el workspace**.
//!
//! === Por que no hace falta ===
//!
//! La idea era un multiplexor: recoger teclado y raton y repartirlos a la
//! ventana con el foco. Ese trabajo **ya lo hace otro**, y de otra forma:
//!
//! - El driver USB vive en `platform/drivers/usb/uhid` (xHCI + HID), en Ring 0.
//! - El kernel lo expone como una capability, `KIND_INPUT`.
//! - **El compositor la reclama directamente** y reparte con `bmo_input::foco`,
//!   cuya politica tiene 17 tests y esta verificada en el Ryzen.
//!
//! O sea que la entrada llega a quien tiene que llegar sin pasar por aqui. Un
//! multiplexor en medio seria hoy un salto de proceso mas para no decidir nada
//! nuevo.
//!
//! === El comentario que estaba aqui, y por que importa ===
//!
//! Decia que los drivers de teclado y raton viven en
//! `Ultra_kernel_x86-64/kernel/src/ring0/irq/`. **Esa carpeta no existe** --
//! comprobado el 2026-08-02. Es de cuando la entrada entraba por IRQ del 8042,
//! dos reorganizaciones atras.
//!
//! Un stub que encima apunta a una ruta falsa no es neutral: manda a buscar a
//! quien venga a entender como funciona la entrada, y le cuesta el rato de
//! descubrir que el mapa esta mal antes de poder empezar.
//!
//! === Que hacer con esto ===
//!
//! **Cablear o borrar**, la regla de siempre. Y hoy se inclina a borrar: el dia
//! que haga falta un servicio de entrada de verdad sera *despues* de
//! `KIND_SUPERFICIE`, cuando haya ventanas de otros procesos a las que
//! repartir -- y ese servicio no se parecera a estas tres firmas. Se conserva
//! solo hasta que el dueno decida, con el aviso escrito para que nadie
//! construya encima mientras tanto.

#![no_std]

/// No hace nada. Ver la cabecera del modulo.
pub fn init() {}
/// Siempre `None`. El teclado lo reclama el compositor con `KIND_INPUT`; no
/// pasa por aqui.
pub fn poll_keyboard() -> Option<u8> { None }
/// Siempre `None`. Ver [`poll_keyboard`].
pub fn poll_mouse() -> Option<u32> { None }
