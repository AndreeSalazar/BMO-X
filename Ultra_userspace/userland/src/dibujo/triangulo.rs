//! # Escalon 2 -- EL TRIANGULO, que es la unidad de una GPU
//!
//! Este es el escalon que de verdad ensena a dibujar, y el que hace que los
//! siguientes tengan sentido. Un rectangulo es un caso particular; un triangulo
//! es **la primitiva** -- todo lo demas (poligonos, sprites deformados, el
//! mundo entero de un juego 3D) se descompone en triangulos, y eso no es una
//! convencion de esta casa: es lo que la GPU acepta.
//!
//! ## Se hace como lo hace el silicio, y a proposito
//!
//! Hay dos formas de rellenar un triangulo:
//!
//! - **Por aristas activas**: ordenar los vertices por `y`, recorrer las dos
//!   aristas a la vez interpolando. Es la clasica, es mas rapida, y **no se
//!   parece en nada a una GPU**.
//! - **Por funcion de arista**: para cada pixel de la caja que lo contiene,
//!   preguntar a las tres aristas si esta del lado de dentro. Es lo que hace el
//!   hardware, porque se puede contestar para cuatro pixeles a la vez sin que
//!   ninguno dependa del anterior.
//!
//! Aqui se usa la segunda, y la razon es el objetivo declarado: el dia que
//! exista el driver de RDNA4, **este modulo es el oraculo** con el que se juzga
//! lo que devuelve la tarjeta. Un oraculo escrito con otro algoritmo contesta
//! "difieren" y no sirve para decir quien tiene razon; escrito con el mismo,
//! cualquier diferencia es un bug de uno de los dos. Ver `PLAN_VULKAN.md`.
//!
//! La funcion de arista es un producto vectorial en 2D:
//!
//! ```text
//!     E(a, b, p) = (b.x - a.x)(p.y - a.y) - (b.y - a.y)(p.x - a.x)
//! ```
//!
//! Su signo dice de que lado del segmento `a->b` cae `p`, y **su valor es el
//! doble del area del triangulo `a,b,p`** -- que es justo lo que hara falta en
//! el escalon 3 para interpolar color por coordenadas baricentricas. O sea que
//! esto no es un rodeo: es el mismo numero que se necesita despues.
//!
//! ## ** LA REGLA TOP-LEFT, que es la razon de que esto no sea trivial
//!
//! Dos triangulos que comparten una arista --que es como esta hecho CUALQUIER
//! rectangulo, malla o sprite-- tienen pixeles cuyo centro cae exactamente
//! SOBRE la arista compartida. Sin una regla, pasa una de dos:
//!
//!   - los pinta **los dos**  -> con mezcla alfa, la costura sale mas oscura
//!   - no los pinta **ninguno** -> una raya de fondo entre las dos mitades
//!
//! La regla: un pixel justo sobre una arista pertenece al triangulo **solo si
//! esa arista es "superior" o "izquierda"**. Como cada arista compartida es
//! superior/izquierda para uno de los dos y no para el otro, cada pixel cae
//! exactamente en uno. Es la regla de D3D y de Vulkan, palabra por palabra, y
//! esta aqui desde el primer dia por lo mismo que arriba.
//!
//! ## Se muestrea en el CENTRO del pixel, sin coma flotante
//!
//! El centro del pixel `(x, y)` esta en `(x + 0.5, y + 0.5)`. Para no tocar
//! `xmm` --ver la cabecera de `linea.rs`-- se trabaja con **todo multiplicado
//! por dos**: los vertices en `2x`, el centro en `2x + 1`. Exacto, entero, y
//! sin un solo redondeo.
//!
//! [!] Y en `i64`. Con `i32`, un producto de dos coordenadas dobladas de una
//! pantalla 4K ya roza el limite, y un desbordamiento aqui **cambia el signo**:
//! el pixel se declara del otro lado de la arista y el triangulo sale del reves.
//!
//! [!] Se prueba junto a `recorte.rs` con el arnes de `pruebas_sueltas.rs`.

use super::recorte::Recorte;

/// Un vertice, en pixeles.
///
/// [!] Se llama `Vertice` y no `Vertice` porque `Vertice` **ya existe** en
/// `entrada.rs`: es donde esta el raton. Dos tipos con el mismo nombre en el
/// mismo `pub use *` no es un choque de nombres cualquiera -- es que quien lea
/// `Vertice` en una firma no sabra si le hablan de geometria o de un cursor. Y
/// ademas `Vertice` es la palabra que usa Vulkan.
pub type Vertice = (i32, i32);

/// La funcion de arista, evaluada en un punto ya DOBLADO (`2x+1`, `2y+1`).
fn arista(a: Vertice, b: Vertice, px2: i64, py2: i64) -> i64 {
    let ax = 2 * a.0 as i64;
    let ay = 2 * a.1 as i64;
    let bx = 2 * b.0 as i64;
    let by = 2 * b.1 as i64;
    (bx - ax) * (py2 - ay) - (by - ay) * (px2 - ax)
}

/// Es la arista `a->b` "superior" o "izquierda"?
///
/// Con el sentido de giro ya normalizado (area positiva) y la `y` creciendo
/// hacia ABAJO --que es como esta el framebuffer--:
///
///   - **superior**: horizontal, y avanza hacia la izquierda
///   - **izquierda**: baja
fn superior_o_izquierda(a: Vertice, b: Vertice) -> bool {
    (a.1 == b.1 && b.0 < a.0) || (b.1 > a.1)
}

/// Cae el punto dentro, contando la regla?
fn dentro(e: i64, top_left: bool) -> bool {
    e > 0 || (e == 0 && top_left)
}

/// Rellena el triangulo `a,b,c` recortado a `r`, entregando **tramos
/// horizontales** medio abiertos: `tramo(y, x0, x1)` pinta `[x0, x1)`.
///
/// ## Por que tramos y no pixeles sueltos
///
/// Porque el destino sabe rellenar una fila de golpe --`Pantalla::rect` de un
/// pixel de alto es un `memset` de la fila-- y entregar pixel a pixel obligaria
/// a pagar una llamada por punto. La geometria se decide por pixel (que es lo
/// que hace la GPU) y se **entrega** por tramo, que es lo que sabe consumir el
/// framebuffer. La cuenta es la misma; el trabajo del que recibe, no.
///
/// El triangulo es convexo, asi que cada fila da **como mucho un tramo**.
pub fn triangulo(
    r: &Recorte,
    a: Vertice,
    b: Vertice,
    c: Vertice,
    mut tramo: impl FnMut(i32, i32, i32),
) {
    let (a, mut b, mut c) = (a, b, c);

    // ** EL SENTIDO DE GIRO SE NORMALIZA, NO SE EXIGE.
    //
    // El doble del area con signo. Si es negativo, los vertices vienen en el
    // otro sentido y se intercambian dos: asi el resto del modulo puede dar por
    // hecho que "dentro" es `> 0`, y quien llama no tiene que saber nada de
    // sentidos de giro. Una GPU descarta por sentido (`face culling`); una
    // primitiva de escritorio que lo hiciera se comeria la mitad de los
    // triangulos sin decir por que.
    let area = (b.0 - a.0) as i64 * (c.1 - a.1) as i64 - (b.1 - a.1) as i64 * (c.0 - a.0) as i64;
    if area == 0 {
        // Degenerado: los tres en linea. No tiene interior, y dibujar la linea
        // "por si acaso" seria inventarse lo que no se pidio.
        return;
    }
    if area < 0 {
        core::mem::swap(&mut b, &mut c);
    }

    // La caja que lo contiene, ya cruzada con el recorte: fuera de aqui no se
    // pregunta ni una vez.
    let minx = a.0.min(b.0).min(c.0);
    let miny = a.1.min(b.1).min(c.1);
    let maxx = a.0.max(b.0).max(c.0);
    let maxy = a.1.max(b.1).max(c.1);
    let caja = Recorte { x0: minx, y0: miny, x1: maxx + 1, y1: maxy + 1 }.interseccion(r);
    if caja.vacio() {
        return;
    }

    // Las tres respuestas de la regla se calculan UNA vez, no por pixel: son
    // propiedad de la arista, no del punto.
    let tl_ab = superior_o_izquierda(a, b);
    let tl_bc = superior_o_izquierda(b, c);
    let tl_ca = superior_o_izquierda(c, a);

    for y in caja.y0..caja.y1 {
        let py2 = 2 * y as i64 + 1;
        let mut inicio: Option<i32> = None;
        for x in caja.x0..caja.x1 {
            let px2 = 2 * x as i64 + 1;
            let esta = dentro(arista(a, b, px2, py2), tl_ab)
                && dentro(arista(b, c, px2, py2), tl_bc)
                && dentro(arista(c, a, px2, py2), tl_ca);
            match (esta, inicio) {
                (true, None) => inicio = Some(x),
                // Se salio: el tramo acaba AQUI (medio abierto) y por
                // convexidad no puede volver a entrar en esta fila.
                (false, Some(x0)) => {
                    tramo(y, x0, x);
                    inicio = None;
                    break;
                }
                _ => {}
            }
        }
        if let Some(x0) = inicio {
            tramo(y, x0, caja.x1);
        }
    }
}

// -- Las pruebas ------------------------------------------------------------
#[cfg(test)]
mod pruebas {
    use super::*;

    struct Lienzo {
        ancho: i32,
        pixeles: [u8; 64 * 64],
    }

    impl Lienzo {
        fn nuevo() -> Self {
            Lienzo { ancho: 64, pixeles: [0; 64 * 64] }
        }
        fn tramo(&mut self, y: i32, x0: i32, x1: i32) {
            for x in x0..x1 {
                assert!(x >= 0 && x < 64 && y >= 0 && y < 64, "tramo fuera: {},{}", x, y);
                self.pixeles[(y * self.ancho + x) as usize] += 1;
            }
        }
        fn en(&self, x: i32, y: i32) -> u8 {
            self.pixeles[(y * self.ancho + x) as usize]
        }
        fn cuantos(&self) -> usize {
            self.pixeles.iter().filter(|&&v| v > 0).count()
        }
    }

    /// ** LA PRUEBA QUE JUSTIFICA LA REGLA TOP-LEFT, y la razon de todo el
    /// modulo.
    ///
    /// Dos triangulos que parten un cuadrado por la diagonal. Juntos tienen que
    /// cubrir el cuadrado **exactamente una vez cada pixel**: ni una costura
    /// sin pintar, ni un pixel pintado dos veces.
    ///
    /// Sin la regla, esta prueba falla por los dos lados a la vez -- y en
    /// pantalla se veria como una raya diagonal mas oscura, que es de las cosas
    /// que se miran diez veces sin entender.
    #[test]
    fn dos_triangulos_que_comparten_arista_cubren_el_cuadrado_una_sola_vez() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut l = Lienzo::nuevo();
        // El cuadrado [0,16) x [0,16), partido por la diagonal (16,0)-(0,16).
        triangulo(&r, (0, 0), (16, 0), (0, 16), |y, x0, x1| l.tramo(y, x0, x1));
        triangulo(&r, (16, 0), (16, 16), (0, 16), |y, x0, x1| l.tramo(y, x0, x1));
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(
                    l.en(x, y), 1,
                    "el pixel {x},{y} se pinto {} veces, y tiene que ser UNA",
                    l.en(x, y)
                );
            }
        }
        assert_eq!(l.cuantos(), 256, "el cuadrado entero, ni un pixel de mas");
    }

    /// Un triangulo rectangulo cubre media caja. Con lados de 16, la mitad
    /// exacta son 136 pixeles contando la diagonal: `16*17/2`.
    #[test]
    fn un_triangulo_rectangulo_cubre_media_caja() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut l = Lienzo::nuevo();
        triangulo(&r, (0, 0), (16, 0), (0, 16), |y, x0, x1| l.tramo(y, x0, x1));
        assert_eq!(l.en(0, 0), 1, "la esquina recta se pinta");
        assert_eq!(l.en(15, 15), 0, "la esquina opuesta no");
        assert_eq!(l.cuantos(), 136);
    }

    /// ** EL SENTIDO DE GIRO NO PUEDE CAMBIAR EL DIBUJO.
    ///
    /// Los mismos tres vertices en los dos ordenes tienen que dar el mismo
    /// relleno, pixel a pixel. Si no se normalizara el area, uno de los dos
    /// saldria vacio -- y quien llama tendria que saberse una regla que no
    /// pidio.
    #[test]
    fn los_dos_sentidos_de_giro_dan_el_mismo_triangulo() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut horario = Lienzo::nuevo();
        let mut antihorario = Lienzo::nuevo();
        triangulo(&r, (3, 2), (20, 9), (7, 25), |y, a, b| horario.tramo(y, a, b));
        triangulo(&r, (3, 2), (7, 25), (20, 9), |y, a, b| antihorario.tramo(y, a, b));
        assert!(horario.cuantos() > 100, "hay triangulo que comparar");
        for y in 0..64 {
            for x in 0..64 {
                assert_eq!(horario.en(x, y), antihorario.en(x, y), "difieren en {},{}", x, y);
            }
        }
    }

    /// Tres puntos en linea no tienen interior. Dibujar la linea "por si
    /// acaso" seria inventarse lo que nadie pidio.
    #[test]
    fn un_triangulo_degenerado_no_pinta_nada() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut n = 0;
        triangulo(&r, (0, 0), (5, 5), (10, 10), |_, x0, x1| n += x1 - x0);
        assert_eq!(n, 0);
    }

    /// El recorte manda sobre la geometria: un triangulo enorme se queda en su
    /// caja y ni un pixel se sale.
    #[test]
    fn el_recorte_manda_sobre_un_triangulo_gigante() {
        let r = Recorte::nuevo(10, 10, 8, 8); // [10,18) x [10,18)
        let mut l = Lienzo::nuevo();
        triangulo(&r, (-500, -500), (900, -100), (100, 900), |y, x0, x1| {
            assert!(y >= 10 && y < 18, "fila fuera del recorte: {}", y);
            assert!(x0 >= 10 && x1 <= 18, "tramo fuera del recorte: {}..{}", x0, x1);
            l.tramo(y, x0, x1);
        });
        assert_eq!(l.cuantos(), 64, "el recorte entero queda cubierto");
    }

    /// Un triangulo fuera del recorte no cuesta ni una pregunta por pixel.
    #[test]
    fn un_triangulo_entero_fuera_no_pinta_nada() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut n = 0;
        triangulo(&r, (100, 100), (120, 100), (100, 120), |_, x0, x1| n += x1 - x0);
        assert_eq!(n, 0);
    }

    /// Cada fila da un tramo y solo uno: es lo que permite pintarlo con un
    /// relleno de fila en vez de pixel a pixel.
    #[test]
    fn cada_fila_entrega_un_solo_tramo() {
        let r = Recorte::nuevo(0, 0, 64, 64);
        let mut filas = [0u8; 64];
        triangulo(&r, (5, 5), (40, 12), (18, 40), |y, _, _| filas[y as usize] += 1);
        for (y, n) in filas.iter().enumerate() {
            assert!(*n <= 1, "la fila {} entrego {} tramos", y, n);
        }
    }
}
