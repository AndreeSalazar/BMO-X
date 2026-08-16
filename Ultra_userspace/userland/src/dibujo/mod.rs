//! # DIBUJO -- los adaptadores de Ring 3 al rasterizador compartido
//!
//! ## ** ESTA CARPETA SE VACIO EL 2026-08-15, Y ESE ES EL ARREGLO
//!
//! Aqui vivian `recorte.rs`, `linea.rs`, `curva.rs` y `triangulo.rs`. Ya no:
//! estan en **`platform/shared/bmo-dibujo`**, que es un crate que alcanzan las
//! tres orillas.
//!
//! El motivo, con el numero delante. Mientras el rasterizador vivio aqui, el
//! kernel no podia usarlo --Ring 0 no alcanza Ring 3-- asi que tenia el suyo:
//! un `fill_rect` con las comprobaciones de limites a mano. Y no comprobaba
//! igual. Este recortaba; aquel **descartaba el rectangulo entero** en cuanto
//! una esquina se salia. A 1920x1080 eso eran 2.625 rectangulos tirados por
//! fotograma y el 7,2% de la pantalla sin escribir nunca, con una franja muerta
//! de 191 px pegada al borde izquierdo que se ve en el video del arranque.
//!
//! El previsualizador no podia cazarlo porque **ejecutaba la otra regla**. Dos
//! implementaciones de la misma decision es una que esta mal y nadie sabe cual.
//!
//! ## Lo que queda aqui, y por que solo esto
//!
//! Los **adaptadores a `Pantalla`**: el unico sitio del sistema que sabe a la
//! vez de geometria y de cajas sucias. La geometria no puede saber de cajas
//! sucias --no existen fuera de Ring 3-- y `Pantalla` no tiene por que saber de
//! Bezier. Esta carpeta es la costura entre las dos, y por eso no se mudo.
//!
//! ## Quien lo usa hoy
//!
//! El **grafo de ESTRATOS** (`gui/escena/datos.rs`): una curva Bezier por hijo,
//! del padre a cada caja, con punta de flecha. Antes era una espina con codos
//! de rectangulos. Ver el comentario de las aristas alli -- explica por que un
//! codo no necesitaba esto y una curva si.
//!
//! [!] Las pruebas ya no necesitan arnes: `cargo test -p bmo-dibujo`.

pub use bmo_dibujo::{
    curva, direccion, linea, mezclar, recortar_segmento, triangulo, triangulo_suave, Color, Lienzo,
    Recorte, Vertice, COBERTURA_LLENA, MUESTRAS,
};

use crate::Pantalla;

impl Pantalla {
    /// El recorte que cubre la pantalla entera. El caso de siempre, y asi no
    /// hay que escribirlo a mano en cada llamada.
    pub fn recorte(&self) -> Recorte {
        Recorte::nuevo(0, 0, self.ancho as i32, self.alto as i32)
    }

    /// ** EL RECORTE SIEMPRE SE CRUZA CON LA PANTALLA, y de eso depende lo de
    /// abajo.
    ///
    /// Quien llama puede pasar el rectangulo de una ventana sin haberse
    /// preocupado de si cae entera dentro del panel. Cruzarlo aqui convierte
    /// esa despreocupacion en la garantia que necesitan los adaptadores para
    /// usar el camino caliente.
    fn recorte_seguro(&self, r: &Recorte) -> Recorte {
        r.interseccion(&self.recorte())
    }

    /// Un segmento de `(xa,ya)` a `(xb,yb)`, recortado a `r`.
    ///
    /// ** LA PRIMERA DIAGONAL DE BMO-X. Hasta hoy, una "linea" era un
    /// `rect(..., ancho, 1, color)` y solo podia ser horizontal o vertical.
    ///
    /// ## Por que NO usa `punto()`, que seria lo obvio
    ///
    /// Porque `punto()` llama a `marcar(x, y, 1, 1)` -- **una caja sucia por
    /// pixel**. Una curva de trescientos pixeles serian trescientas fusiones
    /// contra el juego de ocho cajas, cada fotograma, para acabar en la misma
    /// caja que se puede calcular de antemano.
    ///
    /// Aqui se marca **una vez** la caja del segmento ya recortado y se pinta
    /// por `punto_sin_comprobar`. Y eso es seguro por una razon concreta, no
    /// por confianza: `recortar_segmento` devuelve los dos extremos DENTRO del
    /// recorte, el recorte esta cruzado con la pantalla, y el camino de
    /// Bresenham no se sale de la caja que forman sus extremos. O sea que el
    /// escalon 0 no era solo higiene: **es lo que paga esto**.
    pub fn linea(&self, r: &Recorte, xa: i32, ya: i32, xb: i32, yb: i32, color: u32) {
        let r = self.recorte_seguro(r);
        let (x0, y0, x1, y1) = match recortar_segmento(&r, xa, ya, xb, yb) {
            Some(t) => t,
            None => return,
        };
        let (mx, my) = (x0.min(x1), y0.min(y1));
        self.marcar(mx as u32, my as u32, ((x0 - x1).abs() + 1) as u32, ((y0 - y1).abs() + 1) as u32);
        linea(&r, x0, y0, x1, y1, |x, y| unsafe {
            self.punto_sin_comprobar(x as u32, y as u32, color)
        });
    }

    /// Una Bezier cubica de `a` a `d` con tirantes `b` y `c`.
    ///
    /// Marca la caja de los cuatro puntos de control una vez -- la curva no
    /// sale del casco convexo de sus puntos de control, que es una propiedad de
    /// las Bezier y no una estimacion prudente.
    pub fn curva(
        &self,
        r: &Recorte,
        a: Vertice,
        b: Vertice,
        c: Vertice,
        d: Vertice,
        color: u32,
    ) {
        let r = self.recorte_seguro(r);
        let caja = Recorte {
            x0: a.0.min(b.0).min(c.0).min(d.0),
            y0: a.1.min(b.1).min(c.1).min(d.1),
            x1: a.0.max(b.0).max(c.0).max(d.0) + 1,
            y1: a.1.max(b.1).max(c.1).max(d.1) + 1,
        }
        .interseccion(&r);
        if caja.vacio() {
            return;
        }
        self.marcar(caja.x0 as u32, caja.y0 as u32, caja.ancho() as u32, caja.alto() as u32);
        curva(&r, a, b, c, d, |x, y| unsafe {
            self.punto_sin_comprobar(x as u32, y as u32, color)
        });
    }

    /// Un triangulo relleno, recortado a `r`.
    ///
    /// Se pinta **por tramos** y no por pixeles: cada fila del triangulo es un
    /// `rect` de un pixel de alto, o sea un relleno de fila seguido. Pintarlo
    /// punto a punto costaria una llamada por pixel para el mismo dibujo.
    pub fn triangulo(&self, r: &Recorte, a: Vertice, b: Vertice, c: Vertice, color: u32) {
        triangulo(r, a, b, c, |y, x0, x1| {
            self.rect(x0 as u32, y as u32, (x1 - x0) as u32, 1, color);
        });
    }

    /// **El triangulo con los bordes suaves**, mezclado contra `fondo`.
    ///
    /// # Por que hay que DECIRLE el fondo en vez de leerlo
    ///
    /// Mezclar es `color * cobertura + fondo * (1 - cobertura)`, asi que hace
    /// falta saber que habia debajo. Lo natural seria leer el pixel del
    /// framebuffer... y es justo lo que no se puede hacer aqui: **el
    /// framebuffer es memoria write-combining, y leer de ahi va lentisimo**.
    /// Es la misma trampa que ya se esquivo en el blit de DOOM, donde copiar la
    /// fila desde la pantalla parecia lo obvio y era el peor camino de todos.
    ///
    /// Asi que el fondo se pasa. Quien dibuja sabe sobre que dibuja; la
    /// pantalla no tiene por que contarlo.
    ///
    /// [!] Va **pixel a pixel** y no por tramos, y no es un descuido: cada
    /// pixel del borde lleva su propia mezcla. El interior podria ir por filas
    /// --es todo cobertura llena-- y esa es la optimizacion obvia el dia que
    /// esto pinte algo grande. Hoy pinta logos.
    pub fn triangulo_suave(
        &self,
        r: &Recorte,
        a: Vertice,
        b: Vertice,
        c: Vertice,
        color: u32,
        fondo: u32,
    ) {
        triangulo_suave(r, a, b, c, |x, y, cob| {
            let c = if cob >= COBERTURA_LLENA {
                color
            } else {
                mezclar(color, fondo, cob as u32, COBERTURA_LLENA as u32)
            };
            self.punto(x as u32, y as u32, c);
        });
    }

    /// El contorno de un triangulo, que es lo que hace falta para depurar el
    /// relleno: se dibujan los dos y tienen que coincidir.
    pub fn triangulo_borde(&self, r: &Recorte, a: Vertice, b: Vertice, c: Vertice, color: u32) {
        self.linea(r, a.0, a.1, b.0, b.1, color);
        self.linea(r, b.0, b.1, c.0, c.1, color);
        self.linea(r, c.0, c.1, a.0, a.1, color);
    }
}

/// **La pantalla de Ring 3 ES un lienzo**, y por eso el codigo de dibujo que se
/// escriba contra [`Lienzo`] sirve igual aqui que en el kernel.
///
/// [!] **Los dos `rect` se llaman igual, y hay que escribir cual se quiere.**
/// `Pantalla` tiene un `rect` inherente de toda la vida con coordenadas `u32`,
/// y el trait trae el suyo con `i32`. Dentro de este `impl` el del trait gana
/// la resolucion --el compilador lo dijo a la primera-- asi que las llamadas de
/// aqui abajo van con el nombre del tipo por delante. Fuera de este bloque el
/// escritorio sigue llamando al inherente sin enterarse de nada.
///
/// La puerta del lienzo --la que acepta negativos y recorta-- es la que ve el
/// codigo generico sobre `impl Lienzo`, que es exactamente el que se quiere
/// poder compartir con Ring 0.
impl Lienzo for Pantalla {
    fn recorte(&self) -> Recorte {
        Pantalla::recorte(self)
    }

    fn rect_dentro(&mut self, r: Recorte, color: Color) {
        // Llega garantizado dentro de la pantalla y no vacio: el trait ya
        // cruzo el rectangulo con `recorte()`. De ahi que los `as u32` sean
        // seguros y no una esperanza.
        Pantalla::rect(self, r.x0 as u32, r.y0 as u32, r.ancho() as u32, r.alto() as u32, color);
    }
}
