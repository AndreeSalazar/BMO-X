//! `KIND_FRAMEBUFFER` -- la pantalla como capability.
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! ## Que es esto, y por que no es "un syscall para dibujar"
//!
//! La tentacion evidente seria `INVOKE(fb, DRAW_RECT, x, y, w, h)`. Seria mas
//! facil de escribir y seria un error de diseno: cada pixel cruzaria el
//! anillo, el kernel acabaria con un motor de dibujo dentro, y BMO-X seria un
//! monolito con la etiqueta de microkernel puesta encima.
//!
//! Lo que hace este modulo es lo contrario. Un proceso Ring 3 **reclama** la
//! pantalla una vez; el kernel le mapea el framebuffer en su espacio de
//! direcciones con U/S y R/W, le dice donde quedo y con que geometria, y a
//! partir de ahi **no vuelve a intervenir**. El compositor escribe pixeles con
//! `mov`, no con `syscall`. Ese es el momento library-OS: no se optimiza el
//! cruce de frontera, se borra la frontera.
//!
//! ## Exclusiva, y el kernel se calla
//!
//! Un solo proceso la tiene a la vez. Al concederla, el kernel **cede la
//! pantalla**: `info::has_fb()` pasa a ser falso y con eso se apagan de golpe
//! todos los caminos de dibujo de Ring 0 --panel, CABINA, logs de drivers--
//! porque todos preguntan por ahi. Dos duenos pintando el mismo framebuffer no
//! es compartir, es parpadeo.
//!
//! Se recupera sola: `cap::revoke_all` la suelta cuando el proceso muere, por
//! la razon que sea, y el kernel vuelve a tener pantalla. Un compositor que
//! se cae no deja la maquina ciega.
//!
//! ## Lo que este modulo TODAVIA NO decide
//!
//! * Hoy la reclama el primero que la pide. Eso no es cero-confianza, es
//! orden de llegada -- y esta escrito aqui para que se vea, no escondido. La
//! autoridad correcta es una bandera en el contenedor BEF verificada por el
//! gate al admitir el programa: "este binario declara que quiere la pantalla".
//! Cuando esa bandera exista, la comprobacion entra en `claim` y esta nota
//! se borra. Mientras tanto, el unico proceso que la pide es el que tu
//! arrancas.




//! # ** LOS CARRILES (L6g)
//!
//! ```text
//!    roja.rs    conceder, soltar, rescatar y el muerto. Si falla, la maquina
//!               se queda ciega -- o con dos duenos pintando encima
//!    verde.rs   la geometria, los `FB_OP_*` y quien la tiene. Solo contesta
//! ```
//!
//! *** DOS carriles y no tres, a proposito: aqui no hay un "cambia a menudo y
//! arrastra" con masa propia. **Un modulo lleva los carriles que TIENE.**
//!
//! [!] Fuera no cambia nada: `pub use` deja el modulo con la misma cara.

mod roja;
mod verde;

pub use roja::{claim, process_died, release, rescate_de_emergencia, rescue};
pub use verde::{
    operation, owner, ERROR_BUSY, ERROR_NO_SCREEN, FB_OP_BASE, FB_OP_BYTES, FB_OP_DIMS,
    FB_OP_STRIDE,
};

