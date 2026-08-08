//! `bmo-lower` -- **L1: el descenso generico al BMO ABI**.
//!
//! # Su lugar en el pipeline
//!
//! ```text
//! L2  ESPECIALIZADA (una por lenguaje, jamas se mezclan)
//!     C: printf(fmt,...)      COBOL: DISPLAY con PIC      C++: cout <<
//!     varargs, %d/%s        edicion decimal ZZ9,99      operator<<
//!               |                     |                      |
//!               +---------------------+----------------------+
//!                                     v
//! L1  GENERICA  <- ESTA CRATE: "escribe estos bytes", nada mas
//!                                     v
//! L0  SUPERFICIE CONGELADA (bmo_abi::syscalls::surface)
//!     INVOKE - CHANNEL_KICK - WAIT
//! ```
//!
//! # La regla que mantiene esto modular
//!
//! > **L1 solo contiene lo expresable en la superficie congelada por valor.
//! > Todo lo que tenga semantica de lenguaje --formato `%d`, edicion PIC,
//! > `operator<<`-- se queda en L2.**
//!
//! Esa frontera es lo que impide que esta crate degenere en un embudo de
//! minimo comun denominador. Aqui no se sabe que lenguaje llamo, y ese es
//! exactamente el punto: cuando entre un cuarto frontend, no se toca nada.
//!
//! # Por que existe
//!
//! Antes de esto, `lang/c/codegen.rs` y `lang/cobol/codegen.rs` emitian cada
//! uno su propia "impresion" contra numeros de syscall planos (`0x1F0`,
//! `NR_DEBUG_PRINT`) que el kernel **ya no despacha**, y encima pasaban un
//! puntero, cosa que la superficie congelada rechaza por diseno. Ninguno de
//! los dos imprimia nada en hardware. Un solo emisor correcto, compartido,
//! elimina la clase entera de bug.
//!
//! # Lo que emite
//!
//! Codigo x86-64 crudo, apendizado a un `Vec<u8>`. No conoce secciones,
//! relocations ni el escritor BEF: el frontend que lo llama ya tiene todo
//! eso. Por eso `console::write_const` no necesita meter la cadena en
//! `.rodata` -- el texto viaja **dentro de las instrucciones**, como
//! inmediatos, asi que no hay fixup que parchear ni puntero que cruzar.

/// La puerta de ARCHIVOS. Hermana de `console`: mueve bytes sobre
/// `KIND_ARCHIVO` y no sabe que es un registro ni una PICTURE.
pub mod archivo;
pub mod console;
/// Formateo de valores a texto. LIBRERIA compartida, no puerta: ver
/// la cabecera de `fmt.rs` para por que vive aqui y no en cada frontend.
pub mod fmt;
/// Bloques de bytes: copiar, rellenar, comparar, medir. Lo que C escribe
/// `memcpy` y COBOL escribe `MOVE` de un grupo: **la misma emision**.
pub mod memoria;
/// Decimal EMPAQUETADO (BCD). LIBRERIA compartida por la misma razon que
/// `fmt`: los nibbles del `COMP-3` de COBOL, del `Decimal` de Ada y del
/// `FIXED DECIMAL` de PL/I son los mismos. Ver la cabecera de `packed.rs`.
pub mod packed;
/// Redondeo decimal. LIBRERIA compartida por la misma razon que `fmt` y
/// `packed`: partir un entero y decidir el ultimo digito es aritmetica, no la
/// semantica de un lenguaje. Ver la cabecera de `redondeo.rs` para por que van
/// los SEIS modos y no uno.
pub mod redondeo;
pub mod task;
/// Operaciones sobre bloques de TEXTO: contar, sustituir. Hermana de `memoria`
/// -aquello son los verbos de C, esto los que COBOL escribe `INSPECT`- y con la
/// misma frontera: el largo va explicito, aqui no hay NUL que buscar.
pub mod texto;
/// Decimal ZONADO (`DISPLAY`): un byte por digito y el signo sobrepunzado en el
/// ultimo. La otra mitad de `packed` -- las dos son como un numero vive en un
/// FICHERO. Ver la cabecera de `zoned.rs`.
pub mod zoned;

/// El emisor de instrucciones, PUBLICO desde que hay frontends que necesitan
/// abrir hueco en la pila antes de llamar a `console::read_line`.
///
/// Sigue siendo un emisor, no un cerebro: sabe poner bytes de una instruccion
/// y nada mas. Quien decide QUE instrucciones y en que orden es el lenguaje.
pub mod x86;

/// Banco de pruebas: ejecuta el codigo emitido en vez de comparar bytes.
///
/// Vive detras de la feature `emulator` para no viajar en las builds
/// normales, y es `pub` para que cada frontend verifique **su propio**
/// descenso --el flujo de control de COBOL, el de C-- con el mismo modelo del
/// kernel. Un `IF` que no bifurca se ve identico a uno que si en un volcado
/// de bytes; solo se distinguen ejecutandolos.
#[cfg(any(test, feature = "emulator"))]
pub mod emu;

/// Re-export de la superficie para que un frontend que enlaza `bmo-lower`
/// no tenga que declarar ademas `bmo-abi` solo para nombrar una operacion.
pub use bmo_abi::syscalls::surface;
