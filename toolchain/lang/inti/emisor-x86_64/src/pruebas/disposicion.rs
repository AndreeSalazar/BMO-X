//! LA DISPOSICION -- `p.x` y `a[i]` dejan de mentir.
//!
//! Donde esta cada campo y cuanto mide cada elemento. La pregunta es una:
//! **un acceso lee lo que dice leer?**

use super::*;
// ===================================================================
//  ** F5b -- CAMPOS Y BUFERES. Los dos agujeros que quedaban.
// ===================================================================
//
//  Hasta hoy `p.x` se bajaba a `p` --el campo se IGNORABA, sin una queja-- y
//  `a[i]` bajaba a la DIRECCION del elemento en vez de a su valor. Las dos
//  compilaban, corrian, y hacian otra cosa.
//
//  Las dos eran el mismo agujero: **INTI no sabia cuanto mide nada**. Un campo
//  es una direccion mas un desplazamiento, y el desplazamiento sale de las
//  medidas de los campos de antes.
//
//  ** Y las dos se arreglan con la MISMA cuenta, que es la senal de que el
//  arreglo es el correcto: `p.x`, `p.x = 3`, `a[i]` y `a[i] = 3` calculan
//  exactamente lo mismo y solo cambian la instruccion del final.

const CON_PUNTO: &str = "\
perfil llano
usa monton
usa memoria

registro Punto
    x es entero64
    y es entero64

";

/// ** Un campo se escribe y se lee, y cae donde dice el plano.
#[test]
fn un_campo_de_registro_se_escribe_y_se_lee() {
    let f = format!(
        "{}{}{}{}{}",
        CON_PUNTO,
        "funcion principal devuelve entero32\n",
        "    m = monton_nuevo(4096)\n",
        "    p es Punto = pide(m, 16)\n",
        "    p.x = 11\n    p.y = 31\n    devuelve p.x + p.y\n"
    );
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 42);
}

/// ** Y los dos campos NO son el mismo sitio.
///
/// Es la prueba que echa abajo el comportamiento viejo: cuando `p.x` se bajaba
/// a `p`, los dos campos eran la misma direccion, `p.x = 11` seguido de
/// `p.y = 31` dejaba 31 en las dos, y la suma daba 62. Compilaba igual.
#[test]
fn dos_campos_no_son_el_mismo_sitio() {
    let f = format!(
        "{}{}{}{}{}",
        CON_PUNTO,
        "funcion principal devuelve entero32\n",
        "    m = monton_nuevo(4096)\n",
        "    p es Punto = pide(m, 16)\n",
        "    p.x = 5\n    p.y = 9\n    devuelve p.x\n"
    );
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 5, "no 9");
}

/// El desplazamiento del segundo campo es 8, y se ve desde fuera del registro.
///
/// Se escribe por el campo y se lee a mano por la direccion cruda. Si el plano
/// mintiera, estos dos numeros no coincidirian.
#[test]
fn el_campo_esta_donde_el_plano_dice() {
    let f = format!(
        "{}{}{}{}{}{}",
        CON_PUNTO,
        "funcion principal devuelve entero32\n",
        "    m = monton_nuevo(4096)\n",
        "    p es Punto = pide(m, 16)\n",
        "    p.y = 77\n",
        "    crudo\n        devuelve lee_natural64(p + 8)\n"
    );
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 77);
}

/// ** Dos registros seguidos no se pisan: el segundo empieza en la medida del
/// primero, y por eso la medida se redondea a la alineacion.
#[test]
fn dos_registros_seguidos_no_se_pisan() {
    let f = format!(
        "{}{}{}{}{}{}",
        CON_PUNTO,
        "funcion principal devuelve entero32\n",
        "    m = monton_nuevo(4096)\n",
        "    a es Punto = pide(m, 16)\n",
        "    b es Punto = pide(m, 16)\n",
        "    a.x = 100\n    b.x = 1\n    devuelve a.x + b.x\n"
    );
    // Si se solaparan, `b.x = 1` habria pisado `a.x` y esto daria 2.
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 101);
}

// ===================================================================
//  ** `bufer de T` -- indexar de verdad
// ===================================================================

/// El indice multiplica por la MEDIDA DEL ELEMENTO, no por uno.
///
/// Con el comportamiento viejo, `a[2]` daba `a + 2`. Ahora da `a + 8` para un
/// bufer de `entero64`, y lo que devuelve es el VALOR.
#[test]
fn un_bufer_se_indexa_por_la_medida_de_su_elemento() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    a es bufer de entero64 = pide(m, 64)
    crudo
        a[0] = 10
        a[1] = 20
        a[2] = 30
        devuelve a[2] - a[0]
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 20);
}

/// Y los elementos no se pisan, que es lo que pasaria con la medida mal.
#[test]
fn los_elementos_de_un_bufer_no_se_pisan() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    a es bufer de entero64 = pide(m, 64)
    crudo
        a[0] = 1
        a[1] = 2
        a[2] = 4
        devuelve a[0] + a[1] + a[2]
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 7);
}

/// ** EL FRAMEBUFFER, otra vez -- pero escrito como se escribe de verdad.
///
/// Comparalo con `inti_rellena_una_pantalla_de_pixeles`, que es el mismo
/// programa de hace un rato:
///
/// ```text
///    antes   escribe_natural32(pantalla + i * 4, color)
///    ahora   pantalla[i] = color
/// ```
///
/// El `* 4` desaparece del fuente porque **lo sabe el tipo**. Y no es solo mas
/// corto: el `4` escrito a mano es un numero que hay que cambiar en todos los
/// sitios el dia que los pixeles sean de 16 bits, y el que no se cambie
/// compilara igual.
#[test]
fn un_framebuffer_escrito_con_un_bufer() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion pinta(pantalla es bufer de natural32, cuantos es entero64, color es entero64)
    crudo
        cambiante i = 0
        repite mientras i < cuantos
            pantalla[i] = color
            i = i + 1

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    p es bufer de natural32 = pide(m, 64)
    pinta(p, 16, 65280)
    crudo
        devuelve p[10]
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 65280);
}

/// Y con pixeles de 32 bits, el ultimo de 16 esta en el byte 60 -- lo que
/// confirma que el paso fue de cuatro y no de ocho.
#[test]
fn el_paso_del_bufer_es_el_del_elemento_y_no_una_palabra() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    p es bufer de natural32 = pide(m, 64)
    crudo
        p[15] = 123
        devuelve lee_natural32(p + 60)
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 123);
}
