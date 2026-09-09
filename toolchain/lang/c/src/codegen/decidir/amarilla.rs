//! **CARRIL AMARILLO** -- LA IMAGEN. Donde cae cada cosa dentro del `.bex`.
//!
//! [carril]  AMARILLO  no calcula ningun valor del programa: calcula SITIOS.
//!                     Equivocarse aqui no cambia lo que el programa piensa,
//!                     cambia donde lo guarda.
//!
//! [cuesta]  TAREA -- un reparto mal hecho no arranca, o lo rechaza el cargador
//!           al leerlo. Se ve al momento y en el sitio, que es exactamente lo
//!           contrario del carril rojo de al lado.
//!
//! [riesgo]  ESPEJO
//!           ESPEJO -- estas cuentas las repite el CARGADOR del kernel al leer
//!                    el `.bex`. Son dos aritmeticas sobre el mismo formato en
//!                    dos anillos distintos, y el dia que se separen, el fichero
//!                    que este compilador escribe deja de ser el que aquel lee.
//!
//! # Por que estan aqui y no en el emisor
//!
//! Porque son PURAS: entran numeros, salen numeros, y no hay un `&mut self` en
//! ninguna. Esa es toda la prueba que pide la regla de `decidir/` -- si algo de
//! aqui necesitara el estado del compilador para contestar, es que no era una
//! decision: era emision disfrazada.

/// Redondea hacia arriba al multiplo de pagina. La cuenta del cargador.
pub(in crate::codegen) fn hasta_pagina(n: usize) -> usize {
    const PAGE: usize = 4096;
    (n + PAGE - 1) & !(PAGE - 1)
}

/// Cual de las regiones contiene el offset `off`. Las regiones vienen
/// ordenadas por offset y son contiguas, asi que la busqueda binaria cae en
/// la que empieza en `off` o en la inmediatamente anterior.
pub(in crate::codegen) fn region_de(regiones: &[(u32, u32, String)], off: u32) -> Option<usize> {
    match regiones.binary_search_by(|r| r.0.cmp(&off)) {
        Ok(i) => Some(i),
        Err(0) => None,
        Err(i) => Some(i - 1),
    }
}
