//! **CARRIL VERDE -- LAS CUENTAS: lo que solo MIRA.**
//!
//! [carril]  VERDE     `cubiertos`, `vuelos` y `neutros`
//!
//! [cuesta]  NADA -- ni un `mut` sale de aqui. Equivocarse devuelve un numero
//!           feo a quien pregunto, y decide ese quien (L6e)
//!
//! [riesgo]  -- ninguno declarado.
//!
//! # ** DOS DE LAS TRES NO RECORREN NADA, Y ESO ES UNA DECISION
//!
//! ```text
//!    vuelos()     tres `static`         O(1)   cabe en un panel a 60 Hz
//!    neutros()    dos `static`          O(1)   cabe en un panel a 60 Hz
//!    cubiertos()  recorre la tabla      O(n)   4.194.304 entradas: NO cabe
//! ```
//!
//! *** Las dos primeras llevan la cuenta desde el lado del MARCADO --carril
//! amarillo-- precisamente para poder ser O(1). La primera version de
//! `neutros` recorria, y una cifra que no se puede repintar es una cifra que
//! el dueno no va a mirar: acaba en una orden que hay que acordarse de
//! escribir, o sea en ninguna parte.
//!
//! ** Por eso `vuelo=V:P:C` esta delante de la cara en CABINA y `cubiertos`
//! no: el precio de una cuenta decide DONDE se puede ensenar.

use super::{tabla, MARCOS, EN_VUELO_CHOQUES, EN_VUELO_PISADOS, EN_VUELO_VIVOS,
            NEUTROS_SOLTADOS, NEUTROS_VIVOS};


/// **Cuantos marcos llevan una etiqueta de verdad**, o sea ni libres ni
/// anonimos.
///
/// Es la unica defensa contra el `[riesgo] SILENCIO`: si esto se queda en
/// cifras bajas, el juez esta callado porque no sabe, no porque todo vaya bien.
/// Lo dice el informe de la purga, junto a los marcos que volvieron.
pub fn cubiertos() -> u64 {
    let t = tabla();
    let mut n = 0u64;
    for i in 0..MARCOS {
        let b = t[i];
        if b >= 2 {
            n += 1;
        }
    }
    n
}


/// **Las tres cuentas del vuelo**: `(vivos, pisados, choques)`.
///
/// ```text
///    vivos     lo que hay ahora. Al apagar, CERO
///    pisados   ** CERO. Un marco reasignado con DMA dentro
///    choques   ** CERO. Dos aparatos pidiendo el mismo bufer
/// ```
///
/// Las tres son de leer un `static`: preguntar esto no recorre nada, asi que
/// cabe en un panel que se repinta -- igual que [`neutros`].
pub fn vuelos() -> (u64, u64, u64) {
    unsafe { (EN_VUELO_VIVOS, EN_VUELO_PISADOS, EN_VUELO_CHOQUES) }
}


/// **`(marcos de aparato, veces que uno se solto)`.** La cifra de `NEUTRO/`.
///
/// Va aparte de [`cubiertos`] a proposito. `cubiertos` contesta *"cuanto sabe
/// el juez"*; esto contesta *"cuanta RAM de esta maquina esta fuera del celo"*.
///
/// ```text
///    vivos     PEQUENO Y QUIETO. Los aparatos piden al arrancar y ya.
///              Si sube en marcha, alguien reparte DMA en caliente
///    soltados  ** CERO. Cualquier otra cosa es N3 rota, y con su cuenta
/// ```
///
/// Las dos son de leer un `static`: la cuenta la lleva [`marcar`], que es el
/// unico sitio por el que un marco cambia de dueno. Preguntar esto **no
/// recorre nada**, asi que se puede poner en un panel que se repinta.
pub fn neutros() -> (u64, u64) {
    unsafe { (NEUTROS_VIVOS, NEUTROS_SOLTADOS) }
}
