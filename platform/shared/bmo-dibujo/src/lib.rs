//! # DIBUJO -- lo que BMO-X no sabia hacer, y ahora sabe hacer UNA sola vez
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
//! ## ** POR QUE ESTO SE MUDO AQUI, Y CUANTO COSTO NO HABERLO HECHO
//!
//! Esta carpeta vivia en `Ultra_userspace/userland/src/dibujo/`, o sea en Ring
//! 3. Y el kernel no puede alcanzar Ring 3, asi que Ring 0 tenia **su propio
//! dibujante**: un `fill_rect` de diecisiete lineas en `splash.rs` con las
//! comprobaciones de limites escritas a mano.
//!
//! Dos dibujantes, dos reglas. Y no coincidian:
//!
//! ```text
//!    previsualizador   for fx in x.max(0)..(x+w).min(ancho)   -> RECORTA
//!    kernel            if x >= 0 { fill_rect(...) }           -> DESCARTA
//! ```
//!
//! El precio, medido sobre el video del arranque del 2026-08-15 a 1920x1080:
//! **2.625 de los 8.775 rectangulos de cada fotograma se tiraban enteros**, y
//! 149.376 pixeles --el 7,2% de la pantalla-- no se escribian jamas. Entre
//! ellos el bloque del marco que existe justamente para que el borde no quede
//! al aire, que se emite en `x = -34` en cuanto la camara deriva. La franja
//! muerta de 191 px pegada al borde izquierdo se ve en el video a simple vista:
//! se queda congelada con el ultimo color que le cayo encima y no vuelve a
//! cambiar aunque el resto de la pantalla se apague a negro.
//!
//! Ninguna prueba lo cazo, y no por descuido: el previsualizador **ejecuta la
//! otra regla**. Las dos orillas no calculaban lo mismo porque no ejecutaban lo
//! mismo, que es el argumento de `bmo-hash` al reves.
//!
//! De ahi la mudanza y de ahi [`Lienzo`]: mientras la geometria entregue
//! coordenadas crudas, cada consumidor decide que hacer con una `x` negativa --
//! y ya sabemos que deciden distinto. Un `trait` que solo deja ver coordenadas
//! **ya recortadas** convierte esa decision en algo que no se puede escribir.
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
//! | [`Recorte`] | `VkRect2D` / `scissor` |
//! | [`Lienzo`] | el framebuffer attachment |
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
//!   [x] 0.5 lienzo           DONDE caen los pixeles. El recorte, aplicado.
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
//! Ninguna de las primitivas recibe una pantalla. Emiten por callback --puntos
//! la linea, tramos el triangulo-- o contra un [`Lienzo`], y quien llama decide
//! donde caen.
//!
//! Eso compra tres cosas de golpe: se prueba en el anfitrion contra un array,
//! sirve igual para pintar en el buffer de una ventana que en el framebuffer,
//! y el dia de la GPU el mismo codigo alimenta la comparacion sin tocar una
//! linea.
//!
//! ## Las pruebas, que ahora CORREN
//!
//! Mientras esto vivio en Ring 3 hubo que mantener un arnes a mano
//! (`pruebas_sueltas.rs`) porque `Ultra_userspace` es `no_std` con su propio
//! guion de enlazado y `cargo test` no arranca alli. Ese fichero decia:
//! *"El dia que exista un sitio donde estas corran solas, se borra este
//! fichero"*. Aqui es ese sitio, y por eso ya no esta:
//!
//! ```text
//!    cargo test -p bmo-dibujo
//! ```

#![cfg_attr(not(test), no_std)]

mod curva;
mod lienzo;
mod linea;
mod recorte;
mod triangulo;

pub use curva::{curva, direccion};
pub use lienzo::Lienzo;
pub use linea::linea;
pub use recorte::{recortar_segmento, Recorte};
pub use triangulo::{triangulo, triangulo_suave, Vertice, COBERTURA_LLENA, MUESTRAS};

/// Un color de 32 bits tal y como lo quiere el framebuffer: `0xAARRGGBB`.
///
/// Vive aqui y no en cada crate que pinta porque el dia que haya un formato de
/// pixel distinto --y lo habra, en cuanto una GPU diga otra cosa-- el sitio
/// donde se cambia tiene que ser uno.
pub type Color = u32;

/// Mezcla `frente` sobre `fondo` con `parte` de `total` de cobertura.
///
/// Entera y por canal. No hay coma flotante en Ring 3 ni en Ring 0, ni falta
/// que hace: la cobertura llega como "cuantas muestras de dieciseis", que ya es
/// una fraccion exacta.
///
/// [!] Los canales se mezclan **en el espacio del framebuffer** (sRGB sin
/// linealizar), que es lo que hace todo el mundo y lo que hace la GPU por
/// defecto. Es ligeramente incorrecto --mezclar luz de verdad pide linealizar-- y
/// se deja dicho porque sera la segunda diferencia que aparezca al comparar con
/// la tarjeta, despues del patron de muestras.
///
/// Con `total = 0` devuelve `fondo`: una cobertura de cero muestras sobre cero
/// muestras es "no se toco este pixel", no una division por cero.
pub fn mezclar(frente: Color, fondo: Color, parte: u32, total: u32) -> Color {
    if total == 0 {
        return fondo;
    }
    let parte = if parte > total { total } else { parte };
    let inv = total - parte;
    let canal = |desp: u32| {
        let f = (frente >> desp) & 0xFF;
        let b = (fondo >> desp) & 0xFF;
        ((f * parte + b * inv) / total) & 0xFF
    };
    0xFF00_0000 | (canal(16) << 16) | (canal(8) << 8) | canal(0)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn mezclar_a_los_extremos_devuelve_los_extremos() {
        assert_eq!(mezclar(0xFF112233, 0xFF445566, 0, 10), 0xFF445566);
        assert_eq!(mezclar(0xFF112233, 0xFF445566, 10, 10), 0xFF112233);
    }

    /// Pasarse de `total` no desborda: se recorta. Un color desbordado se ve
    /// como un pixel de otro color en mitad de un degradado.
    #[test]
    fn pasarse_de_total_se_recorta() {
        assert_eq!(mezclar(0xFF00E5FF, 0xFF000000, 99, 10), 0xFF00E5FF);
    }

    /// El alfa sale siempre opaco: el framebuffer no mezcla, y un alfa a cero
    /// se veria negro sin motivo.
    #[test]
    fn el_alfa_siempre_sale_opaco() {
        assert_eq!(mezclar(0x00FFFFFF, 0x00000000, 3, 10) & 0xFF00_0000, 0xFF00_0000);
    }

    /// Un total de cero no divide por cero: contesta el fondo.
    #[test]
    fn un_total_de_cero_devuelve_el_fondo() {
        assert_eq!(mezclar(0xFFFFFFFF, 0xFF123456, 5, 0), 0xFF123456);
    }
}
