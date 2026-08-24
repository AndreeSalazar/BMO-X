//! AVX2: cuatro `flotante64` de golpe.
//!
//! ** Y NO LAS EMITE NADIE SOLO. La seccion 13.7 del maestro decidio que INTI no
//! vectoriza, y eso no cambia: quien quiera cuatro a la vez **lo escribe**. Lo
//! que se prueba aqui es que se pueda escribir y que de el numero correcto.
//!
//! *** El bloque `crudo` que hace falta SE CUENTA, y sale en el manifiesto. Esa
//! es la diferencia con un compilador que vectoriza a tus espaldas: aqui el
//! sitio donde nadie comprueba tiene un numero.

use super::*;

/// Un banco de reales a mano: `a` en `base`, `b` en `base+32`, el resultado en
/// `base+64`. Todo alineado a 8, que es lo que `vmovupd` pide (no alineado).
fn con_reales(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa x86_64\nusa memoria\n\nfuncion prueba(base es natural64, n es natural64) devuelve natural64\n    crudo\n        a = base\n        b = base + 32\n        c = base + 64\n{}",
        cuerpo
    )
}

/// Escribe `v` como los bits de un `flotante64`.
fn pon(dir: &str, i: u64, v: f64) -> String {
    format!("        escribe_natural64({} + {}, {})\n", dir, i * 8, v.to_bits())
}

/// ***CUATRO SUMAS EN UNA INSTRUCCION, y las cuatro salen bien.***
#[test]
fn suma_de_cuatro_suma_los_cuatro() {
    let mut cuerpo = String::new();
    for i in 0..4 {
        cuerpo += &pon("a", i, (i + 1) as f64);
        cuerpo += &pon("b", i, 10.0);
    }
    cuerpo += "        suma_de_cuatro(c, a, b)\n";
    // El tercero: 3 + 10 = 13.
    cuerpo += "        devuelve lee_natural64(c + 16)\n";
    let r = ejecuta_en(&con_reales(&cuerpo), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 13.0, "el tercero de los cuatro");
}

/// Y el CUARTO tambien, que es el que se pierde si alguien emite `xmm` en vez de
/// `ymm`: una instruccion SSE haria dos y dejaria los otros dos intactos.
#[test]
fn el_cuarto_no_se_queda_atras() {
    let mut cuerpo = String::new();
    for i in 0..4 {
        cuerpo += &pon("a", i, (i + 1) as f64);
        cuerpo += &pon("b", i, 100.0);
        cuerpo += &pon("c", i, 0.0);
    }
    cuerpo += "        suma_de_cuatro(c, a, b)\n";
    cuerpo += "        devuelve lee_natural64(c + 24)\n";
    let r = ejecuta_en(&con_reales(&cuerpo), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 104.0, "4 + 100, y es el cuarto");
}

/// ***`funde_de_cuatro` ACUMULA: lee el destino antes de escribirlo.***
///
/// ** Es la operacion de la que esta hecho un producto de matrices, y la unica
/// de las cuatro que justifica las otras tres. El `231` de `vfmadd231pd` dice
/// exactamente eso: el acumulador es el destino.
#[test]
fn funde_de_cuatro_multiplica_y_acumula() {
    let mut cuerpo = String::new();
    for i in 0..4 {
        cuerpo += &pon("a", i, 2.0);
        cuerpo += &pon("b", i, 3.0);
        cuerpo += &pon("c", i, 100.0);
    }
    // c += a * b  ->  100 + 6 = 106
    cuerpo += "        funde_de_cuatro(c, a, b)\n";
    cuerpo += "        devuelve lee_natural64(c)\n";
    let r = ejecuta_en(&con_reales(&cuerpo), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 106.0, "acumula, no pisa");

    // Y dos vueltas acumulan dos veces: 100 + 6 + 6 = 112.
    let mut dos = String::new();
    for i in 0..4 {
        dos += &pon("a", i, 2.0);
        dos += &pon("b", i, 3.0);
        dos += &pon("c", i, 100.0);
    }
    dos += "        funde_de_cuatro(c, a, b)\n";
    dos += "        funde_de_cuatro(c, a, b)\n";
    dos += "        devuelve lee_natural64(c)\n";
    let r = ejecuta_en(&con_reales(&dos), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 112.0);
}

/// Restar y multiplicar tambien, que si no la fila sobra.
#[test]
fn resta_y_producto_de_cuatro() {
    let mut cuerpo = String::new();
    for i in 0..4 {
        cuerpo += &pon("a", i, 10.0);
        cuerpo += &pon("b", i, 4.0);
    }
    cuerpo += "        resta_de_cuatro(c, a, b)\n";
    cuerpo += "        devuelve lee_natural64(c + 8)\n";
    let r = ejecuta_en(&con_reales(&cuerpo), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 6.0);

    let mut m = String::new();
    for i in 0..4 {
        m += &pon("a", i, 10.0);
        m += &pon("b", i, 4.0);
    }
    m += "        por_de_cuatro(c, a, b)\n";
    m += "        devuelve lee_natural64(c + 8)\n";
    let r = ejecuta_en(&con_reales(&m), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 40.0);
}

/// ***Y PIDEN `crudo`, que es lo que las hace CONTABLES.***
///
/// ** Escriben 32 bytes en una direccion que da el programa, y nadie comprueba
/// que quepan. Es memoria cruda, igual que `escribe_natural64` -- por eso estan
/// en la misma lista y por eso su uso sale en el manifiesto con un numero.
///
/// *** Esa es la diferencia entera con un compilador que vectoriza a tus
/// espaldas: aqui **el sitio donde nadie comprueba se puede contar**.
#[test]
fn las_de_avx_piden_crudo_y_por_eso_se_cuentan() {
    let fuera = bmo_inti_front::comprobar(
        "perfil llano\nusa x86_64\n\nfuncion f(a es natural64)\n    suma_de_cuatro(a, a, a)\n",
    );
    assert!(
        fuera.codigos().contains(&"E0072"),
        "sin `crudo` tiene que denunciarse: {:?}",
        fuera.codigos()
    );

    let dentro = bmo_inti_front::comprobar(
        "perfil llano\nusa x86_64\n\nfuncion f(a es natural64)\n    crudo\n        suma_de_cuatro(a, a, a)\n",
    );
    assert!(dentro.codigos().is_empty(), "{:?}", dentro.codigos());
    assert_eq!(dentro.valor.bloques_crudo, 1, "y el bloque se CUENTA");
}
