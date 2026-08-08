/* semantic/atomico.h -- operaciones que no se pueden partir por la mitad.
 *
 * == Por que hacen falta ANTES de que haya dos nucleos ==
 *
 * Hoy BMO corre en un nucleo, y en un solo nucleo un `contador = contador + 1`
 * parece seguro. No lo es: **una interrupcion cae entre la lectura y la
 * escritura**. El temporizador entra mil veces por segundo, y si su manejador
 * toca el mismo contador, la suma se pierde.
 *
 * Y el dia que arranque el segundo nucleo --el codigo de SMP ya esta escrito,
 * solo que nadie lo llama-- cada `static mut` del kernel pasa a ser una carrera
 * de golpe. Escribir esto ahora es mas barato que auditarlo despues.
 *
 * == El prefijo LOCK va DENTRO de los bytes ==
 *
 * Cada una de estas lleva `F0` en su fila de `intrinsics.toml`. No es un
 * detalle de codificacion: sin el la instruccion hace lo mismo pero **no es
 * atomica**, y el fallo aparece una vez cada mil arranques, en el sitio
 * equivocado, y no se reproduce.
 *
 * `xchg` sobre memoria es la excepcion: el manual dice que el prefijo se asume,
 * asi que es la unica sin `F0` -- y no por descuido.
 *
 * == Lo que NO hay aqui ==
 *
 * Un cerrojo. `atomico_xchg` es la instruccion con la que se construye uno;
 * donde se guarda, quien lo tiene y que pasa si el dueno muere son decisiones
 * de politica, y este fichero es de instrucciones.
 */
#ifndef SEMANTIC_ATOMICO_H
#define SEMANTIC_ATOMICO_H

#include <semantic/tipos.h>

/* Pone `valor` en `[dir]` y devuelve lo que habia, sin que nadie se cuele.
 *
 * El cerrojo mas simple del mundo se hace con esto: intercambiar 1 y mirar lo
 * que habia. Si habia 0, es tuyo; si habia 1, lo tiene otro. */
u64 atomico_xchg(u64 *dir, u64 valor) { return __xchg(dir, valor); }

/* Compara-e-intercambia. Devuelve **lo que HABIA**.
 *
 * Si el retorno es igual a `esperado`, el cambio ocurrio. Si no, otro llego
 * antes y el valor de vuelta es el suyo -- se reintenta con ese.
 *
 * Es la base de todo lo que no usa cerrojos, y la forma correcta de sumar a un
 * campo compartido cuando la suma depende del valor:
 *
 *     for (;;) {
 *         u64 v = *contador;
 *         if (atomico_cas(contador, v, v + 1) == v) break;
 *     }
 *
 * * Devolver el valor y no un si/no es a proposito: con un booleano habria que
 *   releer para reintentar, y entre el fallo y la relectura cabe otro cambio. */
u64 atomico_cas(u64 *dir, u64 esperado, u64 nuevo) {
    return __cmpxchg(dir, esperado, nuevo);
}

/* Suma y devuelve lo ANTERIOR. Un contador que reparte numeros --pids, tickets--
 * se hace con esto y nadie recibe el mismo dos veces. */
u64 atomico_sumar_y_devolver(u64 *dir, u64 cuanto) { return __xadd(dir, cuanto); }

/* Suma sin mirar el resultado. Mas corta que `xadd` cuando el valor no importa.
 */
void atomico_sumar(u64 *dir, u64 cuanto) { __lock_add(dir, cuanto); }

/* Enciende bits sin perder los que otro encienda a la vez. Un `|=` normal lee,
 * modifica y escribe: lo que el otro encendio entre medias desaparece. */
void atomico_encender(u64 *dir, u64 mascara) { __lock_or(dir, mascara); }

/* Apaga bits, misma razon. La mascara va con los bits a CONSERVAR a uno. */
void atomico_apagar(u64 *dir, u64 mascara) { __lock_and(dir, mascara); }

#endif /* SEMANTIC_ATOMICO_H */
