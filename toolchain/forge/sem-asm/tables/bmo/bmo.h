/* bmo.h -- la superficie congelada de BMO, en C.
 *
 * == Por que esto es UNA cabecera y no un runtime ==
 *
 * En Linux o en Windows, `#include <unistd.h>` promete un `libc.so` que el
 * cargador resolvera mas tarde. Aqui no hay cargador que resuelva nada: no hay
 * enlazado dinamico, no hay libc, no hay un simbolo que alguien vaya a
 * rellenar. Asi que la cabecera **trae el cuerpo**, y lo que hay dentro del
 * cuerpo baja a la instruccion en la misma linea.
 *
 * Esa es la diferencia entera de BMO C/Control con el C de siempre: el ASM no
 * esta encapsulado en una caja negra `asm("...")` que el compilador copia sin
 * leer. `__syscall(...)` es una fila de la tabla
 * `forge/sem-asm/tables/arch/x86_64/intrinsics.toml`, con sus bytes exactos y
 * el registro de cada argumento escrito ahi. El compilador emite ESOS bytes.
 * Se lee como C, se comporta como ASM, y ninguna de las dos mitades esconde
 * nada de la otra.
 *
 * == La superficie ==
 *
 * DOS llamadas (2026-08-10; eran tres):
 *
 *     INVOKE(cap, operacion, a0, a1, a2)   haz esto AHORA
 *     WAIT(esperable, visto, timeout_ns)   despiertame CUANDO
 *
 * Todo lo demas --abrir un archivo, leer el raton, reclamar la pantalla-- es una
 * OPERACION sobre una capability. La API crece por dentro, en la pareja
 * (tipo de objeto, operacion), y el ABI no se toca.
 *
 * El tercero se fue porque no era una puerta: CHANNEL_KICK resolvia un handle y
 * avisaba a su consumidor, o sea una OPERACION con numero de syscall propio.
 * WAIT si se queda, y por algo que INVOKE no puede decir: lo unico que hace es
 * NO DEVOLVER EL TURNO. Una llamada sincrona tendria que contestar "todavia no"
 * y dejar que el programa vuelva a preguntar -- quemando el turno en preguntar,
 * que es justo lo que WAIT existe para no hacer.
 *
 * == Lo que un programa NO recibe ==
 *
 * No hay `argv`, no hay `environ`, no hay descriptores heredados. Un proceso
 * Ring 3 recibe *capabilities*, y lo que no le hayan dado no existe para el.
 * Por eso aqui no hay `open("/dev/input")`: hay `bmo_valor(BMO_TAREA_ACTUAL,
 * BMO_OP_ENTRADA_RECLAMAR, ...)`, que puede contestar que no.
 *
 * -- ** ESTE FICHERO SE PARTIO (L6g, nivel 3) -------------------------
 *
 * Tenia DOS MASAS con costes distintos, y la regla de L6e dice que eso
 * es un fichero mal cortado: *el corte va justo por donde cambia el
 * coste*. Cada mitad vive ahora en `bmo/bmo/` con su semaforo,
 * su `[cuesta]` y su `[riesgo]`.
 *
 * **Fuera no cambia nada.** `#include <bmo/bmo.h>` sigue trayendo
 * lo mismo que traia: esto es la fachada, igual que un `mod.rs` que
 * re-exporta. Incluir un carril suelto tambien vale.
 *
 *     roja.h     las DOS PUERTAS y sus numeros -- lo que no se puede mover
 *     verde.h    la tabla de INFO -- crece por filas, y por eso es verde
 *
 * [carril]  ROJO         el reparto, y hereda el color del carril que manda
 * [cuesta]  PUERTA       lo peor de sus dos mitades: cambiar que trae esta
 *                        fachada cambia lo que ve TODO binario que la incluya
 * [riesgo]  UNICO        lo que se congela aqui no se descongela: el numero 1
 *                        sigue reservado desde que se retiro `CHANNEL_KICK`
 *
 */
#ifndef BMO_BMO_H
#define BMO_BMO_H

#include <bmo/bmo/roja.h>
#include <bmo/bmo/verde.h>

#endif /* BMO_BMO_H */
