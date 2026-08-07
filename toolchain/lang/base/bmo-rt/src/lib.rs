//! **`bmo-rt` — LA LIBC DE BMO.** Lo que un `.bex` enlaza.
//!
//! Exporta con nombre C: `crt0` (`_start` → `main` → `exit`), el montón
//! (`malloc`/`free`/`calloc`/`realloc`), las cadenas (`memcpy`, `strlen`,
//! `strcmp`, `strdup`…) y el formato (`printf`, `sprintf`, `snprintf`).
//! Veintitrés símbolos `#[no_mangle]` en total.
//!
//! ═══════════════════════════════════════════════════════════════════════
//! ★ POR QUÉ EXISTE, si el compilador ya emite `printf` en línea
//! ═══════════════════════════════════════════════════════════════════════
//!
//! Ésta es LA pregunta, y sin contestarla este crate parece duplicado. En BMO
//! hay **tres sitios** que saben hacer `memcpy`, y cada uno existe por un
//! motivo distinto:
//!
//! ```text
//!   1. bmo_lower::{memoria, console, fmt}     EN LÍNEA, dentro del .bex
//!      -> el codegen escupe los bytes del bucle en el sitio de la llamada
//!      -> USADO HOY y verificado en el Ryzen
//!      -> cero enlazador, cero relocaciones, cero formato de librería
//!      -> y CADA llamada paga su copia del código
//!
//!   2. toolchain/lang/base/{lib,bmo}/*.c      MÓDULOS EN C
//!      -> fuente C con su BMO.toml, que el sistema de módulos resuelve
//!      -> para lo que se escribe MEJOR en C que emitiendo bytes a mano
//!
//!   3. ESTE CRATE                              SÍMBOLOS ENLAZABLES
//!      -> una implementación, una vez, a la que se llama con `call`
//! ```
//!
//! **No compiten: escalan distinto.** Emitir en línea es perfecto para las
//! seis funciones que un programa pequeño usa —y por eso es lo que corre hoy—
//! y deja de serlo en cuanto un programa usa doscientas. DOOM no es un
//! programa que llame a `memcpy`: es un programa que llama a media libc, y
//! meterle una copia de cada función en cada sitio de llamada infla la imagen
//! sin darle nada a cambio.
//!
//! La regla que decide, y que hay que aplicar función por función:
//!
//! > **En línea lo que no tiene semántica de lenguaje y se usa poco. Enlazado
//! > lo que tiene estado, tamaño, o se llama desde muchos sitios.**
//!
//! `malloc` es el ejemplo claro: tiene **estado** (la lista de libres). Emitirlo
//! en línea significaría un montón por sitio de llamada, que no es un montón.
//!
//! ═══════════════════════════════════════════════════════════════════════
//! ★ QUÉ NO ES — para no chocar con lo que ya existe
//! ═══════════════════════════════════════════════════════════════════════
//!
//! - **No es `bmo-userland`.** Aquel (`Ultra_userspace/userland`) es la API en
//!   **Rust** que usa el compositor: `Pantalla`, `Archivo`, `Directorio`,
//!   `Memoria`. Éste exporta **símbolos C** para que los enlace un `.bex`
//!   compilado. Los dos envuelven los mismos 3 syscalls y **ninguno sustituye
//!   al otro**: distinto consumidor, distinto idioma, distinta forma.
//! - **No es `bmo-abi`.** Aquél es el CONTRATO —números de operación,
//!   estructuras, disposición—; éste es una IMPLEMENTACIÓN que lo usa. Por eso
//!   este crate ya no vive en `platform/abi/`: tenerlo ahí hacía que esa
//!   carpeta significara dos cosas.
//! - **No es un driver.** Tenía dentro un `input/ps2.rs` de 176 líneas — un
//!   driver de teclado PS/2 en una biblioteca estándar. Borrado el 2026-08-02:
//!   la entrada de este sistema es **USB HID por xHCI**, y a Ring 3 le llega
//!   como `KIND_INPUT`. Ni el bus era ése ni el camino.
//!
//! ═══════════════════════════════════════════════════════════════════════
//! ★ ESTADO HONESTO: escrito, y sin un solo usuario
//! ═══════════════════════════════════════════════════════════════════════
//!
//! **Ningún frontend lo enlaza todavía.** Son 1.279 líneas y 6 tests que
//! **pasan desde el 2026-08-07 y no los llama nadie** — hasta ese día la frase
//! aquí decía "que compilan", y era falsa: `cargo test -p bmo-rt` moría al
//! enlazar por el `_start` de `crt0` (ver el `cfg` de abajo) y no ejecutaba ni
//! un test. Se creyó durante días que el montón estaba probado. **Un test que
//! no corre no es una prueba, y no dar salida es peor que fallar** — un fallo
//! se ve; un `running 0 tests` que nadie mira, no.
//!
//! Con eso quitado: 6/6 en verde. El montón (`malloc`/`free`/`calloc`/`realloc`,
//! incluido `test_many_small_allocs`) está **probado de verdad**, y eso es lo
//! único que cambió de estado — la misma categoría que las seis librerías
//! que se borraron el 2026-08-02, y se conserva por una razón concreta y no
//! por cariño: es **el punto 12 de la hoja de ruta**, lo que DOOM necesita, y
//! está escrito.
//!
//! Lo que le falta para ser una libc de verdad, en orden de lo que más duele:
//!
//! 1. **Ficheros**: `fopen`/`fread`/`fclose` sobre `KIND_ARCHIVO`. Sin esto
//!    DOOM no carga su WAD, y es literalmente lo único que le falta al motor.
//! 2. **`malloc` sobre `KIND_MEMORIA`**: hoy el montón toma su arena de un
//!    `backend` que hay que cablear al bloque real (ya verificado en metal).
//! 3. **`printf` completo**: `%f` necesita la ruta SSE, que **desde el
//!    2026-08-02 el emulador ya sabe ejecutar** — así que ahora se puede
//!    probar de verdad.
//! 4. **El enlace**: que un frontend emita las relocaciones contra estos
//!    símbolos en vez de la copia en línea. Es lo que convierte este crate de
//!    "escrito" en "usado", y sin ello los tres puntos de arriba no se notan.
//!
//! Mientras el punto 4 no exista, esto es una promesa. Está dicho aquí para
//! que nadie lo cuente como hecho.

#![no_std]
#![allow(static_mut_refs)]

#[cfg(test)]
extern crate std;

pub mod syscall;
pub mod heap;
pub mod string;
pub mod fmt;

/// ★ FUERA DE LOS TESTS, y no es un detalle de compilación.
///
/// `crt0` define `_start` y referencia `__bss_start`/`__bss_end`, que **sólo
/// los da el script de enlazado de BMO**. En el host, el arnés de `cargo test`
/// intenta enlazar un `.exe` normal, no los encuentra, y muere con `LNK2019`
/// antes de ejecutar nada.
///
/// Por eso los 6 tests del montón **no habían corrido nunca**: no fallaban,
/// que sería una señal — es que no llegaban a existir. `cargo test -p bmo-rt`
/// no imprimía ni un `test result:`, y "escrita y probada" era una hipótesis.
#[cfg(not(test))]
pub mod crt0;

mod init;
pub mod ffi;
