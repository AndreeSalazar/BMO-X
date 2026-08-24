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
/// **`placa`**: lo que el firmware cuenta de si mismo. Aparte de `hardware`
/// porque contesta otra pregunta -- aquel ensena APARATOS y este ensena la
/// TABLA que la placa dejo en memoria.
pub mod placa;
/// 1b -- el CENSO de extensiones. Mismo grupo que [`hardware`] --solo mira--
/// pero fichero propio: aquel pregunta por un APARATO (disco, red, audio) y
/// este por el CONJUNTO DE INSTRUCCIONES. Y `hardware.rs` es el que mas crece
/// del shell; treinta y seis filas mas lo dejarian en mil lineas.
pub mod extensions;
/// 2 -- toca el DISCO. El unico grupo donde un fallo se lleva datos.
pub mod files;
/// 3 -- PINTA. Se puede repetir, por eso va antes que lo irreversible.
pub mod screen;
/// 4 -- lo que NO SE DESHACE. Despues de estas no se sigue.
pub mod danger;

/// The shell's own UI: the colour hierarchy, the `L` row builder, the line
/// editor and the history ring. It stayed behind in `phase.rs` when the
/// commands moved here -- so the thing that READS a command line lived four
/// hundred lines from every command it feeds.
pub mod ui;
/// The loop that never returns. Separate from the editor and from the commands
/// because it is the only part with a lifetime: `run_shell` is `-> !` and owns
/// the keyboard for the rest of the machine's life.
pub mod session;
