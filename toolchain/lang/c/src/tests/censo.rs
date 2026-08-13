//! **EL ARNES DEL CENSO** -- barrer una matriz de casillas y comparar el
//! informe entero contra lo que estaba escrito.
//!
//! ## Por que esto es un modulo y no una funcion copiada
//!
//! El patron nacio en `sonda_del_lenguaje` y funciono: convirtio cuatro
//! arranques del Ryzen en una ejecucion de 0,25 s. En cuanto hizo falta un
//! SEGUNDO eje --la disposicion de los agregados, que es lo que DOOM necesita
//! en `R_Init`-- la eleccion era copiar cuarenta lineas o sacarlas aqui.
//!
//! Copiarlas habria sido el patron 26 otra vez con otro disfraz: dos copias de
//! la misma regla, y la segunda arreglandose sola cuando alguien se acuerde.
//!
//! ## Las tres propiedades que hay que conservar, y ninguna es accidental
//!
//! 1. **Una casilla rota no tapa a las demas.** Cada una va dentro de un
//!    `catch_unwind`, asi que una que ni compile se anota y el barrido sigue.
//!    Un censo que se para en el primer hueco no es un censo.
//! 2. **La suite se queda en VERDE con defectos abiertos**, porque el censo
//!    dice la verdad -- incluido lo que no funciona. Un `ROTO` con su sintoma
//!    al lado es mas util que una fila en un `TODO`.
//! 3. ** **En cuanto la realidad cambie, el test falla.** Se arregle un `ROTO`
//!    o se rompa un `BIEN`, el informe deja de coincidir con la constante y
//!    hay que actualizarla. Es la cura del documento que miente, que en esta
//!    casa ya se ha pagado varias veces.

use super::*;

/// Una casilla del censo: como se llama, que programa la ejerce, que tiene que
/// imprimir.
pub(super) struct Casilla {
    pub nombre: &'static str,
    pub fuente: &'static str,
    pub espera: &'static str,
}

/// Corre todas las casillas y compara el informe con `esperado`.
///
/// [!] El hook de panico se calla mientras dura el barrido: si no, la salida se
/// llena de trazas de las casillas rotas y el informe --que es lo que hay que
/// leer-- se pierde entre ellas.
pub(super) fn barrer(casillas: &[Casilla], esperado: &str, aviso: &str) {
    let anterior = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let mut informe = String::new();
    for c in casillas {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_c_con_pp(c.fuente)));
        let veredicto = match r {
            Ok(salida) if salida.trim() == c.espera => "BIEN".to_string(),
            Ok(salida) => format!("ROTO da {:?} y toca {:?}", salida.trim(), c.espera),
            Err(_) => "NO COMPILA o revienta".to_string(),
        };
        // El ancho es 30 y no el del nombre mas largo a proposito: los tres
        // que se pasan empujan su veredicto una columna, y eso los senala en
        // el informe sin necesidad de una marca.
        informe.push_str(&format!("{:<30} {}\n", c.nombre, veredicto));
    }

    std::panic::set_hook(anterior);

    assert_eq!(informe.trim_end(), esperado.trim_end(), "\n\n{aviso}\n");
}
