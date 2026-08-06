//! **Las pruebas del compilador de COBOL, por CATEGORÍA.**
//!
//! Estaban las 167 en un solo `mod tests` dentro de `lib.rs`, y por eso `lib.rs`
//! medía **3687 líneas: 193 de compilador y el resto de pruebas**. El fichero
//! del compilador era, en realidad, un almacén de tests con una API pegada
//! arriba.
//!
//! ## Por qué importa, y no es orden por el orden
//!
//! Un test que falla decía su nombre y nada más. `rounded_respeta_el_signo`
//! rojo entre otras ciento sesenta y seis, en un fichero de tres mil líneas, no
//! dice **de qué parte del lenguaje se trata** hasta que lo buscas a mano.
//!
//! Ahora **el fichero ES la categoría**: `rounded::rounded_respeta_el_signo`.
//! Antes de leer una línea del test ya sabes qué se rompió, y `cargo test
//! rounded::` corre esa parte sola.
//!
//! El corte no reescribió ni un test: se movieron bloques enteros y la cuenta
//! salió idéntica —217 verdes antes, 217 después—, que es la única prueba que
//! vale de que un refactor de pruebas no se comió nada.
//!
//! ## Las categorías
//!
//! Siguen el orden de `PLAN_BANCA.md`, no el alfabético del disco: lo que
//! decide dónde va un test es **qué capacidad del lenguaje ejercita**.
//!
//! - **La base**: `compilacion` (parsea, emite BEF/BEX, la puerta del syscall),
//!   `aritmetica`, `condiciones`, `grupos`, `tablas`, `value`.
//! - **La estructura**: `parrafos`, `evaluate`, `perform_varying`, `go_to`,
//!   `nivel88`.
//! - **El dato de banco**: `comp3` (decimal empaquetado), `rounded` (los seis
//!   modos), `desbordes` (`ON SIZE ERROR`), `texto`, `texto_compuesto`
//!   (`INSPECT`/`STRING`).
//! - **El fichero**: `ficheros`, `file_status`, `binario` (registros de ancho
//!   fijo y el visor).
//! - **Y la prueba de verdad**: `banca` — los ejemplos completos de
//!   `examples/`, que son los únicos que comprueban que las piezas juntas hacen
//!   algo que un banco reconocería.

mod comun;

mod compilacion;
mod aritmetica;
mod condiciones;
mod grupos;
mod tablas;
mod value;

mod parrafos;
mod evaluate;
mod perform_varying;
mod go_to;
mod nivel88;

mod comp3;
mod rounded;
mod desbordes;
mod texto;
mod texto_compuesto;

mod ficheros;
mod file_status;
mod binario;

mod banca;
