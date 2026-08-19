//! # MAQUETA -- EMIT, un CONSUMIDOR (no una generacion)
//!
//! generacion: ninguna -- es el CONSUMIDOR de la cadena
//!
//! ** Ninguna de las cinco generaciones sabe que este crate existe, y eso se
//! comprueba mirando los `Cargo.toml`: las flechas van `lex -> node -> cascade ->
//! layout -> verdict`, y ninguna apunta aqui.
//!
//! ## El reparto de dentro, y por que se corto asi
//!
//! ```text
//!    orden.rs     QUE se dibuja    una lista de trazos, sin destino
//!    recorte.rs   QUE cae dentro   filtra y corta esa lista a un rectangulo
//!    rust.rs      COMO se escribe  traduce; no decide
//! ```
//!
//! Antes `rust.rs` mezclaba las tres. Con un solo destino no se notaba; con tres
//! --Rust, el recurso BEF, el reflejo en PPM-- cada uno habria vuelto a deducir
//! *que hay que dibujar*, y **tres deducciones de la misma cosa son tres sitios
//! donde puede salir distinta**.
//!
//! ## ** Y es lo que hace diagnosticable un fotograma
//!
//! Un recorte mal hecho no da un error de compilacion: da basura en pantalla, o
//! un trozo que no se repinta. Con la lista separada, *"por que salio mal este
//! fotograma"* deja de ser una lectura del codigo generado y pasa a ser
//! **filtrar una lista y mirarla** -- `recorte::dentro`, probado en el anfitrion
//! sin arrancar nada.
//!
//! ```text
//!    rust.rs    HECHO      codigo, para compilar dentro del servicio
//!    bef.rs     escalon 8  un recurso, para cambiar la cara SIN recompilar
//!    ppm.rs     escalon 9  el reflejo, con el rasterizador de verdad
//! ```

#![forbid(unsafe_code)]

pub mod orden;
pub mod recorte;
pub mod rust;
