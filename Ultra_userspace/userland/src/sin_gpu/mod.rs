//! # [!!] TODO LO DE ESTA CARPETA SE BORRA CUANDO LLEGUE LA GPU.
//!
//! No es un aviso amable: es **el proposito del directorio**. Lo que hay aqui
//! dentro existe por UNA razon y solo una --que BMO-X no tiene driver de
//! pantalla-- y el dia que lo tenga, esto no se refactoriza ni se adapta: se
//! borra la carpeta y se quita su `mod`.
//!
//! Dicho por el dueno el 2026-08-12, y con razon:
//!
//! > *"eso es solo para ser reemplazado cuando llegue la GPU, porque la verdad
//! > eso no hara el trabajo para la CPU sino la GPU"*
//!
//! ## Que hace falta para borrarla
//!
//! **Page flip.** Dos framebuffers y cambiar la direccion que lee el escaner de
//! video. Cero copia, cero desgarro, y el coste es **escribir un registro**.
//!
//! Y por que no se puede hoy, con nombre y no con "algun dia": despues de
//! `ExitBootServices` **el GOP ya no existe**. BMO-X tiene la direccion de
//! framebuffer que le dio el firmware y ningun modo de decirle a la tarjeta que
//! mire a otro sitio. Mover la base del escaner son registros del controlador de
//! PANTALLA de la GPU -- `platform/drivers/gpu/rdna4/PLAN_VULKAN.md`, lo
//! aparcado.
//!
//! Es el **escalon 8** de `docs/LA_RAM.md`. Ver ahi el porque completo.
//!
//! ## Por que entonces esto existe
//!
//! Porque mientras tanto la CPU tiene que mover los pixeles igual, y moverlos
//! mal se veia: el volcado copiaba **8,3 MB por fotograma** a memoria
//! write-combining, y eso era a la vez la lentitud y el parpadeo que el dueno
//! reporto. Ver [`sucio`].
//!
//! Pero que funcione no lo convierte en arquitectura. **Es una muleta con
//! fecha de caducidad escrita**, y esta carpeta es la fecha.
//!
//! ## La regla, para quien venga despues
//!
//! No se anade nada aqui que no cumpla las dos:
//!
//! 1. **Existe solo porque no hay driver de pantalla.**
//! 2. **Se borra entero el dia que lo haya**, sin que nadie tenga que decidir
//!    que se queda.
//!
//! Si algo no cumple la segunda, no va aqui: va en `pantalla.rs` con el resto
//! del dibujo, que ese SI se queda.

/// Que trozos de la pantalla han cambiado. Existe para no copiar la pantalla
/// entera cada fotograma -- trabajo que con page flip **no habria que hacer**.
pub mod sucio;
