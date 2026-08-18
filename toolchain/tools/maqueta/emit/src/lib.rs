//! # MAQUETA -- EMIT, un CONSUMIDOR (no una generacion)
//!
//! ★ **Ninguna de las cinco generaciones sabe que este crate existe**, y eso se
//! comprueba mirando los `Cargo.toml`: las flechas van `lex -> node -> cascade ->
//! layout -> verdict`, y ninguna apunta aqui.
//!
//! Esa es la ley L7 --*el conocimiento solo baja: ninguna generacion sabe quien
//! la consume*-- pagando lo que prometio: la eleccion entre **emitir Rust** y
//! **emitir un recurso BEF 0x0B** no habia que tomarla el primer dia, y sigue sin
//! haber que tomarla. Se anade un modulo aqui y no se toca ni una linea de la
//! cadena.
//!
//! ```text
//!    rust.rs    HECHO      codigo, para compilar dentro del servicio
//!    bef.rs     escalon 8  un recurso, para cambiar la cara SIN recompilar
//!    ppm.rs     escalon 9  el reflejo, con el rasterizador de verdad
//! ```

#![forbid(unsafe_code)]

pub mod rust;
