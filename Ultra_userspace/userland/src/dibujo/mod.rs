//! # DIBUJO -- lo que BMO-X no sabia hacer
//!
//! ## Lo que habia antes de esta carpeta, medido y no supuesto
//!
//! `Pantalla` sabia hacer cinco cosas: `punto`, `rect`, `limpiar`, `glifo` y
//! volcar. **Nada mas.** Todas las "lineas" del escritorio son
//! `rect(x, y, ancho, 1, color)` --rectangulos de un pixel de alto--, el
//! degradado son franjas de rects, y las esquinas redondeadas son una prueba
//! por pixel dentro de un rect. En todo BMO-X **no habia una sola diagonal**.
//!
//! Dicho con precision, que es como hay que decirlo: el sistema no dibujaba
//! mal. **Sabia rellenar rectangulos alineados a los ejes y estampar letras.**
//! DOOM no cuenta -- trae su propio renderizador y solo hace `memcpy`.
//!
//! ## Por que esto NO vive en `sin_gpu/`
//!
//! Fue el primer sitio en el que se penso, y esta mal. La cabecera de esa
//! carpeta lo dice ella misma: *"todo lo de esta carpeta se borra cuando
//! llegue la GPU"*, porque lo de alli son apanos de CPU para una tarjeta que
//! no responde -- el troceado en cajas sucias existe para no copiar 8,3 MB por
//! fotograma, y el dia que haya `page flip` sobra.
//!
//! **Un rasterizador de referencia es lo contrario: tiene que SOBREVIVIR a la
//! GPU**, porque su trabajo entonces empieza. Ver la seccion siguiente.
//!
//! ## ** PARA QUE SIRVE ESTO EL DIA QUE HAYA VULKAN
//!
//! La pregunta del dueno, y es la correcta: *"si un dia llega a Vulkan, si no
//! sabe dibujar menos para Vulkan"*.
//!
//! Vulkan no da "dibujar". Vulkan **pide** que sepas describir un pipeline de
//! rasterizacion: triangulo, recorte, interpolacion de atributos, profundidad,
//! mezcla. Y cuando el driver conteste algo, hace falta poder decir si esta
//! bien -- que con una GPU es justo lo dificil, porque no se puede parar a
//! mirar por dentro.
//!
//! Por eso cada escalon de aqui lleva **el nombre de su pieza en Vulkan** y se
//! escribe con **el mismo algoritmo que usa el silicio**, aunque no sea el mas
//! rapido en una CPU:
//!
//! | aqui | alli |
//! |---|---|
//! | `Recorte` | `VkRect2D` / `scissor` |
//! | la funcion de arista | el rasterizador de triangulos |
//! | la regla top-left | la regla de relleno de D3D y Vulkan, palabra por palabra |
//! | (escalon 3) baricentricas | lo que interpola un fragment shader |
//!
//! O sea que esto **es el oraculo**: misma entrada, dos salidas, comparar. Un
//! driver de GPU sin implementacion de referencia se depura a ojo, que es
//! exactamente como se depuro DOOM hasta el 2026-08-13.
//!
//! ## La escalera
//!
//! ```text
//!   [x] 0  recorte           el scissor -- lo necesitan todos los demas
//!   [x] 1  linea             la primera diagonal del sistema
//!   [x] 1.5 curva            Bezier = polilinea; es lo que pide un grafo
//!   [x] 2  triangulo         la unidad de la GPU
//!   [ ] 3  baricentricas     interpolar color/UV = un fragment shader
//!   [ ] 4  mezcla alfa       ventanas translucidas
//!   [ ] 5  textura           el sampler
//!   [ ] 6  transformada 2D   y ahi ya es el vertex stage
//! ```
//!
//! ## El contrato: la geometria NO conoce el destino
//!
//! Ninguna de las tres primitivas recibe una `Pantalla`. Emiten por callback
//! --puntos la linea, tramos el triangulo-- y quien llama decide donde caen.
//!
//! Eso compra tres cosas de golpe: se prueba en el anfitrion contra un array
//! (que es como estan verdes las 24 pruebas de esta carpeta **ejecutadas de
//! verdad**), sirve igual para pintar en el buffer de una ventana que en el
//! framebuffer, y el dia de la GPU el mismo codigo alimenta la comparacion sin
//! tocar una linea.
//!
//! Los adaptadores a `Pantalla` viven **aqui abajo y solo aqui**: es el unico
//! sitio de la carpeta que sabe que existe una pantalla.
//!
//! ## Quien lo usa hoy
//!
//! El **grafo de ESTRATOS** (`gui/escena/datos.rs`): una curva Bezier por hijo,
//! del padre a cada caja, con punta de flecha. Antes era una espina con codos
//! de rectangulos. Ver el comentario de las aristas alli -- explica por que un
//! codo no necesitaba esto y una curva si.
//!
//! [!] Las pruebas se ejecutan con el arnes: ver `pruebas_sueltas.rs`.

mod curva;
mod linea;
mod recorte;
mod triangulo;

pub use curva::{curva, direccion};
pub use linea::linea;
pub use recorte::{recortar_segmento, Recorte};
pub use triangulo::{triangulo, Vertice};

use crate::Pantalla;

impl Recorte {
    /// El recorte que cubre la pantalla entera. El caso de siempre, y asi no
    /// hay que escribirlo a mano en cada llamada.
    pub fn de_pantalla(p: &Pantalla) -> Recorte {
        Recorte::nuevo(0, 0, p.ancho as i32, p.alto as i32)
    }
}

impl Pantalla {
    /// ** EL RECORTE SIEMPRE SE CRUZA CON LA PANTALLA, y de eso depende lo de
    /// abajo.
    ///
    /// Quien llama puede pasar el rectangulo de una ventana sin haberse
    /// preocupado de si cae entera dentro del panel. Cruzarlo aqui convierte
    /// esa despreocupacion en la garantia que necesitan los adaptadores para
    /// usar el camino caliente.
    fn recorte_seguro(&self, r: &Recorte) -> Recorte {
        r.interseccion(&Recorte::de_pantalla(self))
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
        let (x0, y0, x1, y1) = match recorte::recortar_segmento(&r, xa, ya, xb, yb) {
            Some(t) => t,
            None => return,
        };
        let (mx, my) = (x0.min(x1), y0.min(y1));
        self.marcar(mx as u32, my as u32, ((x0 - x1).abs() + 1) as u32, ((y0 - y1).abs() + 1) as u32);
        linea::linea(&r, x0, y0, x1, y1, |x, y| unsafe {
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
        curva::curva(&r, a, b, c, d, |x, y| unsafe {
            self.punto_sin_comprobar(x as u32, y as u32, color)
        });
    }

    /// Un triangulo relleno, recortado a `r`.
    ///
    /// Se pinta **por tramos** y no por pixeles: cada fila del triangulo es un
    /// `rect` de un pixel de alto, o sea un relleno de fila seguido. Pintarlo
    /// punto a punto costaria una llamada por pixel para el mismo dibujo.
    pub fn triangulo(&self, r: &Recorte, a: Vertice, b: Vertice, c: Vertice, color: u32) {
        triangulo::triangulo(r, a, b, c, |y, x0, x1| {
            self.rect(x0 as u32, y as u32, (x1 - x0) as u32, 1, color);
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
