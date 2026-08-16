//! **EL AURA** -- el cielo encendido detras del logo.
//!
//! === Por que existe, y por que vive con la ciudad y no con el gato ===
//!
//! El video del 2026-08-15 lo enseno sin lugar a dudas: el gato es un trazo
//! blanco de un pixel sobre un cielo violeta claro, y **se pierde dentro de la
//! escena**. Cuando el degradado del cielo llega a su parte clara, el gato casi
//! desaparece. El dueno lo dijo asi: *"la capa estan mezcladas"*.
//!
//! La escalera de valores de [`crate::paleta`] separa el cielo de las torres, y
//! eso funciono. Lo que no cubria es el tercer plano: **el logo no tiene ninguna
//! separacion del cielo**. Esta simplemente estampado encima.
//!
//! Un neon de verdad no se estampa: **enciende el aire que tiene alrededor**. Un
//! tubo de gas en una calle de noche se ve porque hay un halo de su color en la
//! pared de detras, y ese halo es lo que dice "esto esta DELANTE y esta
//! ENCENDIDO". Sin el, un trazo brillante es una pegatina.
//!
//! Asi que el aura **es cielo**, no es gato: es el cielo respondiendo a una luz.
//! Por eso vive en este crate, junto al que sabe de que color esta el cielo en
//! cada fila, y por eso [`aura`] pide una funcion `cielo` en vez de un color
//! fijo. Puesta del lado del gato habria que copiar ahi el degradado, y una copia
//! del degradado se separa del original el dia que alguien toque uno.
//!
//! === Y por que no se mezcla con lo que hay debajo ===
//!
//! Lo natural seria pintar el aura con alfa sobre la pantalla. **No se puede**:
//! mezclar con lo de abajo obliga a LEER el framebuffer, que es memoria
//! write-combining y va lentisimo -- la misma trampa que ya costo cara en el blit
//! de DOOM y que ya esta anotada en `draw_gato_encendido`.
//!
//! Aqui no hace falta: el aura se dibuja sobre CIELO, y el color del cielo se
//! sabe sin leer nada. Se emiten rectangulos opacos con el color ya mezclado.
//!
//! [!] De ahi una regla que hay que respetar arriba: **el aura tiene que caber en
//! el cielo despejado**. Si se solapara con las torres las borraria, porque es
//! opaca. Quien la usa la coloca por encima de `Ciudad::techo`.

use crate::paleta::{mezcla, Color};
use bmo_dibujo::Lienzo;

/// En cuantas franjas horizontales se parte el aura.
///
/// Dieciseis, las mismas que el cielo. No es casualidad: si el aura tuviera un
/// escalonado mas fino que el degradado sobre el que vive, se leeria como un
/// objeto de otra resolucion pegado encima -- que es justo el efecto que se
/// intenta quitar.
const FRANJAS: i32 = 16;

/// Cuantos anillos concentricos. Cada uno un punto mas encendido que el de
/// fuera.
///
/// [!] **Estaba en seis y el previsualizador lo tumbo.** Con seis se veian los
/// escalones como aros de diana en vez de como un degradado: la primera imagen
/// que saco `bmo-vista-ciudad` mostraba un disco con bordes, no un resplandor.
/// Es exactamente el fallo que antes habria hecho falta grabar el disco y
/// reiniciar el Ryzen para descubrir.
const ANILLOS: u32 = 16;

/// Hasta donde se encoge el anillo mas interior, en centesimas del radio.
///
/// ** Y ESTE NUMERO ES EL QUE CONVIERTE UN PUNTO EN UN RESPLANDOR.
///
/// Los anillos iban de radio completo hasta CERO, asi que el mas interior era un
/// ovalo diminuto con la intensidad al maximo: un **punto brillante** clavado en
/// el pecho del gato. Se veia como una pelota, no como luz.
///
/// Parando en el 40% queda un nucleo plano de ese tamano --que es lo que hace
/// una luz de verdad: una zona saturada y una caida alrededor-- y los otros
/// quince anillos son la caida.
const NUCLEO: u32 = 40;

/// Raiz cuadrada entera. Sin coma flotante, que aqui no la hay.
///
/// Newton con arranque por bits: converge en pocas vueltas y no necesita tabla.
fn raiz(n: i32) -> i32 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// **El aura: una elipse de cielo encendido, hecha de franjas.**
///
/// * `cielo(y)` -- de que color esta el cielo en esa fila. Es lo que hace que el
///   aura se funda con el fondo en vez de ser un ovalo pegado.
/// * `(cx, cy)` -- el centro, en pixeles.
/// * `(rx, ry)` -- los radios. Se quiere ancha y no muy alta: el logo lo es.
/// * `tinte` -- el color de la luz. El mismo neon que el trazo.
/// * `fuerza` -- 0..255, cuanto tine el centro. **No llega nunca a 255**: un
///   aura opaca deja de ser luz y pasa a ser un fondo, y entonces el logo vuelve
///   a estar pegado sobre una mancha en vez de encendido sobre un cielo.
///
/// Se emiten los anillos de FUERA hacia DENTRO, asi que los de dentro tapan a
/// los de fuera y no hace falta recortar nada.
#[allow(clippy::too_many_arguments)]
pub fn aura(
    cielo: impl Fn(i32) -> Color,
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    tinte: Color,
    fuerza: u32,
    lienzo: &mut impl Lienzo,
) {
    aura_emitir(cielo, cx, cy, rx, ry, tinte, fuerza, |x, y, w, h, c| {
        lienzo.rect(x, y, w, h, c)
    });
}

/// La geometria del aura, sin recortar y sin destino.
///
/// Existe aparte de [`aura`] por las pruebas: lo que hay que poder comprobar es
/// que **los anillos no se salen de sus radios** --el aura es opaca, y si se
/// saliera borraria las torres que tiene al lado-- y eso solo se ve en la
/// geometria cruda. Recortada contra un lienzo, un anillo desbordado se veria
/// igual que uno correcto.
#[allow(clippy::too_many_arguments)]
pub(crate) fn aura_emitir(
    cielo: impl Fn(i32) -> Color,
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    tinte: Color,
    fuerza: u32,
    mut rect: impl FnMut(i32, i32, i32, i32, Color),
) {
    if rx <= 0 || ry <= 0 {
        return;
    }
    let ultimo = (ANILLOS - 1).max(1);
    for anillo in 0..ANILLOS {
        // Del anillo 0 (el mas grande y mas debil) al ultimo (el nucleo). El
        // radio NO baja a cero: se para en `NUCLEO`, o el ultimo anillo seria un
        // punto brillante en vez de una zona de luz. Ver la constante.
        let frac = 100 - (100 - NUCLEO) * anillo / ultimo;
        let arx = rx * frac as i32 / 100;
        let ary = ry * frac as i32 / 100;
        if arx <= 0 || ary <= 0 {
            continue;
        }
        // La intensidad sube al acercarse al centro. Cuadratica y no lineal:
        // la luz cae con la distancia mucho mas rapido que en linea recta, y una
        // caida lineal se ve como un disco con borde en vez de como un
        // resplandor.
        let cerca = anillo + 1;
        let f = fuerza * cerca * cerca / (ANILLOS * ANILLOS);

        let alto_franja = (ary * 2 / FRANJAS).max(1);
        let mut y = cy - ary;
        while y < cy + ary {
            // Medio ancho de la elipse a esta altura: rx * sqrt(1 - dy^2/ry^2),
            // hecho con enteros para no necesitar coma flotante.
            let dy = (y + alto_franja / 2) - cy;
            let dentro = ary * ary - dy * dy;
            if dentro > 0 {
                let semi = arx * raiz(dentro) / ary;
                if semi > 0 {
                    let alto = alto_franja.min(cy + ary - y);
                    // El color de ESTA fila, tenido. Cada franja mira su propio
                    // cielo: sin eso el aura seria de un tono plano y se veria
                    // el borde contra el degradado.
                    let c = mezcla(cielo(y), tinte, f, 255);
                    rect(cx - semi, y, semi * 2, alto, c);
                }
            }
            y += alto_franja;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::paleta::{luminancia, CIELO_BAJO, NEON_CIAN};

    /// Un cielo plano, para poder juzgar el aura sin el degradado de por medio.
    fn cielo_plano(_y: i32) -> Color {
        CIELO_BAJO
    }

    /// El crate es `no_std` salvo en pruebas, asi que aqui si hay `Vec`. Nada de
    /// esto entra en el kernel: `aura` emite por callback y no reserva un byte.
    fn recoger(rx: i32, ry: i32, fuerza: u32) -> Vec<(i32, i32, i32, i32, Color)> {
        let mut v = Vec::new();
        aura_emitir(cielo_plano, 100, 100, rx, ry, NEON_CIAN, fuerza, |x, y, w, h, c| {
            v.push((x, y, w, h, c))
        });
        v
    }

    /// ** LO QUE EL AURA TIENE QUE HACER: que el centro NO sea del color del
    /// cielo. Si lo fuera, el gato seguiria estampado sobre el fondo -- que es
    /// exactamente el fallo que se ve en el video del 08-15.
    #[test]
    fn el_centro_esta_mas_encendido_que_el_cielo() {
        let v = recoger(60, 40, 150);
        assert!(!v.is_empty(), "el aura no dibujo nada");
        // El ultimo anillo es el mas interior y el mas encendido.
        let ultimo = v.last().unwrap().4;
        assert!(
            luminancia(ultimo) > luminancia(CIELO_BAJO) + 20,
            "el centro del aura no se distingue del cielo"
        );
    }

    /// ** Y LA OTRA MITAD: el aura NO puede llegar a opaca. Un aura del color
    /// puro del neon deja de ser luz y pasa a ser un fondo solido, y entonces el
    /// logo esta pegado sobre una mancha en vez de encendido sobre un cielo.
    #[test]
    fn el_aura_nunca_llega_al_color_puro_del_neon() {
        let v = recoger(60, 40, 200);
        for (_, _, _, _, c) in &v {
            assert_ne!(*c, NEON_CIAN, "el aura llego a opaca y tapo el cielo");
        }
    }

    /// El resplandor CAE hacia fuera. Si no cayera, seria un disco con borde
    /// duro, que es lo contrario de un halo.
    #[test]
    fn el_resplandor_cae_del_centro_al_borde() {
        let v = recoger(60, 40, 200);
        let primero = v.first().unwrap().4;
        let ultimo = v.last().unwrap().4;
        assert!(
            luminancia(ultimo) > luminancia(primero),
            "el anillo de fuera esta tan encendido como el de dentro"
        );
    }

    /// ** EL AURA CABE EN SU CAJA. Es opaca: si se saliera, borraria las torres
    /// que tiene al lado -- y el arreglo de la separacion habria creado un
    /// agujero en la ciudad.
    #[test]
    fn el_aura_no_se_sale_de_sus_radios() {
        let (cx, cy, rx, ry) = (100, 100, 60, 40);
        aura_emitir(cielo_plano, cx, cy, rx, ry, NEON_CIAN, 150, |x, y, w, h, _| {
            assert!(x >= cx - rx, "se salio por la izquierda: {}", x);
            assert!(x + w <= cx + rx, "se salio por la derecha: {}", x + w);
            assert!(y >= cy - ry, "se salio por arriba: {}", y);
            assert!(y + h <= cy + ry, "se salio por abajo: {}", y + h);
        });
    }

    /// Con fuerza 0 el aura es cielo: se puede apagar del todo sin quitarla del
    /// codigo. Es lo que permite que entre con el trazo en vez de estar siempre.
    #[test]
    fn con_fuerza_cero_el_aura_es_el_cielo() {
        let v = recoger(60, 40, 0);
        for (_, _, _, _, c) in &v {
            assert_eq!(*c, CIELO_BAJO);
        }
    }

    /// Radios degenerados no rompen ni dibujan basura. Pasa en pantallas
    /// pequenas, donde el hueco despejado puede quedarse en nada.
    #[test]
    fn unos_radios_de_cero_no_dibujan_nada() {
        let mut n = 0;
        aura_emitir(cielo_plano, 10, 10, 0, 10, NEON_CIAN, 200, |_, _, _, _, _| n += 1);
        aura_emitir(cielo_plano, 10, 10, 10, 0, NEON_CIAN, 200, |_, _, _, _, _| n += 1);
        assert_eq!(n, 0);
    }

    /// La raiz entera es la de verdad, que es lo que sostiene la forma de la
    /// elipse. Una raiz mala se ve como un rombo.
    #[test]
    fn la_raiz_entera_es_correcta() {
        for n in 0..400i32 {
            let r = raiz(n);
            assert!(r * r <= n, "raiz({}) = {} se paso", n, r);
            assert!((r + 1) * (r + 1) > n, "raiz({}) = {} se quedo corta", n, r);
        }
    }
}
