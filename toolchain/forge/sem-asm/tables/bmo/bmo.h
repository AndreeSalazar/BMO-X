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
 */
#ifndef BMO_BMO_H
#define BMO_BMO_H

/* -- Los DOS numeros de llamada, y el que quedo reservado --------------- */
#define BMO_INVOKE 0
/* ** RETIRADO el 2026-08-10 y RESERVADO: ya no hay una llamada en el 1.
 * Avisar al consumidor de un canal es ahora una operacion sobre el canal
 * (`CHANNEL_OP_KICK`, 0x03) y entra por `INVOKE`, como todo lo demas.
 * El numero no se recicla: un binario viejo que lo llame falla diciendolo. */
#define BMO_CHANNEL_KICK 1
#define BMO_WAIT 2

/* Pseudo-capability que se refiere al proceso que llama.
 *
 * No es un handle concedido: es la forma de pedir lo que uno ya tiene por ser
 * quien es. No otorga autoridad sobre nadie mas y nunca debe transferirse.
 *
 * * Este literal es la razon por la que el lexer de BMO C tuvo que aprender a
 *   leer hexadecimales de 64 bits: no cabe en un `long long` con signo, y
 *   antes se convertia en CERO en silencio -- o sea, en la capability 0. */
#define BMO_TAREA_ACTUAL 0xFFFFFFFFFFFFFFFE

/* -- Operaciones sobre BMO_TAREA_ACTUAL -------------------------------- */
#define BMO_OP_PID 0x01
#define BMO_OP_TID 0x02
#define BMO_OP_CEDER 0x03
#define BMO_OP_SALIR 0x04
#define BMO_OP_CONSOLA_ESCRIBIR 0x06
#define BMO_OP_PANTALLA_RECLAMAR 0x09
#define BMO_OP_ENTRADA_RECLAMAR 0x0A
#define BMO_OP_RUTA 0x0B
#define BMO_OP_EJECUTAR 0x0C
#define BMO_OP_CONSOLA_LEER 0x0F
#define BMO_OP_ARCHIVO_ABRIR 0x10
#define BMO_OP_ARCHIVO_CREAR 0x11
#define BMO_OP_INFO 0x13
/* El SONIDO. Exclusivo como la pantalla; ver <bmo/sonido.h>. */
#define BMO_OP_SONIDO_RECLAMAR 0x21
#define BMO_OP_SONIDO_SOLTAR 0x22

/* Campos de BMO_OP_INFO. Son una TABLA: anadir un dato es una fila, no una
 * operacion nueva. */
#define BMO_INFO_RAM_TOTAL 0x01
#define BMO_INFO_RAM_LIBRE 0x02
#define BMO_INFO_TSC_HZ 0x05
/* -- LA RED, para un programa de C ---------------------------------
 *
 * ** Mirar la red no necesita capability: son campos de INFORME, igual que la
 * RAM o los nucleos. Preguntar si hay cable no es un privilegio.
 * TRANSMITIR si la necesitara, y ese dia sera una operacion sobre un handle. */
#define BMO_INFO_NET_PRESENTE       0x27
#define BMO_INFO_NET_VENDOR_DEVICE  0x28
/* Los seis bytes en los 48 bits bajos, byte 0 el mas significativo. */
#define BMO_INFO_NET_MAC            0x29
/* El `PHYstatus` CRUDO. El byte entero es la prueba; los otros campos son la
 * opinion. */
#define BMO_INFO_NET_PHY_CRUDO      0x2A
/* 10, 100, 1000 -- o 0, que significa "no hay cable" y es una respuesta. */
#define BMO_INFO_NET_MEGABITS       0x2B
/* Distingue "no llega nada" de "no estamos escuchando". */
#define BMO_INFO_NET_RX_ARMADO      0x2C
#define BMO_INFO_NET_RX_TRAMAS      0x2D
#define BMO_INFO_NET_PCI            0x2E

#define BMO_INFO_CPU_HILOS 0x06
#define BMO_INFO_CPU_NUCLEOS 0x07
#define BMO_INFO_TICKS 0x0B

/* -- La puerta --------------------------------------------------------- */

/* El VALOR que devuelve una operacion.
 *
 * La puerta contesta dos cosas: `rax` lleva el codigo y las banderas, `rdx`
 * lleva el valor. En C un par no cabe en un registro de retorno, asi que hay
 * dos funciones y cada una recoge una mitad. Esto no es una limitacion que se
 * pueda tapar: es la forma real de la llamada, y taparla obligaria a inventar
 * una struct que el codegen tendria que devolver por memoria. */
unsigned long long bmo_valor(unsigned long long cap, unsigned long long op,
                             unsigned long long a0, unsigned long long a1,
                             unsigned long long a2) {
    return __syscall_valor(BMO_INVOKE, cap, op, a0, a1, a2);
}

/* El CODIGO de la misma operacion. `0` es lo unico que significa exito.
 *
 * Los 32 bits altos llevan las banderas del kernel -- por ejemplo la que
 * distingue "no tienes permiso" de "ese handle no existe". */
unsigned long long bmo_codigo(unsigned long long cap, unsigned long long op,
                              unsigned long long a0, unsigned long long a1,
                              unsigned long long a2) {
    return __syscall(BMO_INVOKE, cap, op, a0, a1, a2);
}

/* -- Lo que uno tiene por ser quien es --------------------------------- */

unsigned long long bmo_pid() {
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_PID, 0, 0, 0);
}

/* Ceder el turno.
 *
 * Un bucle de espera en Ring 3 que no cede se come el quantum entero sin
 * avanzar nada -- y como aqui casi todas las lecturas son NO BLOQUEANTES
 * (`bmo_entrada_tecla`, `bmo_entrada_rueda`), el bucle de espera es la forma
 * normal de esperar. Sin este `ceder` el sistema entero va a tirones. */
void bmo_ceder() {
    bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_CEDER, 0, 0, 0);
}

/* Un dato numerico del sistema. `0` si el kernel no sabe contestar ese campo. */
unsigned long long bmo_info(unsigned long long campo) {
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_INFO, campo, 0, 0);
}

/* Terminar. No vuelve: el kernel revoca las capabilities del proceso y cambia
 * de contexto en el propio borde del syscall.
 *
 * `main` ya termina asi sola; esto es para salir desde dentro de un bucle. */
void bmo_salir() {
    bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_SALIR, 0, 0, 0);
}

#endif /* BMO_BMO_H */
