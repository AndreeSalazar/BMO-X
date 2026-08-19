//! **EL DIRECTORIO** -- entradas de 32 bytes, y los nombres que las cruzan.
//!
//! Se parte en tres porque son tres preguntas distintas:
//!
//! ```text
//!   larga.rs     EL NOMBRE DE VERDAD    UCS-2, la suma de control  [x] puro
//!   corta.rs     LA ENTRADA DE 32 BYTES 8.3, atributos, cluster    [ ]
//!   recorrer.rs  QUE HAY AQUI DENTRO?   juntar los dos, y crecer   [ ]
//! ```
//!
//! `larga` va primero a proposito: no toca el disco, asi que su censo existe
//! desde el primer dia. `recorrer` es el unico de los tres que necesitara
//! sectores, y por eso es el ultimo.

pub mod larga;
