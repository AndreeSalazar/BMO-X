//! # EL ARNES: como se ejecutan de verdad las pruebas de esta carpeta
//!
//! **Este fichero NO forma parte del crate.** No hay ningun `mod
//! pruebas_sueltas;` en `mod.rs`, y es a proposito.
//!
//! ## El problema
//!
//! `Ultra_userspace` es `no_std` con su propio guion de enlazado, asi que
//! `cargo test` aqui choca con lo mismo que en el kernel: enlaza `std` y no hay
//! `std`. O sea que un `#[cfg(test)]` en estos modulos **no lo corre nadie**.
//!
//! Y un `#[cfg(test)]` que nadie ha corrido no es una prueba: es una intencion.
//! La cicatriz de la casa esta apuntada en `sin_gpu/sucio.rs` -- nueve pruebas
//! de coma flotante del frontend de C que estan "en verde" y **ninguna
//! ejecuta**.
//!
//! ## La salida, y por que estos tres modulos la permiten
//!
//! `recorte`, `linea` y `triangulo` no tienen ni un `unsafe`, ni un puntero, ni
//! una dependencia, ni tocan la pantalla: **son aritmetica**. Lo unico que les
//! hacia falta era no estar atados al destino, y por eso emiten por callback.
//!
//! Asi que se compilan fuera tal cual, con este fichero como raiz:
//!
//! ```text
//!    cd Ultra_userspace/userland/src/dibujo
//!    rustc --test pruebas_sueltas.rs -o pruebas && ./pruebas
//! ```
//!
//! Los `#[path]` de abajo meten los tres modulos de verdad --no una copia-- asi
//! que lo que se prueba es el fichero que se commitea. Y `super::recorte`
//! resuelve igual aqui que dentro del crate, porque en los dos casos los
//! modulos son hermanos.
//!
//! [!] Si se anade un modulo a la carpeta, **se anade tambien aqui**. Un modulo
//! que no este en esta lista no lo prueba nadie, y no habra ningun aviso.
//!
//! El dia que exista un sitio donde estas corran solas, se borra este fichero.

#[path = "recorte.rs"]
mod recorte;

#[path = "linea.rs"]
mod linea;

#[path = "triangulo.rs"]
mod triangulo;

#[path = "curva.rs"]
mod curva;
