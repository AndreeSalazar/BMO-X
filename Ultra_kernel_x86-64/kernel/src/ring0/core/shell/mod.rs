//! **Las ordenes del shell de Ring 0**, repartidas por lo que hacen.
//!
//! # Por que existe esta carpeta
//!
//! `phase.rs` tenia **27 funciones `shell_*` dentro**, y ocupaban 1.480 de sus
//! 2.328 lineas. El fichero del ARRANQUE era, en dos tercios, un interprete de
//! ordenes -- dos trabajos que no comparten nada salvo el bucle que los junta.
//!
//! El dueno lo puso por su nombre el 2026-08-12: *"si unes tendre deudas.
//! Siempre modular"*.
//!
//! # ** EL ORDEN NO ES ALFABETICO NI POR TAMANO: ES POR LO QUE PUEDE PASAR
//!
//! De lo que solo MIRA a lo que NO SE DESHACE:
//!
//! | # | modulo | que hace | si se equivoca |
//! |---|---|---|---|
//! | 1 | [`hardware`] | pregunta al SILICIO | da un numero raro |
//! | 2 | [`ficheros`] | toca el DISCO | **pierde un archivo** |
//! | 3 | [`pantalla`] | PINTA y reclama la pantalla | se ve mal, se repinta |
//! | 4 | [`peligro`] | reinicia, para, provoca un fault | **no se sigue** |
//!
//! Esa columna de la derecha es el criterio entero. Ordenar un shell por lo que
//! cuesta equivocarse hace que anadir una orden nueva sea una pregunta con
//! respuesta --*"que pasa si esto falla?"*-- en vez de una eleccion de gusto.
//!
//! [!] Y `pantalla` va DESPUES de `ficheros` aunque parezca menos grave: pintar
//! se puede repetir, y en el orden manda lo irreversible, no lo aparatoso.
//!
//! # Lo que se quedo en `phase.rs`, y no es un resto
//!
//! El ARRANQUE y **la LINEA**: el prompt, `read_line`, el historial, `layout` y
//! `help`. Eso no son ordenes: es la herramienta con la que se escriben las
//! ordenes, y vive con el bucle que la usa.
//!
//! O sea que `phase.rs` paso de *"el arranque y 27 comandos"* a **"el arranque y
//! la linea"**, que si es una frase.

/// 1 -- pregunta al SILICIO y cuenta lo que contesta.
pub mod hardware;
/// 2 -- toca el DISCO. El unico grupo donde un fallo se lleva datos.
pub mod ficheros;
/// 3 -- PINTA. Se puede repetir, por eso va antes que lo irreversible.
pub mod pantalla;
/// 4 -- lo que NO SE DESHACE. Despues de estas no se sigue.
pub mod peligro;
