//! Los tipos que dos lenguajes (o un lenguaje y la maquina) tienen que
//! acordar, y nada mas:
//!
//! ```text
//!    convention    por donde viaja cada argumento: lo IMPORTAN los emisores
//!                  de C e INTI
//!    disposicion   donde cae cada miembro de un agregado: la comparten C,
//!                  C++, COBOL e INTI
//! ```
//!
//! ** Hasta el 2026-09-19 esta cabecera describia `TypeField`,
//! `FunctionSignature` y un `TypeRegistry` del kernel que cargaba una seccion
//! `.type_map` "al arrancar". Los dos primeros no tenian usuario y se fueron;
//! el tercero no existio nunca.

pub mod convention;
/// La regla de disposicion de agregados -- donde cae cada miembro y cuanto
/// mide el conjunto. **Una sola copia**, compartida por los frontends: estaba
/// escrita tres veces y una divergencia no da un error, da un programa que
/// escribe en el campo de al lado.
pub mod disposicion;

pub use convention::*;
pub use disposicion::{alinear, alineado_de, ranuras, Disposicion, DisposicionUnion};
