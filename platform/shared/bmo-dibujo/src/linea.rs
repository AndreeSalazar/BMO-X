//! # Escalon 1 -- LA LINEA, que es la primera diagonal de BMO-X
//!
//! Hasta hoy no habia ni una. Todas las "lineas" del escritorio son
//! `rect(x, y, ancho, 1, color)` -- rectangulos de un pixel de alto-- y el
//! grafo de ESTRATOS une sus cajas con eso. Funciona mientras todo sea
//! horizontal o vertical, y se acaba en cuanto dos cosas no estan alineadas.
//!
//! ## Bresenham, y por que sigue siendo el correcto en 2026
//!
//! La tentacion es `y = y0 + (x - x0) * pendiente` con coma flotante. Aqui eso
//! es peor por dos motivos, y ninguno es la velocidad:
//!
//! 1. **No hay que tocar la unidad SSE.** El compositor corre en Ring 3 con el
//!    estado de coma flotante que le toque; el kernel de BMO-X ya se peleo con
//!    `xrstor` una semana (ver el episodio del `#GP`). Una primitiva de dibujo
//!    que no usa `xmm` no puede participar en esa clase de fallo.
//! 2. **El redondeo se acumula.** Sumar la pendiente por cada columna arrastra
//!    error; una diagonal larga acaba un pixel desviada por el extremo. El
//!    error de Bresenham es una cuenta entera exacta que **nunca deriva**.
//!
//! ## El orden: primero se recorta, DESPUES se anda
//!
//! Y no al reves. Andar la linea entera descartando lo que cae fuera es
//! correcto y es una trampa: una coordenada calculada que salga grande da
//! millones de vueltas descartando. Ver la nota de `recorte.rs`.
//!
//! ** Como se recorta contra pixeles y los dos extremos quedan DENTRO, todo lo
//! que Bresenham pisa entre ellos esta dentro tambien: su camino no se sale
//! nunca de la caja que forman los dos extremos. O sea que **este modulo no
//! comprueba limites por pixel**, y no es un descuido: es la consecuencia de
//! haber recortado antes. Hay una prueba que lo vigila.
//!
//! ## Por que emite por callback en vez de recibir la pantalla
//!
//! Porque asi la geometria no conoce el destino. El mismo codigo pinta en el
//! framebuffer, en el buffer de una ventana, o **en un array de pruebas del
//! anfitrion** -- que es lo que permite que esto se ejecute sin encender el
//! Ryzen. Una primitiva atada a `Pantalla` solo se puede probar mirandola.
//!
//! [!] Se prueba junto a `recorte.rs` con el arnes de `pruebas_sueltas.rs`.
//! Ver la cabecera de ese fichero.

use super::recorte::{recortar_segmento, Recorte};

/// Dibuja el segmento `(xa,ya)-(xb,yb)`, recortado a `r`, llamando a `pixel`
/// una vez por cada punto.
///
/// Los dos extremos se pintan: el segmento es **cerrado**, al reves que el
/// rectangulo de `Recorte`. No es una incoherencia -- un area tiene borde y un
/// segmento tiene extremos, y quien dibuja de A a B espera ver la B.
pub fn linea(r: &Recorte, xa: i32, ya: i32, xb: i32, yb: i32, mut pixel: impl FnMut(i32, i32)) {
    let (mut x, mut y, xf, yf) = match recortar_segmento(r, xa, ya, xb, yb) {
        Some(t) => t,
        None => return,
    };

    // El Bresenham general, en la forma que trata los ocho octantes con el
    // mismo codigo: `dy` va NEGADO, y el error se compara contra los dos
    // limites. Sin eso hacen falta cuatro variantes y tres de ellas no se
    // prueban nunca.
    let dx = (xf - x).abs();
    let paso_x: i32 = if x < xf { 1 } else { -1 };
    let dy = -(yf - y).abs();
    let paso_y: i32 = if y < yf { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        pixel(x, y);
        if x == xf && y == yf {
            return;
        }
        let e2 = 2 * error;
        if e2 >= dy {
            error += dy;
            x += paso_x;
        }
        if e2 <= dx {
            error += dx;
            y += paso_y;
        }
    }
}

// -- Las pruebas ------------------------------------------------------------
#[cfg(test)]
mod pruebas {
    use super::*;

    /// Junta los pixeles en un lienzo de pruebas: un array plano y una cuenta.
    /// Es el "destino" mas tonto posible, y por eso vale como testigo.
    struct Lienzo {
        ancho: i32,
        alto: i32,
        pixeles: [u8; 64 * 64],
        n: usize,
    }

    impl Lienzo {
        fn nuevo(ancho: i32, alto: i32) -> Self {
            Lienzo { ancho, alto, pixeles: [0; 64 * 64], n: 0 }
        }
        fn marcar(&mut self, x: i32, y: i32) {
            assert!(
                x >= 0 && x < self.ancho && y >= 0 && y < self.alto,
                "se pinto FUERA del lienzo: {},{}", x, y
            );
            self.pixeles[(y * self.ancho + x) as usize] += 1;
            self.n += 1;
        }
        fn en(&self, x: i32, y: i32) -> u8 {
            self.pixeles[(y * self.ancho + x) as usize]
        }
    }

    #[test]
    fn una_horizontal_pinta_los_dos_extremos() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut l = Lienzo::nuevo(64, 64);
        linea(&r, 2, 5, 8, 5, |x, y| l.marcar(x, y));
        assert_eq!(l.n, 7, "de 2 a 8 inclusive son SIETE pixeles");
        assert_eq!(l.en(2, 5), 1);
        assert_eq!(l.en(8, 5), 1, "el extremo final se pinta");
        assert_eq!(l.en(9, 5), 0);
    }

    #[test]
    fn una_diagonal_perfecta_pinta_la_diagonal() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut l = Lienzo::nuevo(64, 64);
        linea(&r, 0, 0, 9, 9, |x, y| l.marcar(x, y));
        assert_eq!(l.n, 10);
        for i in 0..10 {
            assert_eq!(l.en(i, i), 1, "falta el pixel {},{}", i, i);
        }
    }

    /// ** LOS OCHO OCTANTES, que es donde se esconden los bugs de Bresenham.
    ///
    /// Una implementacion con cuatro variantes acierta en el primer cuadrante
    /// --que es el que se prueba a ojo-- y se equivoca en los otros. Aqui se
    /// dibuja la misma linea en las ocho direcciones desde el centro y se
    /// exige que las ocho tengan la MISMA longitud.
    #[test]
    fn las_ocho_direcciones_dan_la_misma_linea() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let (cx, cy) = (32, 32);
        let destinos = [
            (12, 0), (12, 6), (0, 12), (-6, 12),
            (-12, 0), (-12, -6), (0, -12), (6, -12),
        ];
        let mut primera = 0usize;
        for (i, (dx, dy)) in destinos.iter().enumerate() {
            let mut l = Lienzo::nuevo(64, 64);
            linea(&r, cx, cy, cx + dx, cy + dy, |x, y| l.marcar(x, y));
            assert_eq!(l.en(cx, cy), 1, "direccion {}: falta el origen", i);
            assert_eq!(l.en(cx + dx, cy + dy), 1, "direccion {}: falta el destino", i);
            if i == 0 {
                primera = l.n;
            }
            assert_eq!(l.n, primera, "la direccion {} tiene otra longitud", i);
        }
    }

    /// Una linea es la MISMA se dibuje de A a B o de B a A. Un Bresenham mal
    /// escrito da dos escaleras distintas y eso se ve al repintar.
    #[test]
    fn dibujarla_al_reves_da_los_mismos_pixeles() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut ida = Lienzo::nuevo(64, 64);
        let mut vuelta = Lienzo::nuevo(64, 64);
        linea(&r, 3, 7, 40, 25, |x, y| ida.marcar(x, y));
        linea(&r, 40, 25, 3, 7, |x, y| vuelta.marcar(x, y));
        assert_eq!(ida.n, vuelta.n);
        for y in 0..64 {
            for x in 0..64 {
                assert_eq!(ida.en(x, y), vuelta.en(x, y), "difieren en {},{}", x, y);
            }
        }
    }

    /// ** LA PRUEBA QUE SOSTIENE LA DECISION DE NO COMPROBAR LIMITES POR PIXEL.
    ///
    /// El lienzo hace `assert` si le piden pintar fuera. Si el recorte previo
    /// no bastara, esta prueba **revienta** en vez de dibujar mal en silencio.
    #[test]
    fn nada_se_pinta_fuera_aunque_la_linea_venga_de_muy_lejos() {
        let r = Recorte::nuevo(10, 10, 20, 20); // [10,30) x [10,30)
        let mut l = Lienzo::nuevo(64, 64);
        // ** La diagonal `y = x`, que SI cruza la caja -- de (10,10) a (29,29).
        //
        // [!] La primera version de esta prueba usaba (-5000,-3000)-(5000,4000)
        // "porque viene de lejos", y esa linea **no pasa por la caja**: a la
        // altura de `x=10` ya va por `y=507`. El recorte contesto `None`, que
        // era lo correcto, y la prueba fallo acusandolo. Una prueba mal
        // planteada no destapa un bug: inventa uno.
        linea(&r, -5000, -5000, 5000, 5000, |x, y| {
            assert!(r.contiene(x, y), "pinto fuera del RECORTE: {},{}", x, y);
            l.marcar(x, y);
        });
        assert!(l.n > 0, "la diagonal cruza la caja: algo tiene que pintarse");
        assert_eq!(l.en(10, 10), 1, "entra por la esquina de arriba");
        assert_eq!(l.en(29, 29), 1, "y sale por la de abajo");
    }

    #[test]
    fn una_linea_entera_fuera_no_pinta_nada() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut n = 0;
        linea(&r, -50, -50, -10, -20, |_, _| n += 1);
        assert_eq!(n, 0);
    }

    #[test]
    fn un_punto_solo_es_una_linea_de_un_pixel() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut l = Lienzo::nuevo(64, 64);
        linea(&r, 5, 5, 5, 5, |x, y| l.marcar(x, y));
        assert_eq!(l.n, 1);
        assert_eq!(l.en(5, 5), 1);
    }
}
