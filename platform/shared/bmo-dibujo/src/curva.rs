//! # Escalon 1.5 -- LA CURVA, que son lineas y por eso va aqui
//!
//! Una Bezier cubica no es una primitiva nueva: es **una polilinea calculada**.
//! Se parte en tramos rectos y se dibujan con `linea`. Por eso este modulo son
//! treinta lineas y no un algoritmo -- todo el trabajo lo hizo el escalon 1.
//!
//! ## Para que hace falta: el grafo de ESTRATOS
//!
//! El grafo unia sus cajas con **codos** --tramos horizontales y verticales--
//! y el comentario que lo justificaba decia *"es como pinta n8n"*.
//!
//! [!] Y eso no es cierto: **n8n une sus nodos con curvas Bezier**, con las
//! tangentes horizontales en los dos extremos. Es justo lo que hace que un
//! grafo se lea como un grafo y no como un cuadro de tuberias -- la curva dice
//! de donde SALE y a donde ENTRA cada arista sin necesidad de flechas.
//!
//! Un codo se dibuja perfectamente con rectangulos, asi que cambiarlo por
//! `linea` no habria ganado nada. Lo que la primitiva desbloquea es ESTO.
//!
//! ## Cuantos tramos, y por que no es un numero fijo
//!
//! Un numero fijo parte mal por los dos lados: con 32 tramos, una curva de
//! quince pixeles paga treinta y dos llamadas para pintar quince puntos; con 8,
//! una curva que cruza la pantalla se ve **poligonal**.
//!
//! Se estima desde el tamano de la caja que contiene los cuatro puntos de
//! control: `(ancho + alto) / 8`, acotado a `[4, 64]`. No es exacto --la caja
//! es mayor que la curva-- y no hace falta que lo sea: pasarse cuesta un tramo
//! de mas, quedarse corto se ve.
//!
//! ## Todo entero, otra vez
//!
//! `B(t)` con `t = i/n` se evalua multiplicando por `n^3` y dividiendo al
//! final: cero coma flotante, por lo mismo que en `linea.rs`. En `i64` porque
//! `n^3 * coordenada` con `n = 64` son 262.144 por pixel, y en una pantalla
//! ancha eso ya no cabe comodo en 32 bits.
//!
//! [!] Se prueba con el arnes de `pruebas_sueltas.rs`.

use super::linea::linea;
use super::recorte::Recorte;
use super::triangulo::Vertice;

/// Cuantos tramos rectos merece esta curva.
fn tramos(a: Vertice, b: Vertice, c: Vertice, d: Vertice) -> i64 {
    let minx = a.0.min(b.0).min(c.0).min(d.0);
    let maxx = a.0.max(b.0).max(c.0).max(d.0);
    let miny = a.1.min(b.1).min(c.1).min(d.1);
    let maxy = a.1.max(b.1).max(c.1).max(d.1);
    let bulto = (maxx - minx) as i64 + (maxy - miny) as i64;
    (bulto / 8).clamp(4, 64)
}

/// Un punto de la Bezier cubica en `t = i/n`, en enteros.
///
/// `(n-i)^3 P0 + 3(n-i)^2 i P1 + 3(n-i) i^2 P2 + i^3 P3`, todo partido por
/// `n^3`. El `+ den/2` antes de dividir es redondeo al mas cercano: sin el,
/// truncar sesga la curva medio pixel hacia el origen en cada muestra, y eso
/// se nota como una curva que no llega a tocar su propio extremo.
fn en(a: Vertice, b: Vertice, c: Vertice, d: Vertice, i: i64, n: i64) -> (i32, i32) {
    let u = n - i;
    let (u3, u2i, ui2, i3) = (u * u * u, 3 * u * u * i, 3 * u * i * i, i * i * i);
    let den = n * n * n;
    let mitad = den / 2;
    let x = (u3 * a.0 as i64 + u2i * b.0 as i64 + ui2 * c.0 as i64 + i3 * d.0 as i64 + mitad)
        .div_euclid(den);
    let y = (u3 * a.1 as i64 + u2i * b.1 as i64 + ui2 * c.1 as i64 + i3 * d.1 as i64 + mitad)
        .div_euclid(den);
    (x as i32, y as i32)
}

/// Dibuja la Bezier cubica `a -> d` con tirantes `b` y `c`, recortada a `r`.
///
/// Los extremos se pintan exactamente: en `i = 0` e `i = n` la formula da `a` y
/// `d` sin redondeo ninguno --los otros tres terminos valen cero-- y eso es lo
/// que permite que una arista **toque** su caja en vez de quedarse cerca.
pub fn curva(
    r: &Recorte,
    a: Vertice,
    b: Vertice,
    c: Vertice,
    d: Vertice,
    mut pixel: impl FnMut(i32, i32),
) {
    let n = tramos(a, b, c, d);
    let mut anterior = a;
    for i in 1..=n {
        let punto = en(a, b, c, d, i, n);
        linea(r, anterior.0, anterior.1, punto.0, punto.1, &mut pixel);
        anterior = punto;
    }
}

/// La tangente de salida en `t = 0`, normalizada a un paso de un pixel.
///
/// Hace falta para colocar una punta de flecha mirando a donde mira la curva.
/// Es `3(b - a)`, y aqui solo importa su DIRECCION, asi que se reduce a
/// `(-1, 0, 1)` por eje: una punta de flecha de siete pixeles no distingue mas.
pub fn direccion(desde: Vertice, hacia: Vertice) -> (i32, i32) {
    let dx = hacia.0 - desde.0;
    let dy = hacia.1 - desde.1;
    (dx.signum(), dy.signum())
}

// -- Las pruebas ------------------------------------------------------------
#[cfg(test)]
mod pruebas {
    use super::*;

    struct Lienzo {
        pixeles: [u8; 128 * 128],
    }

    impl Lienzo {
        fn nuevo() -> Self {
            Lienzo { pixeles: [0; 128 * 128] }
        }
        fn marcar(&mut self, x: i32, y: i32) {
            assert!(x >= 0 && x < 128 && y >= 0 && y < 128, "fuera: {},{}", x, y);
            self.pixeles[(y * 128 + x) as usize] = 1;
        }
        fn en(&self, x: i32, y: i32) -> u8 {
            self.pixeles[(y * 128 + x) as usize]
        }
        fn cuantos(&self) -> usize {
            self.pixeles.iter().filter(|&&v| v > 0).count()
        }
    }

    /// ** LA PRUEBA QUE ATA LA CURVA A LA LINEA.
    ///
    /// Con los cuatro puntos de control alineados, una Bezier cubica ES el
    /// segmento. Asi que tiene que pintar **exactamente los mismos pixeles**
    /// que `linea`. Si no, el muestreo o el redondeo estan mal, y esta prueba
    /// lo dice sin que nadie tenga que mirar una curva a ojo.
    #[test]
    fn con_los_tirantes_en_linea_es_una_recta() {
        let r = Recorte::nuevo(0, 0, 128, 128);
        let (a, d) = ((10, 10), (100, 100));
        let b = (40, 40); // ambos sobre el segmento a->d
        let c = (70, 70);
        let mut porcurva = Lienzo::nuevo();
        let mut porlinea = Lienzo::nuevo();
        curva(&r, a, b, c, d, |x, y| porcurva.marcar(x, y));
        linea(&r, a.0, a.1, d.0, d.1, |x, y| porlinea.marcar(x, y));
        for y in 0..128 {
            for x in 0..128 {
                assert_eq!(porcurva.en(x, y), porlinea.en(x, y), "difieren en {},{}", x, y);
            }
        }
    }

    /// Los dos extremos se tocan exactamente. Es lo que hace que una arista
    /// llegue a su caja en vez de quedarse a un pixel.
    #[test]
    fn los_extremos_se_tocan_exactamente() {
        let r = Recorte::nuevo(0, 0, 128, 128);
        let mut l = Lienzo::nuevo();
        let (a, d) = ((5, 5), (120, 90));
        curva(&r, a, (80, 5), (40, 90), d, |x, y| l.marcar(x, y));
        assert_eq!(l.en(a.0, a.1), 1, "no sale de su origen");
        assert_eq!(l.en(d.0, d.1), 1, "no llega a su destino");
    }

    /// ** SIN HUECOS: cada pixel pintado toca al siguiente.
    ///
    /// Es la prueba que justifica partir en tramos y unirlos con `linea` en vez
    /// de muestrear puntos sueltos. Muestrear y pintar puntos deja una curva de
    /// PUNTITOS en cuanto se estira, y eso no se ve en una captura pequena.
    ///
    /// Se comprueba contando: cada pixel de la curva tiene que tener al menos
    /// un vecino en las ocho direcciones.
    #[test]
    fn la_curva_no_tiene_huecos() {
        let r = Recorte::nuevo(0, 0, 128, 128);
        let mut l = Lienzo::nuevo();
        curva(&r, (2, 60), (60, 2), (60, 120), (125, 60), |x, y| l.marcar(x, y));
        assert!(l.cuantos() > 100, "hay curva que comprobar");
        for y in 0..128 {
            for x in 0..128 {
                if l.en(x, y) == 0 {
                    continue;
                }
                let mut vecinos = 0;
                for dy in -1..=1i32 {
                    for dx in -1..=1i32 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let (vx, vy) = (x + dx, y + dy);
                        if vx >= 0 && vx < 128 && vy >= 0 && vy < 128 && l.en(vx, vy) == 1 {
                            vecinos += 1;
                        }
                    }
                }
                assert!(vecinos > 0, "el pixel {},{} esta suelto", x, y);
            }
        }
    }

    /// El recorte manda igual que en las demas: una curva que se sale no pinta
    /// fuera. El lienzo revienta si lo intenta.
    #[test]
    fn el_recorte_manda_sobre_la_curva() {
        let r = Recorte::nuevo(20, 20, 30, 30); // [20,50) x [20,50)
        let mut l = Lienzo::nuevo();
        // [!] Los puntos estan ELEGIDOS, no puestos a ojo: en `t = 1/2` la
        // formula da (41, 35), que cae dentro de la caja. La primera version de
        // esta prueba uso unos tirantes salvajes "para que cruzara seguro" y la
        // curva **no pasaba por la caja** -- donde su `x` entraba en rango, su
        // `y` iba por -6. El recorte contesto que no habia nada, que era lo
        // correcto, y la prueba lo acuso. Una prueba mal planteada no destapa un
        // bug: inventa uno.
        curva(&r, (-50, 35), (0, 20), (60, 50), (200, 35), |x, y| {
            assert!(r.contiene(x, y), "pinto fuera del recorte: {},{}", x, y);
            l.marcar(x, y);
        });
        assert!(l.cuantos() > 0, "la curva cruza la caja por su punto medio");
    }

    /// Una curva grande merece mas tramos que una pequena. Sin esto, o se
    /// pagan llamadas de mas o se ve poligonal.
    #[test]
    fn el_numero_de_tramos_sigue_al_tamano() {
        assert_eq!(tramos((0, 0), (1, 0), (2, 0), (3, 0)), 4, "el minimo");
        assert_eq!(tramos((0, 0), (0, 0), (0, 0), (2000, 2000)), 64, "el tope");
        let medio = tramos((0, 0), (40, 0), (40, 80), (80, 80));
        assert!(medio > 4 && medio < 64, "una curva normal cae en medio: {}", medio);
    }

    #[test]
    fn la_direccion_se_reduce_a_los_ocho_sentidos() {
        assert_eq!(direccion((0, 0), (50, 0)), (1, 0));
        assert_eq!(direccion((50, 50), (0, 90)), (-1, 1));
        assert_eq!(direccion((7, 7), (7, 7)), (0, 0));
    }
}
