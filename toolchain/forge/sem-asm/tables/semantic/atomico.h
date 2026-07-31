/* semantic/atomico.h — operaciones que no se pueden partir por la mitad.
 *
 * ══ Por qué hacen falta ANTES de que haya dos núcleos ══
 *
 * Hoy BMO corre en un núcleo, y en un solo núcleo un `contador = contador + 1`
 * parece seguro. No lo es: **una interrupción cae entre la lectura y la
 * escritura**. El temporizador entra mil veces por segundo, y si su manejador
 * toca el mismo contador, la suma se pierde.
 *
 * Y el día que arranque el segundo núcleo —el código de SMP ya está escrito,
 * sólo que nadie lo llama— cada `static mut` del kernel pasa a ser una carrera
 * de golpe. Escribir esto ahora es más barato que auditarlo después.
 *
 * ══ El prefijo LOCK va DENTRO de los bytes ══
 *
 * Cada una de éstas lleva `F0` en su fila de `intrinsics.toml`. No es un
 * detalle de codificación: sin él la instrucción hace lo mismo pero **no es
 * atómica**, y el fallo aparece una vez cada mil arranques, en el sitio
 * equivocado, y no se reproduce.
 *
 * `xchg` sobre memoria es la excepción: el manual dice que el prefijo se asume,
 * así que es la única sin `F0` — y no por descuido.
 *
 * ══ Lo que NO hay aquí ══
 *
 * Un cerrojo. `atomico_xchg` es la instrucción con la que se construye uno;
 * dónde se guarda, quién lo tiene y qué pasa si el dueño muere son decisiones
 * de política, y este fichero es de instrucciones.
 */
#ifndef SEMANTIC_ATOMICO_H
#define SEMANTIC_ATOMICO_H

#include <semantic/tipos.h>

/* Pone `valor` en `[dir]` y devuelve lo que había, sin que nadie se cuele.
 *
 * El cerrojo más simple del mundo se hace con esto: intercambiar 1 y mirar lo
 * que había. Si había 0, es tuyo; si había 1, lo tiene otro. */
u64 atomico_xchg(u64 *dir, u64 valor) { return __xchg(dir, valor); }

/* Compara-e-intercambia. Devuelve **lo que HABÍA**.
 *
 * Si el retorno es igual a `esperado`, el cambio ocurrió. Si no, otro llegó
 * antes y el valor de vuelta es el suyo — se reintenta con ése.
 *
 * Es la base de todo lo que no usa cerrojos, y la forma correcta de sumar a un
 * campo compartido cuando la suma depende del valor:
 *
 *     for (;;) {
 *         u64 v = *contador;
 *         if (atomico_cas(contador, v, v + 1) == v) break;
 *     }
 *
 * ★ Devolver el valor y no un sí/no es a propósito: con un booleano habría que
 *   releer para reintentar, y entre el fallo y la relectura cabe otro cambio. */
u64 atomico_cas(u64 *dir, u64 esperado, u64 nuevo) {
    return __cmpxchg(dir, esperado, nuevo);
}

/* Suma y devuelve lo ANTERIOR. Un contador que reparte números —pids, tickets—
 * se hace con esto y nadie recibe el mismo dos veces. */
u64 atomico_sumar_y_devolver(u64 *dir, u64 cuanto) { return __xadd(dir, cuanto); }

/* Suma sin mirar el resultado. Más corta que `xadd` cuando el valor no importa.
 */
void atomico_sumar(u64 *dir, u64 cuanto) { __lock_add(dir, cuanto); }

/* Enciende bits sin perder los que otro encienda a la vez. Un `|=` normal lee,
 * modifica y escribe: lo que el otro encendió entre medias desaparece. */
void atomico_encender(u64 *dir, u64 mascara) { __lock_or(dir, mascara); }

/* Apaga bits, misma razón. La máscara va con los bits a CONSERVAR a uno. */
void atomico_apagar(u64 *dir, u64 mascara) { __lock_and(dir, mascara); }

#endif /* SEMANTIC_ATOMICO_H */
