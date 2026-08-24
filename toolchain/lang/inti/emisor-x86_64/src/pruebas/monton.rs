//! El monton y el contador de referencias, EJECUTADOS.
//!
//! ** Salieron de `pruebas.rs` el 2026-08-23 por L6a, y el corte es por TEMA:
//! aqui esta todo lo que mira la memoria de un programa de INTI -- repartirla,
//! recibirla, y contar quien tiene cada objeto.
//!
//! Los ayudantes (`ejecuta_en`, `emitido`) viven en el padre.

use super::*;
// ===================================================================
//  *** EL MONTON RECIBE DE VERDAD (2026-08-23)
// ===================================================================
//
//  `MONTON.md` llevaba desde que existe diciendo la verdad incomoda en su
//  seccion 3: *"`suelta` existe, se puede llamar, y NO devuelve nada al
//  monton"*. Ya devuelve.
//
//  ** Y estas pruebas EJECUTAN. Un asignador que compila no dice nada: lo unico
//  que decide si un trozo vuelve es pedirlo, soltarlo, y volver a pedir.

/// El montaje comun: un monton fabricado a mano sobre una direccion cualquiera.
///
/// *** No pasa por `monton_nuevo`, y es a proposito: `reparto.inti` **no habla
/// con el kernel** --lo dice su primera linea-- asi que probarlo a traves de la
/// puerta probaria las dos piezas juntas y no diria cual falla. Aqui se le da la
/// disposicion escrita a mano, que es todo lo que esa pieza sabe de un monton.
fn con_monton(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa monton\n\nfuncion prueba(base es natural64, cuantos es natural64) devuelve natural64\n    crudo\n        escribe_natural64(base, base + 32)\n        escribe_natural64(base + 8, base + cuantos)\n        escribe_natural64(base + 16, 0)\n{}",
        cuerpo
    )
}

/// ***UN TROZO SOLTADO VUELVE, Y EL SIGUIENTE `pide` LO REUTILIZA.***
///
/// Es la prueba entera en una linea: `c` tiene que ser **la misma direccion**
/// que `a`. Si el reparto siguiera siendo solo de avance, `c` estaria mas
/// adelante y el monton se habria comido cien bytes que ya no usaba nadie.
#[test]
fn un_trozo_soltado_se_reutiliza_en_el_siguiente_pide() {
    let f = con_monton(
        "        a = pide(base, 100)\n        b = pide(base, 100)\n        suelta(base, a)\n        c = pide(base, 100)\n        si c no es a\n            devuelve 0\n        si b = a\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 4096), 1);
}

/// **`suelta` devuelve CUANTOS bytes vuelven**, y el numero sale de la cabecera
/// del trozo, no de quien suelta.
///
/// 100 bytes pedidos se redondean a 112 --el monton reparte a 16-- y eso es lo
/// que vuelve. Que el numero no sea 100 es la prueba de que sale de la cabecera.
#[test]
fn suelta_dice_cuantos_bytes_devuelve_y_salen_de_la_cabecera() {
    let f = con_monton(
        "        a = pide(base, 100)\n        devuelve suelta(base, a)\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 4096), 112);
}

/// *** EL MONTON SE PUEDE AUDITAR: `queda_suelto` recorre la lista y suma.
///
/// ** Sin este numero, *"suelta de verdad"* seria una afirmacion sin forma de
/// comprobarla desde fuera. Es la costumbre de esta casa: el dato sale del
/// propio monton, no de quien lo usa.
#[test]
fn lo_suelto_se_puede_contar_y_vuelve_a_cero_al_reutilizarlo() {
    // Dos trozos sueltos: 112 + 112.
    let dos = con_monton(
        "        a = pide(base, 100)\n        b = pide(base, 100)\n        suelta(base, a)\n        suelta(base, b)\n        devuelve queda_suelto(base)\n",
    );
    assert_eq!(ejecuta_en(&dos, "prueba", 0x40000, 4096), 224);

    // Y al reutilizar uno, la cuenta baja: el hueco sale de la lista.
    let uno = con_monton(
        "        a = pide(base, 100)\n        b = pide(base, 100)\n        suelta(base, a)\n        suelta(base, b)\n        c = pide(base, 100)\n        devuelve queda_suelto(base)\n",
    );
    assert_eq!(ejecuta_en(&uno, "prueba", 0x40000, 4096), 112);
}

/// [!] Y EL CURSOR NO SE MUEVE al soltar: un trozo vuelve por la lista, no
/// desandando el camino.
///
/// ** Desandar solo se podria con el ULTIMO trozo, y una regla que funciona a
/// veces es peor que una que no funciona nunca -- porque la primera se aprende
/// mal y se usa donde no vale.
#[test]
fn soltar_no_baja_el_cursor() {
    let f = con_monton(
        "        a = pide(base, 100)\n        antes = queda_en(base)\n        suelta(base, a)\n        si queda_en(base) no es antes\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 4096), 1);
}

/// Un hueco demasiado pequeno NO se reutiliza: se sigue mirando, y si ninguno
/// cabe, avanza el cursor.
///
/// ** Sin esta, `pide` podria estar devolviendo el primer hueco de la lista sin
/// mirar su medida -- y la prueba de arriba seguiria en verde, porque alli todos
/// los trozos miden lo mismo.
#[test]
fn un_hueco_que_no_cabe_no_se_reutiliza() {
    let f = con_monton(
        "        pequeno = pide(base, 16)\n        suelta(base, pequeno)\n        grande = pide(base, 500)\n        si grande = pequeno\n            devuelve 0\n        devuelve 1\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 4096), 1);
}

/// Soltar la direccion cero no toca nada, y no es una comprobacion de adorno:
/// `pide` devuelve 0 cuando no cabe, asi que **el cero llega aqui por el camino
/// normal** el dia que un programa no mire lo que le dieron.
#[test]
fn soltar_un_cero_no_rompe_la_lista() {
    let f = con_monton(
        "        suelta(base, 0)\n        devuelve queda_suelto(base)\n",
    );
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 4096), 0);
}

// ===================================================================
//  *** EL CONTADOR DE REFERENCIAS (2026-08-23)
// ===================================================================
//
//  La pieza que le faltaba a `pleno` para tener objetos con vida propia. Vive
//  en `runtime/objetos/contador.inti` y **esta escrita en INTI `llano`**, que es
//  como este proyecto demuestra que `llano` sirve para escribir el sistema en
//  vez de repetirlo.

/// Un monton fabricado a mano, un objeto pedido, y su contador puesto a `refs`.
///
/// El cuerpo recibe `o` --la direccion del objeto, que es la que devolvio
/// `pide`-- y `refs` como segundo argumento de la funcion.
fn con_objeto(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa objetos\nusa monton\n\nfuncion prueba(base es natural64, refs es natural64) devuelve natural64\n    crudo\n        escribe_natural64(base, base + 32)\n        escribe_natural64(base + 8, base + 4096)\n        escribe_natural64(base + 16, 0)\n        o = pide(base, 64)\n        escribe_natural64(o, refs)\n{}",
        cuerpo
    )
}

const INMORTAL: u64 = 1 << 63;

/// ***`retiene` DICE LO MISMO QUE EL ABI, valor por valor.***
///
/// ** Esta es la prueba que importa de las cuatro. `bmo_abi::dynobj::header`
/// declara la semantica --`retain`, `release`, `is_last`-- y `contador.inti` la
/// vuelve a escribir en otro lenguaje. **Dos escrituras de la misma regla se
/// separan el dia que alguien toca una**, y este proyecto ya se comio ese fallo
/// con la tabla de intrinsecos esta misma manana.
///
/// Asi que no se comprueba "que suba": se comprueba que **coincida**.
#[test]
fn retiene_dice_lo_mismo_que_el_abi() {
    use bmo_abi::dynobj::header;
    let f = con_objeto("        devuelve retiene(o)\n");
    for refs in [0u64, 1, 2, 7, 1000, INMORTAL, INMORTAL | 5] {
        assert_eq!(
            ejecuta_en(&f, "prueba", 0x40000, refs),
            header::retain(refs),
            "INTI y el ABI no dicen lo mismo de retain({refs:#x})"
        );
    }
}

/// Y `libera` tambien: **el que muere es el que el ABI llama `is_last`.**
#[test]
fn libera_mata_exactamente_a_quien_el_abi_llama_el_ultimo() {
    use bmo_abi::dynobj::header;
    let f = con_objeto("        devuelve libera(base, o)\n");
    for refs in [0u64, 1, 2, 7, 1000, INMORTAL, INMORTAL | 5] {
        let murio = ejecuta_en(&f, "prueba", 0x40000, refs) == 1;
        assert_eq!(
            murio,
            header::is_last(refs),
            "INTI y el ABI no dicen lo mismo de is_last({refs:#x})"
        );
    }
}

/// ***CUANDO MUERE, EL TROZO VUELVE AL MONTON.*** Que es de lo que iba todo.
///
/// 64 bytes pedidos, 64 sueltos. Sin esto, "contador de referencias" seria un
/// numero que baja y una memoria que no vuelve -- que es exactamente lo que
/// `dynobj::lista` avisa de si mismo: *"el contador cuenta y no libera"*.
#[test]
fn al_morir_el_trozo_vuelve_al_monton() {
    let f = con_objeto("        libera(base, o)\n        devuelve queda_suelto(base)\n");
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 1), 64);

    // Y con dos duenos NO vuelve: solo baja el contador.
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, 2), 0);
}

/// *** A UN INMORTAL NO SE LE TOCA: ni el contador, ni el monton.
///
/// ** Y no basta con dejar el numero igual: **escribir los mismos bytes en una
/// pagina de solo lectura falla igual**. Un literal de texto vive en `RoData`
/// desde esta misma manana, asi que esto dejo de ser teorico hoy.
#[test]
fn un_inmortal_ni_se_cuenta_ni_se_suelta() {
    let f = con_objeto(
        "        libera(base, o)\n        si queda_suelto(base) no es 0\n            devuelve 100\n        devuelve referencias(o)\n",
    );
    // El contador vuelve intacto, con su bit 63 y su parte baja.
    assert_eq!(ejecuta_en(&f, "prueba", 0x40000, INMORTAL | 5), INMORTAL | 5);
}

/// ***EL DOBLE `libera` NO DA LA VUELTA, Y ESA ES LA REGLA CARA.***
///
/// Si un contador en cero diera la vuelta valdria `0xFFFF_FFFF_FFFF_FFFF`, que
/// **tiene el bit 63 puesto**: un doble-`libera` convertiria el objeto en
/// INMORTAL en silencio -- una fuga que no se denuncia a si misma jamas.
///
/// ** `header.rs` lo dice con esas palabras --*"it would be the `unwrap_or(0)`
/// failure in a new costume"*-- y se comprueba aqui porque **aqui es donde se
/// puede romper**: en el ABI es una funcion pura; en `contador.inti` es una
/// escritura a memoria.
#[test]
fn un_doble_libera_no_convierte_el_objeto_en_inmortal() {
    let f = con_objeto(
        "        libera(base, o)\n        libera(base, o)\n        devuelve referencias(o)\n",
    );
    let quedo = ejecuta_en(&f, "prueba", 0x40000, 1);
    assert_eq!(quedo, 0, "satura en cero");
    assert_eq!(quedo & INMORTAL, 0, "y sobre todo: NO se volvio inmortal");
}
