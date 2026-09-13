//! `y` Y `o`: el valor de dos condiciones, CORRIENDO.
//!
//! ** Este fichero existe por un fallo que llevaba ahi desde que el emisor
//! nacio: `Op::Y` y `Op::O` caian en el `_ => {}` de `operaciones::binaria` y no
//! se emitia nada. `a y b` devolvia `a`. `verdadero y falso` daba verdadero.
//!
//! Lo cazo `sondas/pulso.inti` (2026-09-12), el primer programa que escribio
//! `si v < p y cifra > 0`: el cero de la ultima cifra salia en blanco.
//!
//! *** Y la leccion es la de siempre, la tercera vez en este emisor: **lo que
//! no se emite no se prueba**. El parser tenia su prueba de precedencia y el
//! analisis de tipos la suya; ninguna EJECUTABA un `y`.
//!
//! Las tablas van por PARAMETROS y no con constantes, para que ningun paso del
//! compilador pueda contestar la pregunta antes de que llegue a bytes.

use super::*;

const Y: &str = "perfil llano\n\nfuncion prueba(a es natural64, b es natural64) devuelve natural64\n    si a > 0 y b > 0\n        devuelve 1\n    devuelve 0\n";
const O: &str = "perfil llano\n\nfuncion prueba(a es natural64, b es natural64) devuelve natural64\n    si a > 0 o b > 0\n        devuelve 1\n    devuelve 0\n";

/// Las cuatro filas de `y`. La que fallaba es la segunda: con el izquierdo
/// cierto, el derecho no se miraba.
#[test]
fn la_tabla_de_y() {
    assert_eq!(ejecuta_en(Y, "prueba", 0, 0), 0, "falso y falso");
    assert_eq!(ejecuta_en(Y, "prueba", 1, 0), 0, "verdadero y falso -- LA QUE FALLABA");
    assert_eq!(ejecuta_en(Y, "prueba", 0, 1), 0, "falso y verdadero");
    assert_eq!(ejecuta_en(Y, "prueba", 1, 1), 1, "verdadero y verdadero");
}

/// Las cuatro de `o`. Fallaba al reves: con el izquierdo falso, daba falso
/// aunque el derecho fuera cierto.
#[test]
fn la_tabla_de_o() {
    assert_eq!(ejecuta_en(O, "prueba", 0, 0), 0, "falso o falso");
    assert_eq!(ejecuta_en(O, "prueba", 1, 0), 1, "verdadero o falso");
    assert_eq!(ejecuta_en(O, "prueba", 0, 1), 1, "falso o verdadero -- LA QUE FALLABA");
    assert_eq!(ejecuta_en(O, "prueba", 1, 1), 1, "verdadero o verdadero");
}

/// ** El caso de verdad, tal como lo escribio la sonda: dos comparaciones de
/// clase distinta (`<` y `>`) y el resultado dentro de un `si`.
#[test]
fn el_caso_de_la_sonda_del_pulso() {
    let f = "perfil llano\n\nfuncion prueba(v es natural64, cifra es natural64) devuelve natural64\n    cambiante p es natural64 = 1\n    si v < p y cifra > 0\n        devuelve 1\n    devuelve 0\n";
    assert_eq!(ejecuta_en(f, "prueba", 0, 0), 0, "la ultima cifra de un cero NO es un espacio");
    assert_eq!(ejecuta_en(f, "prueba", 0, 5), 1, "un cero de delante SI lo es");
}

/// ** Y lo que NO hace, fijado para que nadie lo crea: sin CORTOCIRCUITO.
///
/// Los dos lados se evaluan siempre, de izquierda a derecha (Regla 6). Aqui se
/// ve porque el derecho DIVIDE ENTRE CERO: con cortocircuito, `falso y ...` no
/// llegaria a dividir y daria 0; sin el, atrapa con `E1003`.
///
/// El dia que la IR baje `y`/`o` con saltos, esta prueba cambia -- y tiene que
/// cambiar a proposito, no por accidente.
#[test]
fn sin_cortocircuito_el_lado_derecho_se_evalua() {
    let f = "perfil llano\n\nfuncion prueba(a es entero64, c es entero64) devuelve natural64\n    si a > 0 y 10 entre c > 0\n        devuelve 1\n    devuelve 0\n";
    assert_eq!(ejecuta_en(f, "prueba", 0, 0), 1003, "el derecho se evaluo y atrapo");
}
