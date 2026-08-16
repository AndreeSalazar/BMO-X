//! # LA CIUDAD -- el arranque de BMO-X, dibujado por la CPU
//!
//! Pedido por el dueno con dos capturas de Geoxor delante: *"cuando se prenda mi
//! maquina el gato se ve neon y la CPU dibuja todo en pixeles"*, *"que se sienta
//! que tiene animacion y como la camara avanza"*, y *"un crate independiente
//! fuera del Ring 0 y Ring 3"*.
//!
//! ## Las piezas, y por que estan separadas
//!
//! ```text
//!    paleta.rs   los colores. Lo UNICO que se juzga a ojo.
//!    azar.rs     xorshift + el revoltijo de las ventanas.
//!    torre.rs    que ES una torre y como se pinta su fachada.
//!    ciudad.rs   cuantas hay, donde, y el cielo.
//!    marco.rs    las masas de los bordes: el plano que te pone DENTRO.
//!    halo.rs     el cielo ENCENDIDO detras del logo. Un neon ilumina el aire.
//!    niebla.rs   la bruma que despega los planos, y los jirones que cruzan.
//!    camara.rs   el paralaje: cuanto se ha movido cada capa.
//!    acto.rs     el GUION: que hay en pantalla en el milisegundo N.
//! ```
//!
//! El corte no es por tamano: es por **quien cambia cada cosa**. La paleta la
//! toca quien quiera otro azul; `ciudad.rs`, quien quiera otra skyline;
//! `torre.rs`, quien cambie el estilo de las ventanas; `acto.rs`, quien ajuste
//! los tiempos. Juntos, tocar uno obliga a leer los otros cinco.
//!
//! ## Por que se DIBUJA y no se guarda
//!
//! Una pantalla de 1920x1080 en pixel art guardada como bitmap son megabytes, no
//! cabe en el kernel, y **solo sirve para esa resolucion**: en otro panel hay que
//! estirarla, y estirar pixel art lo destruye -- deja de tener pixeles cuadrados,
//! que es justo lo que lo define.
//!
//! Generada son **cero bytes de datos**, se compone a la medida del panel que
//! haya, y puede decir cosas.
//!
//! ## ** LA CIUDAD ES EL ESTADO DEL SISTEMA
//!
//! Idea del dueno: *"en el fondo se ve el sistema de ciudad con TODO"*.
//!
//! [`Ciudad::encender`] enciende las torres de izquierda a derecha, asi que se
//! lee como una barra de progreso sin poner un numero. Cada torre puede ser un
//! subsistema y sus ventanas un dato real. **Una torre que se queda negra es un
//! subsistema que no arranco**, y eso se ve desde el otro lado de la habitacion
//! sin leer una linea de log. Es CABINA dibujada como skyline.
//!
//! ## Y los dientes aqui NO son un defecto
//!
//! El escalon 2.5 del rasterizador (`triangulo_suave`) existe para quitar los
//! bordes escalonados. Aqui **no se usa a proposito**: en pixel art el borde duro
//! ES el dibujo. Dicho por el dueno: *"asi con dientes no me importa"*, y que la
//! cobertura llegue el dia de la RDNA4.
//!
//! ## Nada de esto toca una pantalla
//!
//! Todo emite `rect(x, y, w, h, color)` por callback, y el guion es una funcion
//! del tiempo. Sin destino y sin dormir, esto es **aritmetica** -- y la
//! aritmetica se prueba en el anfitrion. Un arranque animado que solo se puede
//! juzgar reiniciando la maquina es un arranque que nadie va a ajustar nunca.
//!
//! [!] Por eso vive en `platform/shared/`, con el precedente exacto de
//! `bmo-hash`: *"el unico BLAKE3 del sistema"*, compartido por las dos orillas
//! porque ejecutan el MISMO codigo.

#![cfg_attr(not(test), no_std)]

pub mod acto;
pub mod azar;
pub mod camara;
pub mod ciudad;
/// Donde cae cada pieza del logo. Salio del kernel el 2026-08-15 porque ahi
/// **solo se podia juzgar reiniciando la maquina** -- ver su cabecera.
pub mod encuadre;
/// El aura del logo: el cielo ENCENDIDO detras del gato. Vive aqui y no con el
/// gato porque es cielo respondiendo a una luz -- ver su cabecera.
pub mod halo;
/// Las dos masas oscuras de los bordes: el plano que te mete DENTRO de la
/// escena. Es lo que separa una foto de una ciudad de estar en la calle.
pub mod marco;
pub mod niebla;
pub mod paleta;
pub mod torre;

pub use acto::{fotograma, Acto, Fotograma, DURACION_MS};
pub use encuadre::{componer, Encuadre, Medidas};
pub use halo::aura;
pub use camara::Camara;
pub use ciudad::{Ciudad, MAX_TORRES};
pub use paleta::*;
pub use torre::Torre;
