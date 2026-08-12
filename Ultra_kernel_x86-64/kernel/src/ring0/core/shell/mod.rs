//! **Las ordenes del shell de Ring 0**, repartidas por lo que preguntan.
//!
//! # Por que existe esta carpeta
//!
//! `phase.rs` tenia **27 funciones `shell_*` dentro**, y ocupaban 1.480 de sus
//! 2.328 lineas. O sea que el fichero del ARRANQUE era, en dos tercios, un
//! interprete de ordenes -- dos trabajos que no comparten nada salvo el bucle
//! que los junta.
//!
//! El dueno lo puso por su nombre el 2026-08-12: *"si unes tendre deudas.
//! Siempre modular"*.
//!
//! # El reparto, y la regla que lo decide
//!
//! No por tamano: **por a QUIEN le preguntan**.
//!
//! | modulo | pregunta |
//! |---|---|
//! | [`hardware`] | al SILICIO: cpu, memoria, red, audio, disco, nucleos |
//! | `phase.rs` (aun) | al DISCO y al sistema: `ls`, `run`, ESTRATOS, la bitacora |
//!
//! Esa frontera no es estetica: las de [`hardware`] son las que crecen cada vez
//! que aparece un sensor nuevo --y en esta sesion crecieron tres veces-- mientras
//! que las de disco llevan semanas quietas. Separar lo que se mueve de lo que no
//! es la mitad del valor de partir un fichero.
//!
//! # Lo que NO se ha partido todavia, y lo que costaria
//!
//! Quedan en `phase.rs` las ordenes de fichero (`ls`, `run`, `bex`, `estratos`),
//! las de la propia linea (`prompt`, `read_line`, `hist`, `layout`, `help`) y las
//! peligrosas (`panic`, `reboot`, `halt`, `ktest`). **Son otros ~750 lineas y
//! tres cortes mas**, y se dicen en vez de dejarlos implicitos: un corte a medias
//! anunciado como completo es peor que no cortar.
//!
//! El siguiente natural es el de FICHERO, porque es el unico grupo que toca el
//! disco y por tanto el unico donde un fallo se lleva datos.

/// Las ordenes que le preguntan al SILICIO y cuentan lo que contesta.
pub mod hardware;
