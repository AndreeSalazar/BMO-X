//! `tabla de A a B`: la cuarta pieza de `pleno`, y la primera con un ALGORITMO.
//!
//! ** `texto`, `lista` y `numero` eran disposiciones: donde cae cada byte. Una
//! tabla ademas decide **como se busca**, y eso no se lee de una cabecera.
//!
//! Lo que se prueba aqui es que sea una TABLA y no una lista de parejas con otro
//! nombre -- que es lo unico que justifica que el tipo exista.

use super::*;

/// Un monton a mano, una tabla de ocho ranuras, y dos textos de clave.
fn con_tabla(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa objetos\nusa monton\n\nfuncion prueba(base es natural64, n es natural64) devuelve natural64\n    crudo\n        escribe_natural64(base, base + 32)\n        escribe_natural64(base + 8, base + 4096)\n        escribe_natural64(base + 16, 0)\n        t = tabla_nueva(base, 8)\n        k1 = pide(base, 26)\n        escribe_natural64(k1, 1)\n        escribe_natural64(k1 + 16, 2)\n        escribe_natural8(k1 + 24, 104)\n        escribe_natural8(k1 + 25, 111)\n        k2 = pide(base, 26)\n        escribe_natural64(k2, 1)\n        escribe_natural64(k2 + 16, 2)\n        escribe_natural8(k2 + 24, 108)\n        escribe_natural8(k2 + 25, 97)\n{}",
        cuerpo
    )
}

/// ***PONER Y BUSCAR: la tabla guarda y encuentra.***
#[test]
fn poner_una_pareja_y_encontrarla() {
    let f = con_tabla(
        "        si pon(t, k1, 77) no es 1\n            devuelve 0\n        d = busca(t, k1, 0)\n        si d = 0\n            devuelve 0\n        devuelve lee_natural64(d)\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 77);
}

/// Y una clave que no esta contesta que no, sin dar vueltas.
///
/// *** ESTO ES LO QUE EXIGE EL MARGEN. En direccionamiento abierto, buscar una
/// clave ausente en una tabla LLENA no termina: la sonda da vueltas para
/// siempre. `pon` deja siempre una ranura libre, y esa ranura es la condicion de
/// parada -- por eso el ABI lo comprueba y no lo recomienda.
#[test]
fn una_clave_que_no_esta_contesta_que_no() {
    let f = con_tabla("        pon(t, k1, 77)\n        devuelve busca(t, k2, 0)\n");
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 0);
}

/// ***DOS CLAVES DISTINTAS NO SE PISAN, aunque caigan cerca.***
#[test]
fn dos_claves_distintas_guardan_dos_valores() {
    let f = con_tabla(
        "        pon(t, k1, 11)\n        pon(t, k2, 22)\n        si parejas(t) no es 2\n            devuelve 0\n        a = lee_natural64(busca(t, k1, 0))\n        b = lee_natural64(busca(t, k2, 0))\n        si a no es 11\n            devuelve 0\n        si b no es 22\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 1);
}

/// ***LA CLAVE SE COMPARA POR SUS BYTES, no por su direccion.***
///
/// ** Es lo que hace que esto sea una tabla y no un mapa de punteros. Dos textos
/// **distintos objetos** con los mismos bytes son la MISMA clave -- si no, un
/// literal y un texto construido con lo mismo serian dos entradas, y nadie
/// entenderia por que.
#[test]
fn dos_textos_iguales_en_objetos_distintos_son_la_misma_clave() {
    // `k3` tiene los mismos bytes que `k1` y es otro objeto.
    let f = con_tabla(
        "        k3 = pide(base, 26)\n        escribe_natural64(k3, 1)\n        escribe_natural64(k3 + 16, 2)\n        escribe_natural8(k3 + 24, 104)\n        escribe_natural8(k3 + 25, 111)\n        pon(t, k1, 55)\n        si parejas(t) no es 1\n            devuelve 0\n        d = busca(t, k3, 0)\n        si d = 0\n            devuelve 0\n        devuelve lee_natural64(d)\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 55, "mismos bytes = misma clave");
}

/// Y volver a poner la misma clave PISA el valor y no anade una pareja.
#[test]
fn poner_dos_veces_la_misma_clave_no_anade_una_pareja() {
    let f = con_tabla(
        "        pon(t, k1, 11)\n        pon(t, k1, 99)\n        si parejas(t) no es 1\n            devuelve 0\n        devuelve lee_natural64(busca(t, k1, 0))\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 0), 99);
}

/// ***Y NO SE LLENA DEL TODO: la ultima ranura NO se ocupa.***
///
/// Ocho ranuras, y a la septima pareja `pon` dice que no. **Esa ranura que
/// sobra es lo que hace que buscar termine**, y por eso no es un desperdicio:
/// es la condicion de parada del bucle, guardada en memoria.
#[test]
fn la_tabla_deja_siempre_una_ranura_libre() {
    // Se meten claves de un byte, generadas en un bucle.
    let f = format!(
        "perfil llano\nusa objetos\nusa monton\n\nfuncion prueba(base es natural64, n es natural64) devuelve natural64\n    crudo\n        escribe_natural64(base, base + 32)\n        escribe_natural64(base + 8, base + 8192)\n        escribe_natural64(base + 16, 0)\n        t = tabla_nueva(base, 8)\n        cambiante i es natural64 = 0\n        cambiante metidas es natural64 = 0\n        repite mientras i < 20\n            k = pide(base, 25)\n            escribe_natural64(k, 1)\n            escribe_natural64(k + 16, 1)\n            escribe_natural8(k + 24, 65 + i)\n            metidas = metidas + pon(t, k, i)\n            i = i + 1\n        devuelve metidas\n"
    );
    assert_eq!(
        ejecuta_en(&f, "prueba", 0x40000, 0),
        7,
        "ocho ranuras, siete parejas: la octava se queda libre a proposito"
    );
}

/// ***Y LO QUE CONSTRUYE INTI LO ACEPTA EL ABI DE RUST.***
///
/// ** Otro codigo, a proposito: `dynobj::tabla::revisar` mira lo que INTI no
/// mira -- y sobre todo mira **que quede margen**, que es la unica de sus
/// comprobaciones que no habla de bytes sino de si un bucle termina.
#[test]
fn la_tabla_que_construye_inti_la_acepta_el_abi() {
    use bmo_abi::dynobj::tabla as abi;
    let f = con_tabla("        pon(t, k1, 11)\n        pon(t, k2, 22)\n        devuelve t\n");
    let e = emitido(&f);
    let inicio = e
        .inicios
        .iter()
        .find(|(n, _)| n == "prueba")
        .map(|(_, o)| *o)
        .expect("sin `prueba`");
    let largo = e.codigo.len() as i32;
    let mut codigo = Vec::new();
    codigo.push(0xE9);
    codigo.extend_from_slice(&largo.to_le_bytes());
    codigo.extend_from_slice(&e.codigo);
    codigo.push(0xE8);
    let desde = codigo.len() as i32 + 4;
    codigo.extend_from_slice(&((inicio as i32 + 5) - desde).to_le_bytes());

    let mut m = Machine::new(codigo);
    m.regs[7] = 0x40000;
    m.regs[6] = 0;
    let m = run(m, 500_000);
    let donde = m.regs[0];
    assert_ne!(donde, 0, "`tabla_nueva` no devolvio nada");

    let mut bytes = Vec::new();
    for i in 0..abi::bytes_para(8).unwrap() {
        bytes.push(m.read_u8_pub(donde + i));
    }
    let t = abi::revisar(&bytes).expect("el ABI rechazo la tabla que construyo INTI");
    assert_eq!(t.cuantos, 2, "dos parejas");
    assert_eq!(t.capacidad, 8);
    assert_eq!(t.refs, 1, "nace con UN dueno");
}
