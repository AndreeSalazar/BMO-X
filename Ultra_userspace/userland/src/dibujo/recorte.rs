//! # Escalon 0 -- EL RECORTE, que es lo que todos los demas necesitan
//!
//! Antes de saber pintar una linea hay que saber **donde NO se pinta**. Sin
//! esto, cada primitiva tendria que acordarse sola de comprobar los limites, y
//! la que se olvide escribe fuera del framebuffer -- que en Ring 3 es un `#PF`
//! y en el kernel seria peor.
//!
//! ## Por que un modulo y no un `if` en cada sitio
//!
//! Porque el recorte no es "no te salgas de la pantalla": es **el `scissor` de
//! una GPU**, y una ventana lo necesita igual que el borde del monitor. El dia
//! que el compositor pinte dentro de un marco, la primitiva no tiene que
//! enterarse: se le da otro rectangulo y ya.
//!
//! ## El convenio, dicho una vez para las tres primitivas
//!
//! El rectangulo es **medio abierto**: `[x0, x1) x [y0, y1)`. O sea que `x1` e
//! `y1` NO se pintan.
//!
//! No es capricho, y es la decision que evita la clase de bug mas tonta que
//! hay aqui: con intervalos cerrados, dos rectangulos pegados comparten la
//! columna del borde y **se pinta dos veces** -- que se ve con mezcla alfa
//! (escalon 4) y no se ve sin ella, o sea que el bug entra hoy y se descubre
//! dentro de tres escalones. Con medio abierto, `ancho = x1 - x0` y dos cajas
//! contiguas no se tocan. Es el mismo convenio que usa Vulkan en `VkRect2D`.
//!
//! ## Coordenadas con signo, y a proposito
//!
//! Todo va en `i32` aunque la pantalla no tenga pixeles negativos. Una linea
//! que entra desde fuera **tiene** un extremo negativo, y obligarla a `u32`
//! obliga a quien llama a recortar antes -- que es justo el trabajo de este
//! modulo. Un `u32` aqui convierte "esta a la izquierda" en "esta a cuatro mil
//! millones de pixeles a la derecha".
//!
//! [!] **Como se prueban estas pruebas.** `Ultra_userspace` es `no_std` con su
//! propio guion de enlazado, asi que `cargo test` no corre aqui. Este fichero
//! no tiene ni un `unsafe`, ni un puntero, ni una dependencia: se copia tal
//! cual y se ejecuta.
//!
//! ```text
//!    rustc --test recorte.rs -o recorte_test && ./recorte_test
//! ```
//!
//! ** Y asi se hizo antes de commitear. Un `#[cfg(test)]` que nadie ha corrido
//! no es una prueba: es una intencion.

/// Un rectangulo medio abierto: `[x0, x1) x [y0, y1)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Recorte {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Recorte {
    /// Desde esquina y tamano, que es como piensa quien llama.
    pub fn nuevo(x: i32, y: i32, ancho: i32, alto: i32) -> Self {
        Recorte { x0: x, y0: y, x1: x + ancho, y1: y + alto }
    }

    /// El rectangulo que no contiene nada. `vacio()` contesta `true`.
    pub fn nada() -> Self {
        Recorte { x0: 0, y0: 0, x1: 0, y1: 0 }
    }

    /// ** Un recorte invertido o de area cero NO PINTA NADA, y eso es lo
    /// correcto.
    ///
    /// La alternativa --normalizarlo dandole la vuelta-- convierte un error de
    /// quien llama en un dibujo plausible en el sitio equivocado. Que no pinte
    /// se ve; que pinte girado, no.
    pub fn vacio(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    pub fn ancho(&self) -> i32 {
        if self.vacio() { 0 } else { self.x1 - self.x0 }
    }

    pub fn alto(&self) -> i32 {
        if self.vacio() { 0 } else { self.y1 - self.y0 }
    }

    pub fn contiene(&self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    /// La parte comun de dos recortes. Es lo que hace falta para meter una
    /// ventana dentro de la pantalla: recorte de la ventana n recorte del
    /// monitor.
    pub fn interseccion(&self, otro: &Recorte) -> Recorte {
        let r = Recorte {
            x0: if self.x0 > otro.x0 { self.x0 } else { otro.x0 },
            y0: if self.y0 > otro.y0 { self.y0 } else { otro.y0 },
            x1: if self.x1 < otro.x1 { self.x1 } else { otro.x1 },
            y1: if self.y1 < otro.y1 { self.y1 } else { otro.y1 },
        };
        if r.vacio() { Recorte::nada() } else { r }
    }

    /// Recorta un tramo horizontal `y, [xa, xb)` a este rectangulo.
    ///
    /// Devuelve `None` si no queda nada. Es la operacion que usa el relleno de
    /// triangulos, y va aqui --y no alli-- porque es del recorte y no del
    /// triangulo.
    pub fn tramo(&self, y: i32, xa: i32, xb: i32) -> Option<(i32, i32)> {
        if self.vacio() || y < self.y0 || y >= self.y1 {
            return None;
        }
        let a = if xa > self.x0 { xa } else { self.x0 };
        let b = if xb < self.x1 { xb } else { self.x1 };
        if b <= a { None } else { Some((a, b)) }
    }
}

// -- El recorte de una LINEA: Cohen-Sutherland ------------------------------
//
// ** POR QUE NO BASTA CON "si el pixel esta fuera, no lo pintes".
//
// Esa version es correcta y es una trampa: una linea de (-2.000.000, 0) a
// (5, 5) daria dos millones de vueltas descartando pixeles uno a uno. El
// bucle no se ve en una pantalla de 1920 porque los numeros de una ventana
// son pequenos; se ve el dia que alguien calcula una coordenada y le sale
// grande.
//
// Cohen-Sutherland corta la linea ANTES de empezar a andarla, con cuatro bits
// por extremo. Lo unico que hay que saber leer es el codigo:
//
//     1000 arriba   0100 abajo   0010 derecha   0001 izquierda
//
// Y las dos decisiones salen de mirar los dos codigos juntos:
//   - los dos a cero      -> dentro entera, no hay nada que cortar
//   - `a & b != 0`        -> los dos extremos fuera POR EL MISMO LADO, o sea
//                            que la linea entera esta fuera. Se descarta sin
//                            calcular ni una interseccion.

const FUERA_IZQ: u8 = 1;
const FUERA_DER: u8 = 2;
const FUERA_ABAJO: u8 = 4;
const FUERA_ARRIBA: u8 = 8;

fn codigo(r: &Recorte, x: i32, y: i32) -> u8 {
    let mut c = 0;
    if x < r.x0 {
        c |= FUERA_IZQ;
    } else if x > r.x1 - 1 {
        c |= FUERA_DER;
    }
    if y < r.y0 {
        c |= FUERA_ARRIBA;
    } else if y > r.y1 - 1 {
        c |= FUERA_ABAJO;
    }
    c
}

/// Recorta el segmento `(xa,ya)-(xb,yb)` al rectangulo.
///
/// `None` = no queda nada visible. `Some(..)` = los extremos ya recortados,
/// listos para que Bresenham los ande.
///
/// [!] El recorte se hace contra `x1 - 1` / `y1 - 1` --el ultimo pixel que SI
/// se pinta-- y no contra `x1`. El rectangulo es medio abierto para las areas;
/// una linea anda por pixeles, y su ultimo pixel valido es el de dentro.
pub fn recortar_segmento(
    r: &Recorte,
    xa: i32,
    ya: i32,
    xb: i32,
    yb: i32,
) -> Option<(i32, i32, i32, i32)> {
    if r.vacio() {
        return None;
    }
    let (mut xa, mut ya, mut xb, mut yb) = (xa, ya, xb, yb);
    let mut ca = codigo(r, xa, ya);
    let mut cb = codigo(r, xb, yb);

    // ** EL TOPE DE VUELTAS NO ES PARANOIA.
    //
    // El algoritmo converge en cuatro cortes como mucho --uno por lado-- pero
    // eso vale con aritmetica exacta. Aqui se divide con enteros y se trunca,
    // asi que un caso muy oblicuo puede quedarse cortando el mismo pixel. Se
    // para y se descarta: perder una linea de un pixel se ve menos que colgar
    // el compositor, y ademas se puede razonar. Un `loop` sin salida en el
    // camino de pintado es un cuelgue sin panico y sin pista.
    let mut vueltas = 0;
    loop {
        if ca | cb == 0 {
            return Some((xa, ya, xb, yb));
        }
        if ca & cb != 0 {
            return None;
        }
        vueltas += 1;
        if vueltas > 8 {
            return None;
        }

        // Se corta siempre el extremo que este fuera.
        let c = if ca != 0 { ca } else { cb };
        let (x, y);
        if c & FUERA_ABAJO != 0 {
            let borde = r.y1 - 1;
            x = xa + (xb - xa) * (borde - ya) / (yb - ya);
            y = borde;
        } else if c & FUERA_ARRIBA != 0 {
            let borde = r.y0;
            x = xa + (xb - xa) * (borde - ya) / (yb - ya);
            y = borde;
        } else if c & FUERA_DER != 0 {
            let borde = r.x1 - 1;
            y = ya + (yb - ya) * (borde - xa) / (xb - xa);
            x = borde;
        } else {
            let borde = r.x0;
            y = ya + (yb - ya) * (borde - xa) / (xb - xa);
            x = borde;
        }

        if c == ca {
            xa = x;
            ya = y;
            ca = codigo(r, xa, ya);
        } else {
            xb = x;
            yb = y;
            cb = codigo(r, xb, yb);
        }
    }
}

// -- Las pruebas ------------------------------------------------------------
//
// Ver la nota de la cabecera: `rustc --test recorte.rs`.
#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn medio_abierto_significa_que_el_ultimo_no_se_pinta() {
        let r = Recorte::nuevo(0, 0, 10, 10);
        assert!(r.contiene(9, 9), "el 9 es el ultimo pixel y SI se pinta");
        assert!(!r.contiene(10, 9), "el 10 es el borde y NO se pinta");
        assert_eq!(r.ancho(), 10);
    }

    /// La razon de ser del convenio: dos cajas pegadas no comparten columna.
    #[test]
    fn dos_cajas_contiguas_no_se_solapan() {
        let a = Recorte::nuevo(0, 0, 10, 4);
        let b = Recorte::nuevo(10, 0, 10, 4);
        assert!(a.interseccion(&b).vacio(), "pegadas no es solapadas");
        for x in 0..20 {
            let en_a = a.contiene(x, 0);
            let en_b = b.contiene(x, 0);
            assert!(en_a != en_b, "el pixel {} tiene que ser de UNA de las dos", x);
        }
    }

    #[test]
    fn un_recorte_invertido_no_pinta_en_vez_de_darse_la_vuelta() {
        let r = Recorte { x0: 10, y0: 0, x1: 4, y1: 8 };
        assert!(r.vacio());
        assert_eq!(r.ancho(), 0);
        assert!(!r.contiene(6, 4), "no se normaliza: no pinta");
    }

    #[test]
    fn la_interseccion_mete_una_ventana_en_la_pantalla() {
        let pantalla = Recorte::nuevo(0, 0, 100, 50);
        let ventana = Recorte::nuevo(80, 40, 40, 40); // se sale por dos lados
        let v = pantalla.interseccion(&ventana);
        assert_eq!(v, Recorte { x0: 80, y0: 40, x1: 100, y1: 50 });
    }

    #[test]
    fn un_tramo_se_recorta_por_los_dos_lados() {
        let r = Recorte::nuevo(10, 0, 10, 10); // [10,20)
        assert_eq!(r.tramo(5, 0, 100), Some((10, 20)));
        assert_eq!(r.tramo(5, 12, 15), Some((12, 15)));
        assert_eq!(r.tramo(5, 0, 10), None, "acaba justo donde empieza");
        assert_eq!(r.tramo(50, 12, 15), None, "fuera por la fila");
    }

    #[test]
    fn una_linea_entera_dentro_no_se_toca() {
        let r = Recorte::nuevo(0, 0, 100, 100);
        assert_eq!(recortar_segmento(&r, 10, 10, 20, 20), Some((10, 10, 20, 20)));
    }

    /// El caso que justifica el algoritmo: los dos extremos fuera por el mismo
    /// lado se descartan **sin calcular una sola interseccion**.
    #[test]
    fn los_dos_fuera_por_el_mismo_lado_se_descartan() {
        let r = Recorte::nuevo(0, 0, 100, 100);
        assert_eq!(recortar_segmento(&r, -50, 10, -10, 90), None);
        assert_eq!(recortar_segmento(&r, 10, 200, 90, 150), None);
    }

    /// Y el que de verdad importa: una coordenada enorme no se anda pixel a
    /// pixel. Lo que se comprueba es que el resultado cae DENTRO.
    #[test]
    fn una_coordenada_enorme_se_corta_antes_de_andarla() {
        let r = Recorte::nuevo(0, 0, 100, 100);
        let c = recortar_segmento(&r, -2_000_000, 50, 5, 50);
        let (xa, ya, xb, yb) = c.expect("la linea cruza la caja");
        assert!(r.contiene(xa, ya), "extremo A dentro: {},{}", xa, ya);
        assert!(r.contiene(xb, yb), "extremo B dentro: {},{}", xb, yb);
        assert_eq!(xb, 5, "el extremo que ya estaba dentro no se toca");
    }

    /// Una diagonal que entra por una esquina toca los dos bordes: es el caso
    /// que obliga a cortar dos veces, y donde un algoritmo de una sola pasada
    /// devuelve un punto que sigue fuera.
    #[test]
    fn una_diagonal_que_cruza_dos_bordes_queda_dentro() {
        let r = Recorte::nuevo(0, 0, 100, 100);
        let (xa, ya, xb, yb) =
            recortar_segmento(&r, -20, -20, 120, 120).expect("cruza de esquina a esquina");
        assert!(r.contiene(xa, ya), "A dentro: {},{}", xa, ya);
        assert!(r.contiene(xb, yb), "B dentro: {},{}", xb, yb);
    }

    #[test]
    fn contra_un_recorte_vacio_no_sobrevive_nada() {
        let r = Recorte::nada();
        assert_eq!(recortar_segmento(&r, 0, 0, 10, 10), None);
    }
}
