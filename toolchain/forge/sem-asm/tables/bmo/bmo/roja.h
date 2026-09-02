/* bmo/roja.h -- las DOS PUERTAS y sus numeros -- lo que no se puede mover
 *
 * Un CARRIL de `<bmo/bmo.h>` (L6g). La cabecera entera --que
 * explica por que existe esta pieza-- esta en la fachada; aqui va lo
 * que cambia de color.
 *
 * [carril]  ROJO         un numero de esta mitad esta en TODOS los `.bex` que
 *                        existen. Cambiarlo no rompe una compilacion: rompe
 *                        binarios ya firmados que nadie va a recompilar. Y los
 *                        cuerpos bajan a la instruccion, no a una llamada
 * [cuesta]  PUERTA       rompe binarios que YA existen. El precedente esta en
 *                        la propia ley: `KIND_TAREA = 0x80`
 * [riesgo]  UNICO        un numero de syscall se congela una vez y no se
 *                        deshace. Por eso el 1 sigue RESERVADO desde que se
 *                        retiro `CHANNEL_KICK`: reciclarlo haria que un
 *                        binario viejo llamara a otra cosa en silencio
 */
#ifndef BMO_BMO_ROJA_H
#define BMO_BMO_ROJA_H

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
/* Reiniciar la maquina. Desde el 25-08 pide AUTORIDAD, y solo la tiene quien
   arranco desde Ring 0. Ver task/autoridad.rs.                            */
#define BMO_OP_REINICIAR 0x12
#define BMO_OP_CONSOLA_LEER 0x0F
#define BMO_OP_ARCHIVO_ABRIR 0x10
#define BMO_OP_ARCHIVO_CREAR 0x11
#define BMO_OP_INFO 0x13
/* El SONIDO. Exclusivo como la pantalla; ver <bmo/sonido.h>. */
#define BMO_OP_SONIDO_RECLAMAR 0x21
#define BMO_OP_SONIDO_SOLTAR 0x22

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

/* ** DORMIR de verdad, en nanosegundos. La SEGUNDA puerta.
 *
 * `bmo_ceder()` no espera: suelta el turno y vuelve a la cola LISTO, asi que un
 * bucle que solo cede sigue comiendose un turno entero cada vuelta. Para un
 * programa que toma la pantalla eso da igual --no hay nadie mas-- pero para uno
 * EN UNA VENTANA es lo contrario de lo que hace falta: el DIRECTOR tiene que
 * componer sus pixeles, y compite con el por el mismo nucleo.
 *
 * ** El sintoma que esto arregla, y se vio en el Ryzen el 2026-08-19: el
 * raycaster en ventana dejaba el escritorio tan lento que parecia que el
 * teclado no respondia. No lo tenia secuestrado --en ventana no reclama la
 * entrada-- simplemente no quedaba turno para pintar la tecla.
 *
 * `WAIT` con `esperable = 0` es una espera pura por tiempo: la tarea se marca
 * BLOQUEADA y no vuelve a la cola hasta el plazo, asi que el turno es de otro
 * de verdad y no un rato prestado.
 *
 * A 60 fotogramas por segundo, 16.000.000 ns. */
void bmo_dormir(unsigned long long nanos) {
    __syscall(BMO_WAIT, 0, 0, nanos, 0, 0);
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

#endif /* BMO_BMO_ROJA_H */
