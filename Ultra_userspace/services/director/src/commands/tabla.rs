//! **La TIPOGRAFIA de un informe** -- filas, barras, unidades y sangrados.
//!
//! [carril]  VERDE     no decide nada y no lee la maquina: recibe un numero y
//!           lo coloca. Su modo de fallo es una columna torcida
//! [consumo] NADA      no corre en reposo: lo llama un informe, y un informe
//!                     lo pide el dueno escribiendo `cpu`, `mem` o `consumo`
//!                     (L6h)
//!
//! [cuesta]  NADA -- una columna mal alineada no rompe una app, ni un dato, ni
//!           la maquina. Se lee raro y se arregla leyendolo.
//!
//! [riesgo]  ESPEJO -- todo lo que hace es copiar un numero a una rejilla de
//!           ancho fijo. Si el ancho de `Output` cambia, esto sigue contando
//!           con el de ayer y las columnas se van -- y no hay error: se ve.
//!
//! # Por que vive aparte de `reports.rs` (2026-09-12)
//!
//! `reports.rs` tenia 968 lineas de CODIGO contra el limite de mil de L6a, o
//! sea **32 de margen**, y dentro llevaba DOS clases de coste:
//!
//! ```text
//!    la tipografia   recibe un numero y lo coloca          -> NADA
//!    los informes    PREGUNTAN A LA MAQUINA, 91 puertas    -> DATO
//! ```
//!
//! ** Dos clases en un fichero es exactamente lo que L6e llama mal cortado, y
//! el corte va por donde cambia el coste. Que ademas devuelva margen es la
//! consecuencia, no el motivo -- igual que en `codegen/bex.rs` el 12-09.
//!
//! [!] **Esto NO es un fichero de texto.** No envuelve el texto de una frase
//! por gusto ni elige colores: `fila` alinea a ancho fijo **para que dos
//! volcados se puedan poner uno debajo del otro y compararse**, que es el uso
//! real (lanzar DOOM, matarlo, y mirar si la RAM volvio). Cambiar un ancho aqui
//! rompe esa comparacion en silencio.

use crate::scene::output::{Output, INK_GOOD, INK_ECHO, INK_ERR, INK_PLAIN};
use crate::scene::OUT_COLS;

/// Un rotulo de seccion, para que el informe no sea un muro de renglones.
pub(crate) fn section(s: &mut Output, title: &[u8]) {
    s.with_ink(INK_ECHO);
    s.text(b"  ");
    s.text(title);
    s.byte(b' ');
    // Una regla hasta el margen: cuesta nada y separa de verdad.
    let used_one = 3 + title.len();
    for _ in used_one..OUT_COLS.saturating_sub(2) {
        s.byte(b'-');
    }
    s.byte(b'\n');
    s.with_ink(INK_PLAIN);
}

/// Un renglon `label ....... value`, con la etiqueta a ancho fijo.
pub(crate) fn label(s: &mut Output, name: &[u8]) {
    s.text(b"    ");
    s.text(name);
    for _ in name.len()..14 {
        s.byte(b' ');
    }
}

/// Igual, pero a **10** y para los informes de una palabra.
///
/// ** Los catorce de arriba son de los informes en prosa, donde la etiqueta es
/// una frase corta (`marcos libres`, `a Ring 3`). Cuando las etiquetas son de
/// UNA palabra --`medium`, `link`, `queue`-- catorce dejan cuatro espacios en
/// blanco en cada renglon y la tabla se lee como una lista suelta. Una tabla
/// junta se lee de un vistazo; esa es toda la diferencia.
pub(crate) fn campo(s: &mut Output, name: &[u8]) {
    s.text(b"    ");
    s.text(name);
    for _ in name.len()..10 {
        s.byte(b' ');
    }
}

/// **La nota de una fila, partida por PALABRAS y con sangria.**
///
/// ** Existe por el volcado del Ryzen del 2026-09-13, que salio asi:
///
/// ```text
///    en pie                  1 hilos   [#---] 8%
///    en puertas           8712 t/vueltaCOMO MUCHO: trafico x el techo de puerta del perfi
/// l de esta maquina
/// ```
///
/// Dos fallos del mismo sitio: una unidad de ocho letras se pegaba a la nota
/// (el relleno era `8 - largo`, o sea cero), y la rejilla parte en la columna 88
/// sin mirar palabras -- `perfi` / `l`, y la continuacion en la columna 0,
/// debajo de los NOMBRES, donde se lee como una fila nueva.
///
/// La pieza: SIEMPRE un espacio antes, y si la palabra no cabe, sigue en la
/// linea de abajo a la altura de la nota. Una palabra mas larga que la columna
/// entera se deja partir: no hay nada mejor que hacer con ella.
fn nota_partida(s: &mut Output, nota: &[u8]) {
    use crate::scene::OUT_COLS;
    s.byte(b' ');
    let sangria = s.col.min(OUT_COLS / 2);
    s.with_ink(INK_ECHO);
    for (i, palabra) in nota.split(|&c| c == b' ').enumerate() {
        if i > 0 {
            let cabe = s.col + 1 + palabra.len() <= OUT_COLS;
            if !cabe && palabra.len() <= OUT_COLS - sangria {
                s.byte(b'\n');
                for _ in 0..sangria {
                    s.byte(b' ');
                }
            } else {
                s.byte(b' ');
            }
        }
        s.text(palabra);
    }
    s.with_ink(INK_PLAIN);
}

/// Una fila de la tabla de consumo: `que`, el valor a la DERECHA, y la unidad.
///
/// Las tres columnas van a ancho fijo porque una tabla en la que los numeros no
/// estan alineados no es una tabla: es una lista con guiones. El valor va a la
/// derecha (`dec_right`) para que las unidades y las decenas caigan una debajo
/// de otra y se puedan comparar dos volcados de un vistazo.
pub(crate) fn fila(s: &mut Output, que: &[u8], valor: u64, unidad: &[u8], nota: &[u8]) {
    s.text(b"    ");
    s.text(que);
    for _ in que.len()..16 {
        s.byte(b' ');
    }
    s.dec_right(valor, 9);
    s.byte(b' ');
    s.text(unidad);
    if !nota.is_empty() {
        for _ in unidad.len()..8 {
            s.byte(b' ');
        }
        nota_partida(s, nota);
    }
    s.byte(b'\n');
}

/// **Una fila de `X de Y`**, que es otra cosa que una fila con unidad.
///
/// ** Existe por un renglon roto que trajo el Ryzen el 2026-08-17:
///
/// ```text
///    en pie                  1 de              <- de QUE?
///    marcos libres     3878260 de
/// ```
///
/// Las dos llamaban a [`fila`] poniendo `"de"` en la columna de la UNIDAD, y el
/// segundo numero no existia en ninguna parte. No es un fallo de formato: es
/// una frase a medias, y una frase a medias en un informe **se lee como un dato
/// que falta**. El total va por su propio parametro para que no se pueda
/// escribir la primera mitad sin la segunda.
pub(crate) fn fila_de(s: &mut Output, que: &[u8], valor: u64, total: u64, nota: &[u8]) {
    s.text(b"    ");
    s.text(que);
    for _ in que.len()..16 {
        s.byte(b' ');
    }
    s.dec_right(valor, 9);
    s.text(b" de ");
    s.dec(total);
    if !nota.is_empty() {
        s.text(b"  ");
        nota_partida(s, nota);
    }
    s.byte(b'\n');
}

/// Igual, pero para un numero con UN decimal guardado en milesimas: los vatios
/// llegan en milivatios y `57432` se lee como `57.4`.
pub(crate) fn fila_mili(s: &mut Output, que: &[u8], milis: u64, unidad: &[u8], nota: &[u8]) {
    s.text(b"    ");
    s.text(que);
    for _ in que.len()..16 {
        s.byte(b' ');
    }
    s.dec_right(milis / 1000, 7);
    s.byte(b'.');
    s.dec((milis % 1000) / 100);
    s.byte(b' ');
    s.text(unidad);
    if !nota.is_empty() {
        for _ in unidad.len()..8 {
            s.byte(b' ');
        }
        nota_partida(s, nota);
    }
    s.byte(b'\n');
}

/// **Una fila con BARRA**: el numero a la derecha y la proporcion dibujada.
///
/// == *** POR QUE UNA BARRA Y NO SOLO EL NUMERO (2026-09-10) =============
///
/// `15096 MiB libres de 15118` y `1096 MiB libres de 15118` se leen igual de
/// rapido --o sea, mal-- porque el ojo compara **longitudes**, no digitos. Una
/// barra convierte una resta mental en una ojeada.
///
/// ** Y va con caracteres, no con pixeles, porque la salida ES una rejilla de
/// caracteres: `Output::bar` ya existia y no lo usaba nadie. Lo que faltaba no
/// era la herramienta, era llamarla.
///
/// [!] Solo para lo que TIENE denominador. Una barra sobre un contador sin
/// techo --ticks, siestas-- seria dibujar una proporcion inventada.
pub(crate) fn fila_barra(s: &mut Output, que: &[u8], parte: u64, total: u64, unidad: &[u8]) {
    s.text(b"    ");
    s.text(que);
    for _ in que.len()..16 {
        s.byte(b' ');
    }
    s.dec_right(parte, 9);
    s.byte(b' ');
    s.text(unidad);
    for _ in unidad.len()..8 {
        s.byte(b' ');
    }
    // ** El color sale de la PROPORCION, no de una opinion: lleno es malo
    // para lo que se gasta, y por eso quien llama pasa `parte` como *lo
    // usado*. Un umbral fijo en 90 se elige porque por debajo no hay nada que
    // hacer y por encima ya no da tiempo a hacerlo.
    let lleno = if total == 0 { 0 } else { parte.saturating_mul(100) / total };
    s.with_ink(if lleno >= 90 {
        INK_ERR
    } else if lleno >= 70 {
        INK_ECHO
    } else {
        INK_GOOD
    });
    s.bar(parte, total, 20);
    s.byte(b' ');
    s.pct(parte, total);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}

/// **Una fila que TIENE QUE SER CERO**, y lo dice con el color.
///
/// *** Es el ayudante que mas trabaja de los tres, y por un motivo que no es
/// estetico: en el informe del DMA hay CUATRO filas cuyo unico valor bueno es
/// el cero --pisados, choques, caducados, rotos-- y **mezcladas con las demas
/// se leen como numeros cualesquiera**. En verde o en rojo se leen como lo que
/// son: una regla que se cumplio, o una que no.
///
///   > Un panel donde todo se ve igual obliga a leerlo entero. El color no es
///   > adorno: es lo que permite NO leer las filas que estan bien.
pub(crate) fn fila_cero(s: &mut Output, que: &[u8], valor: u64, nota: &[u8]) {
    s.text(b"    ");
    s.text(que);
    for _ in que.len()..16 {
        s.byte(b' ');
    }
    s.with_ink(if valor == 0 { INK_GOOD } else { INK_ERR });
    s.dec_right(valor, 9);
    s.with_ink(INK_PLAIN);
    s.text(b"          ");
    s.with_ink(if valor == 0 { INK_GOOD } else { INK_ERR });
    s.text(if valor == 0 { b"OK" } else { b"[!]" });
    s.with_ink(INK_ECHO);
    s.byte(b' ');
    s.text(nota);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}

/// Un renglon de separacion DENTRO de una seccion. Ver `report_consumo`.
pub(crate) fn subregla(s: &mut Output, titulo: &[u8]) {
    s.with_ink(INK_ECHO);
    s.text(b"    ");
    s.text(titulo);
    s.byte(b' ');
    let usado = 5 + titulo.len();
    for _ in usado..OUT_COLS.saturating_sub(4) {
        s.byte(b'.');
    }
    s.byte(b'\n');
    s.with_ink(INK_PLAIN);
}

/// Escribe `texto` cortando por ESPACIOS, con las continuaciones sangradas.
///
/// Cortar por palabras y no por caracteres es la diferencia entre una frase que
/// sigue debajo y una frase partida a mitad de palabra. `ancho` es lo que cabe
/// contando desde `sangria`.
pub(crate) fn envolver(s: &mut Output, texto: &[u8], sangria: usize, ancho: usize) {
    let mut i = 0usize;
    let mut primera = true;
    while i < texto.len() {
        if !primera {
            for _ in 0..sangria {
                s.byte(b' ');
            }
        }
        if texto.len() - i <= ancho {
            s.text(&texto[i..]);
            s.byte(b'\n');
            return;
        }
        // El ultimo espacio que cabe. Si no hay ninguno --una palabra mas larga
        // que el renglon-- se corta en seco: es lo unico que se puede hacer, y
        // es mejor que un bucle que no avanza.
        let mut corte = ancho;
        while corte > 0 && texto[i + corte] != b' ' {
            corte -= 1;
        }
        if corte == 0 {
            corte = ancho;
        }
        s.text(&texto[i..i + corte]);
        s.byte(b'\n');
        i += corte;
        while i < texto.len() && texto[i] == b' ' {
            i += 1;
        }
        primera = false;
    }
}
