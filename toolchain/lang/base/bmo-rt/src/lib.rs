//! **`bmo-rt` -- LA LIBC DE BMO.** Lo que un `.bex` enlaza.
//!
//! Exporta con nombre C: `crt0` (`_start` -> `main` -> `exit`), el monton
//! (`malloc`/`free`/`calloc`/`realloc`), las cadenas (`memcpy`, `strlen`,
//! `strcmp`, `strdup`...) y el formato (`printf`, `sprintf`, `snprintf`).
//! Veintitres simbolos `#[no_mangle]` en total.
//!
//! =======================================================================
//! * POR QUE EXISTE, si el compilador ya emite `printf` en linea
//! =======================================================================
//!
//! Esta es LA pregunta, y sin contestarla este crate parece duplicado. En BMO
//! hay **tres sitios** que saben hacer `memcpy`, y cada uno existe por un
//! motivo distinto:
//!
//! ```text
//!   1. bmo_lower::{memoria, console, fmt}     EN LINEA, dentro del .bex
//!      -> el codegen escupe los bytes del bucle en el sitio de la llamada
//!      -> USADO HOY y verificado en el Ryzen
//!      -> cero enlazador, cero relocaciones, cero formato de libreria
//!      -> y CADA llamada paga su copia del codigo
//!
//!   2. toolchain/lang/base/{lib,bmo}/*.c      MODULOS EN C
//!      -> fuente C con su BMO.toml, que el sistema de modulos resuelve
//!      -> para lo que se escribe MEJOR en C que emitiendo bytes a mano
//!
//!   3. ESTE CRATE                              SIMBOLOS ENLAZABLES
//!      -> una implementacion, una vez, a la que se llama con `call`
//! ```
//!
//! **No compiten: escalan distinto.** Emitir en linea es perfecto para las
//! seis funciones que un programa pequeno usa --y por eso es lo que corre hoy--
//! y deja de serlo en cuanto un programa usa doscientas. DOOM no es un
//! programa que llame a `memcpy`: es un programa que llama a media libc, y
//! meterle una copia de cada funcion en cada sitio de llamada infla la imagen
//! sin darle nada a cambio.
//!
//! La regla que decide, y que hay que aplicar funcion por funcion:
//!
//! > **En linea lo que no tiene semantica de lenguaje y se usa poco. Enlazado
//! > lo que tiene estado, tamano, o se llama desde muchos sitios.**
//!
//! `malloc` es el ejemplo claro: tiene **estado** (la lista de libres). Emitirlo
//! en linea significaria un monton por sitio de llamada, que no es un monton.
//!
//! =======================================================================
//! * QUE NO ES -- para no chocar con lo que ya existe
//! =======================================================================
//!
//! - **No es `bmo-userland`.** Aquel (`Ultra_userspace/userland`) es la API en
//!   **Rust** que usa el compositor: `Pantalla`, `Archivo`, `Directorio`,
//!   `Memoria`. Este exporta **simbolos C** para que los enlace un `.bex`
//!   compilado. Los dos envuelven los mismos 3 syscalls y **ninguno sustituye
//!   al otro**: distinto consumidor, distinto idioma, distinta forma.
//! - **No es `bmo-abi`.** Aquel es el CONTRATO --numeros de operacion,
//!   estructuras, disposicion--; este es una IMPLEMENTACION que lo usa. Por eso
//!   este crate ya no vive en `platform/abi/`: tenerlo ahi hacia que esa
//!   carpeta significara dos cosas.
//! - **No es un driver.** Tenia dentro un `input/ps2.rs` de 176 lineas -- un
//!   driver de teclado PS/2 en una biblioteca estandar. Borrado el 2026-08-02:
//!   la entrada de este sistema es **USB HID por xHCI**, y a Ring 3 le llega
//!   como `KIND_INPUT`. Ni el bus era ese ni el camino.
//!
//! =======================================================================
//! * ESTADO HONESTO: escrito, y sin un solo usuario
//! =======================================================================
//!
//! **Ningun frontend lo enlaza todavia.** Son 1.279 lineas y 6 tests que
//! **pasan desde el 2026-08-07 y no los llama nadie** -- hasta ese dia la frase
//! aqui decia "que compilan", y era falsa: `cargo test -p bmo-rt` moria al
//! enlazar por el `_start` de `crt0` (ver el `cfg` de abajo) y no ejecutaba ni
//! un test. Se creyo durante dias que el monton estaba probado. **Un test que
//! no corre no es una prueba, y no dar salida es peor que fallar** -- un fallo
//! se ve; un `running 0 tests` que nadie mira, no.
//!
//! Con eso quitado: 6/6 en verde. El monton (`malloc`/`free`/`calloc`/`realloc`,
//! incluido `test_many_small_allocs`) esta **probado de verdad**, y eso es lo
//! unico que cambio de estado -- la misma categoria que las seis librerias
//! que se borraron el 2026-08-02, y se conserva por una razon concreta y no
//! por carino: es **el punto 12 de la hoja de ruta**, lo que DOOM necesita, y
//! esta escrito.
//!
//! Lo que le falta para ser una libc de verdad, en orden de lo que mas duele:
//!
//! 1. **Ficheros**: `fopen`/`fread`/`fclose` sobre `KIND_ARCHIVO`. Sin esto
//!    DOOM no carga su WAD, y es literalmente lo unico que le falta al motor.
//! 2. **`malloc` sobre `KIND_MEMORIA`**: hoy el monton toma su arena de un
//!    `backend` que hay que cablear al bloque real (ya verificado en metal).
//! 3. **`printf` completo**: `%f` necesita la ruta SSE, que **desde el
//!    2026-08-02 el emulador ya sabe ejecutar** -- asi que ahora se puede
//!    probar de verdad.
//! 4. **El enlace**: que un frontend emita las relocaciones contra estos
//!    simbolos en vez de la copia en linea. Es lo que convierte este crate de
//!    "escrito" en "usado", y sin ello los tres puntos de arriba no se notan.
//!
//! Mientras el punto 4 no exista, esto es una promesa. Esta dicho aqui para
//! que nadie lo cuente como hecho.

#![no_std]
#![allow(static_mut_refs)]

#[cfg(test)]
extern crate std;

pub mod syscall;
pub mod heap;
pub mod string;
pub mod fmt;

/// * FUERA DE LOS TESTS, y no es un detalle de compilacion.
///
/// `crt0` define `_start` y referencia `__bss_start`/`__bss_end`, que **solo
/// los da el script de enlazado de BMO**. En el host, el arnes de `cargo test`
/// intenta enlazar un `.exe` normal, no los encuentra, y muere con `LNK2019`
/// antes de ejecutar nada.
///
/// Por eso los 6 tests del monton **no habian corrido nunca**: no fallaban,
/// que seria una senal -- es que no llegaban a existir. `cargo test -p bmo-rt`
/// no imprimia ni un `test result:`, y "escrita y probada" era una hipotesis.
#[cfg(not(test))]
pub mod crt0;

mod init;
pub mod ffi;
