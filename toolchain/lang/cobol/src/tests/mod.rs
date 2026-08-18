//! **Las pruebas del compilador de COBOL, por CATEGORIA.**
//!
//! Estaban las 167 en un solo `mod tests` dentro de `lib.rs`, y por eso `lib.rs`
//! media **3687 lineas: 193 de compilador y el resto de pruebas**. El fichero
//! del compilador era, en realidad, un almacen de tests con una API pegada
//! arriba.
//!
//! ## Por que importa, y no es orden por el orden
//!
//! Un test que falla decia su nombre y nada mas. `rounded_respeta_el_signo`
//! rojo entre otras ciento sesenta y seis, en un fichero de tres mil lineas, no
//! dice **de que parte del lenguaje se trata** hasta que lo buscas a mano.
//!
//! Ahora **el fichero ES la categoria**: `rounded::rounded_respeta_el_signo`.
//! Antes de leer una linea del test ya sabes que se rompio, y `cargo test
//! rounded::` corre esa parte sola.
//!
//! El corte no reescribio ni un test: se movieron bloques enteros y la cuenta
//! salio identica --217 verdes antes, 217 despues--, que es la unica prueba que
//! vale de que un refactor de pruebas no se comio nada.
//!
//! ## Las categorias
//!
//! Siguen el orden de `PLAN_BANCA.md`, no el alfabetico del disco: lo que
//! decide donde va un test es **que capacidad del lenguaje ejercita**.
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
//! - **Y la prueba de verdad**: `banca` -- los ejemplos completos de
//!   `examples/`, que son los unicos que comprueban que las piezas juntas hacen
//!   algo que un banco reconoceria.

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
mod calculadora;
