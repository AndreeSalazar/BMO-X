/* semantic.h -- **el metal como vocabulario del lenguaje**.
 *
 * ==========================================================================
 *  Que es esto
 * ==========================================================================
 *
 * Una libreria donde **cada funcion ES una instruccion**. No una envoltura que
 * llama a algo: al compilar, `barrera_total()` se convierte en los tres bytes
 * `0F AE F0` y no queda ni la llamada.
 *
 * No es un invento de BMO. Es lo que hace todo el mundo:
 *
 *   GCC     `__builtin_*` (miles) y `<immintrin.h>`
 *   MSVC    `<intrin.h>` -- `__readmsr`, `__outbyte`, `_mm_*`
 *   Clang   los mismos `__builtin_*` y el mismo `<immintrin.h>`
 *
 * `<immintrin.h>` es exactamente esto: funciones que son instrucciones.
 *
 * ==========================================================================
 *  En que se diferencia la de BMO
 * ==========================================================================
 *
 * En GCC y en Clang esos intrinsecos estan **cableados en C++ dentro del
 * compilador** -- `BuiltinsX86.def` son miles de lineas, y anadir una
 * instruccion es parchear el compilador, recompilarlo y esperar a la siguiente
 * version.
 *
 * En BMO son **una fila de TOML**:
 * `forge/sem-asm/tables/arch/x86_64/intrinsics.toml`. Ahi esta el nombre, los
 * bytes EXACTOS, a que registro va cada argumento y de cual sale el resultado.
 * Anadir una instruccion es una fila. Cero Rust.
 *
 * Y esa tabla se verifica: hay una prueba que **compila una llamada a cada
 * fila**, asi que un nombre de registro mal escrito falla en el banco y no en
 * el Ryzen seis meses despues.
 *
 * ==========================================================================
 *  Los dos pisos, y por que hay dos
 * ==========================================================================
 *
 *   __outb(0x60, 0x20)                  la instruccion, cruda
 *   puerto_byte(0x60, 0x20)             la instruccion, con tipos y nombre
 *
 * El de abajo lo da el compilador y lleva `__` porque **ese espacio de nombres
 * esta reservado a la implementacion** por el propio estandar de C (section 7.1.3).
 * Es el sitio correcto para algo que no es del lenguaje sino de quien lo
 * implementa.
 *
 * El de arriba es este fichero, y anade tres cosas que la tabla no puede dar:
 *
 *   1. **Tipos.** `__outb` acepta cualquier cosa; `puerto_byte(u16, u8)` dice
 *      que cabe. Un puerto de 32 bits no existe.
 *   2. **Agrupacion.** Puertos, barreras, CPU, memoria, atomos y bits, cada uno
 *      en su fichero. Buscar "como se lee un MSR" no deberia obligar a leer
 *      cuarenta filas de TOML.
 *   3. **El manual.** Que anillo pide, que destruye, y cuando NO usarla. Eso no
 *      cabe en una fila y es lo que evita el bug.
 *
 * ==========================================================================
 *  Lo que una libreria NO puede darte
 * ==========================================================================
 *
 * Sintaxis. Esto da instrucciones con nombre y tipo; no puede hacer que el
 * compilador entienda una forma de sentencia nueva, ni fijar un valor a un
 * registro concreto entre dos lineas, ni asignar registros a mano. Eso es
 * trabajo de compilador, y decirlo aqui es mas honesto que descubrirlo a mitad
 * de un driver.
 */
#ifndef SEMANTIC_H
#define SEMANTIC_H

#include <semantic/tipos.h>
#include <semantic/puertos.h>
#include <semantic/barreras.h>
#include <semantic/cpu.h>
#include <semantic/memoria.h>
#include <semantic/atomico.h>
#include <semantic/bits.h>

#endif /* SEMANTIC_H */
