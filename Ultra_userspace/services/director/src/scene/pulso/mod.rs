//! **EL PULSO DEL ESCRITORIO: cuantas vueltas da por segundo, siempre a la
//! vista.** Y en que se le va el segundo.
//!
//! [carril]  AMARILLO  el reparto, y hereda el color del carril que manda
//!
//! [cuesta]  NADA -- ninguno de los dos carriles rompe nada al fallar. Lo que
//!           se pierde es la CONFIANZA en el unico numero que dice si el
//!           escritorio vive, que es peor de recuperar que un fichero.
//!
//! [riesgo]  SILENCIO RELOJ
//!           los dos entran por `amarilla.rs`. `verde.rs` no declara ninguno, y
//!           eso es informacion: dice cual de las dos mitades hay que mirar
//!           primero el dia que el pulso vuelva a decir algo raro.
//!
//! # *** POR QUE ESTE FICHERO ES UNA CARPETA (L6g nivel 3, 2026-09-08)
//!
//! Lo pidio el dueno, y la razon la habia escrito ya el propio fichero sin
//! darse cuenta. El pulso ha mentido **dos veces en un dia**:
//!
//! ```text
//!    el numero sin aguja     vivo y muerto se veian IGUAL durante un segundo
//!    `pulso 0/s` sin reloj   un cero que nadie midio, apuntando al bucle
//! ```
//!
//! ** Las dos averias fueron de SIGNIFICADO. Ninguna fue de dibujo. Y las dos
//! vivian en la misma funcion que las coordenadas, o sea que un cambio de donde
//! cae un numero y un cambio de que dice ese numero **se leian igual en el
//! diff**. Ese es el corte, y no el tamano: 244 lineas no obligan a nada.
//!
//! ```text
//!    amarilla.rs   QUE ES VERDAD    se equivoca CALLANDO    -> dos manos
//!    verde.rs      DONDE VA         se equivoca A LA VISTA  -> se puede jugar
//!    mod.rs        la costura       ni decide ni pinta
//! ```
//!
//! *** Y el verde no esta ahi de relleno. **Saber que algo es verde tambien es
//! saber**: el dia que el pulso diga algo que no cuadra, este letrero dice que
//! `verde.rs` no puede ser la causa, y la lista de sospechosos se parte por la
//! mitad antes de abrir un fichero.
//!
//! # El corte se eligio por NOMBRES LIBRES, y salio exacto
//!
//! Ni un nombre cruza en los dos sentidos. `amarilla.rs` no toca `bmo::Pantalla`
//! ni una constante de geometria; `verde.rs` no tiene ni un `static mut` ni sabe
//! que es `INFO_TSC_HZ`. Lo unico que cruza es `Dictamen`, que va en una sola
//! direccion -- y que exista **es** el corte: es la decision escrita aparte de
//! su dibujo.
//!
//! # Lo que se cayo al partir, y por que se dice
//!
//! `ULTIMO` y `olvidar()`. Eran de la version que se CALLABA si el numero no
//! habia cambiado; la aguja hizo el modulo incondicional y los dejo sin
//! trabajo. Al mirarlos uno por uno para repartirlos salio que `ULTIMO` se
//! escribia dos veces y **no se leia nunca**, y que `paint.rs` seguia llamando a
//! un `olvidar()` que no olvidaba nada.
//!
//! > Eso no lo caza el compilador: un `static mut` que se escribe y no se lee
//! > no es un aviso. Lo caza repartir las piezas y no encontrarle sitio a una.
//!
//! # Como se lee, y parte el problema EN DOS (y luego en dos otra vez)
//!
//! ```text
//!    la AGUJA gira    el bucle VIVE, sea cual sea el numero
//!    la AGUJA quieta  el bucle NO da vueltas. Ahi se acaba la ambiguedad
//!
//!    cuerpo grande    el compositor GASTA el segundo   -> Ring 3
//!    puerta grande    el compositor ESPERA el segundo  -> Ring 0
//!    los dos bajos    ni gasta ni espera: hay UNA vuelta larga
//!
//!    SIN RELOJ        el kernel contesto 0 a INFO_TSC_HZ. No es el bucle
//! ```
//!
//! # Lo que NO hace
//!
//! ```text
//!    [ ] no mide FPS: mide VUELTAS. Una vuelta sin nada sucio no pinta
//!    [ ] no arregla nada
//!    [ ] y no se puede cerrar, igual que la ficha de CABINA
//! ```

mod amarilla;
mod verde;

use bmo_userland as bmo;

pub(crate) use amarilla::Lectura;

/// **Refrescar: leer y pintar, en ese orden y sin mezclarlos.**
///
/// Toda la costura del modulo cabe en una linea, y eso no es casualidad: es lo
/// que demuestra que el corte estaba donde tenia que estar. Si aqui hiciera
/// falta logica, el corte seria otro.
pub(crate) fn refrescar(p: &bmo::Pantalla, l: &Lectura) {
    verde::pintar(p, &amarilla::leer(l));
}

/// La lectura de este cuarto, sacada del reloj del escritorio. Ver
/// `amarilla::de`.
pub(crate) fn de(t: &crate::desktop::Tick) -> Lectura {
    amarilla::de(t)
}
