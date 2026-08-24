//! El SIGNO: cuatro familias de instrucciones que dependen de el.
//!
//! ** Salio de `pruebas.rs` el 2026-08-23 por L6a. Es un tema propio y no un
//! apendice de la aritmetica: el signo no cambia lo que una operacion CALCULA,
//! cambia que instruccion se emite -- y las cuatro fallaban en silencio.

use super::*;
// ===================================================================
//  *** EL SIGNO (2026-08-23) -- cuatro familias, y ninguna fallaba
// ===================================================================
//
//  Se destapo escribiendo A MANO una guardia de desbordamiento dentro de un
//  bloque `crudo`. La guardia estaba bien y no saltaba, porque el emisor bajaba
//  TODA comparacion con `setl` -- la version con signo.
//
//  ** No era comportamiento indefinido. Era peor de encontrar: **una respuesta
//  equivocada, en silencio, sin que ninguna de las doce reglas saltara.** Las
//  reglas vigilan lo que C deja SIN DEFINIR; esto estaba definido, y mal.

/// ***`2 < 18446744073709551615` EN `natural64` ES CIERTO.***
///
/// Con `setl` daba 0: leidos con signo, 2^64-1 es -1. Y este no es un caso de
/// laboratorio -- son **direcciones**: `si nuevo > fin` dentro del propio
/// monton compara punteros, y el dia que uno pase del bit 63 la comparacion
/// contesta al reves.
#[test]
fn los_naturales_se_comparan_sin_signo() {
    let f = "perfil llano\n\nfuncion prueba(a es natural64, b es natural64) devuelve natural64\n    si a < b\n        devuelve 1\n    devuelve 0\n";
    assert_eq!(ejecuta_en(f, "prueba", 2, u64::MAX), 1, "2 < 2^64-1");
    assert_eq!(ejecuta_en(f, "prueba", u64::MAX, 2), 0, "y al reves, no");
}

/// Y los enteros SIGUEN con signo, que es la otra mitad y la que se podia
/// romper al arreglar la primera.
#[test]
fn los_enteros_se_siguen_comparando_con_signo() {
    let f = "perfil llano\n\nfuncion prueba(a es entero64, b es entero64) devuelve natural64\n    si a < b\n        devuelve 1\n    devuelve 0\n";
    assert_eq!(ejecuta_en(f, "prueba", (-1i64) as u64, 2), 1, "-1 < 2");
    assert_eq!(ejecuta_en(f, "prueba", 2, (-1i64) as u64), 0);
}

/// ***DIVIDIR: `div` para los naturales, `idiv` para los enteros.***
///
/// Con `idiv`, dividir 2^63 entre 2 no da 2^62: da una **excepcion del
/// procesador**, porque el cociente no cabe en un `entero64` con signo.
#[test]
fn dividir_un_natural_grande_no_revienta() {
    let f = "perfil llano\n\nfuncion prueba(a es natural64, b es natural64) devuelve natural64\n    devuelve a entre b\n";
    assert_eq!(ejecuta_en(f, "prueba", 1u64 << 63, 2), 1u64 << 62);
    assert_eq!(ejecuta_en(f, "prueba", u64::MAX, 2), u64::MAX / 2);
}

/// Y la division con signo sigue dando negativo.
#[test]
fn dividir_enteros_sigue_llevando_el_signo() {
    let f = "perfil llano\n\nfuncion prueba(a es entero64, b es entero64) devuelve entero64\n    devuelve a entre b\n";
    assert_eq!(ejecuta_en(f, "prueba", (-8i64) as u64, 2), (-4i64) as u64);
}

/// ***DESPLAZAR A LA DERECHA: el fallo AL REVES, y del mismo dia.***
///
/// Aqui el emisor emitia `shr` SIEMPRE --metiendo ceros por arriba-- que es lo
/// correcto para un natural y falso para un entero negativo:
///
/// ```text
///    -8 desplaza derecha 1     con `sar`  ->  -4
///                              con `shr`  ->  9.223.372.036.854.775.804
/// ```
///
/// ** El propio `x86::shr_r64_cl` predijo el dia: *"el dia que INTI distinga el
/// desplazamiento con signo sera otra fila de la tabla y otra instruccion"*.
#[test]
fn desplazar_un_entero_negativo_arrastra_el_signo() {
    let f = "perfil llano\n\nfuncion prueba(a es entero64, b es entero64) devuelve entero64\n    devuelve a desplaza derecha b\n";
    assert_eq!(ejecuta_en(f, "prueba", (-8i64) as u64, 1), (-4i64) as u64);

    // Y un natural sigue metiendo ceros, que es lo suyo.
    let g = "perfil llano\n\nfuncion prueba(a es natural64, b es natural64) devuelve natural64\n    devuelve a desplaza derecha b\n";
    assert_eq!(ejecuta_en(g, "prueba", u64::MAX, 1), u64::MAX >> 1);
}

/// [!] Y LA ARITMETICA DE DIRECCIONES ES SIN SIGNO, aunque nadie escriba un
/// tipo: `p.x` y `a[i]` suman bytes a una direccion, y una direccion no puede
/// ser negativa.
///
/// ** Sin esto, un registro colocado por encima del bit 63 se indexaria al
/// reves. Hoy no pasa porque el monton vive bajo, y "hoy no pasa" es
/// exactamente como se escriben los fallos que aparecen dentro de dos anos.
#[test]
fn la_aritmetica_de_direcciones_no_lleva_signo() {
    // Se mira en la IR y no en los bytes a proposito: un `add` es el mismo byte
    // lleve signo o no. Lo que cambia es lo que se emite DESPUES --la
    // comparacion, el desplazamiento, la guardia-- y eso sale de esta marca.
    let fuente = concat!(
        "perfil llano\n\n",
        "registro Punto\n    x es entero64\n    y es entero64\n\n",
        "funcion prueba(p es Punto) devuelve entero64\n    devuelve p.y\n"
    );
    let arbol = bmo_inti_front::armar(fuente);
    assert!(!arbol.hay_errores(), "{}", arbol.pintar("p.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let m = ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec()).valor;

    let sumas: Vec<bool> = m
        .funciones
        .iter()
        .flat_map(|f| f.instrucciones.iter())
        .filter_map(|i| match i {
            Instr::Binaria { sin_signo, .. } => Some(*sin_signo),
            _ => None,
        })
        .collect();
    assert!(!sumas.is_empty(), "sin aritmetica de campos que mirar");
    assert!(
        sumas.iter().all(|s| *s),
        "la aritmetica de direcciones perdio la marca de sin signo: {sumas:?}"
    );
}

/// La tabla de necesidades de las pruebas: **la incrustada**.
///
/// ** Y no la del disco a proposito. Una prueba que leyera `$BMO_MODS` diria
/// cosas distintas segun quien la corra, que es justo lo que un test no puede
/// hacer. La que se comprueba contra el disco es otra, y esta declarada aparte.
fn nec() -> bmo_inti_front::necesidades::Necesidades {
    bmo_inti_front::necesidades::Necesidades::por_defecto()
}
