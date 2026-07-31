/* semantic/barreras.h — decirle al CPU que NO reordene.
 *
 * ══ Por qué esto existe ══
 *
 * Un CPU moderno ejecuta fuera de orden y escribe a memoria cuando le conviene.
 * Para un programa solo eso es invisible: el resultado es el mismo. Deja de ser
 * invisible en dos sitios, y los dos están en BMO:
 *
 *   1. **MMIO.** Escribir un registro de un controlador es una ORDEN, no un
 *      dato. Si el CPU adelanta la escritura del timbre a la del descriptor, el
 *      aparato lee un descriptor a medio construir. No falla siempre: falla
 *      cuando el reordenado ocurre, o sea a veces.
 *
 *   2. **Otro núcleo.** Lo que un núcleo escribe puede verse en otro orden
 *      desde otro núcleo. Hoy BMO corre en uno; el día que arranque el segundo,
 *      cada uno de estos sitios es una carrera.
 *
 * ══ Cuál usar ══
 *
 * En x86 el modelo de memoria ya es fuerte (TSO): las lecturas no se adelantan
 * a otras lecturas ni las escrituras a otras escrituras. Lo único que SÍ se
 * reordena es una lectura adelantándose a una escritura anterior. Por eso:
 *
 *   - Entre dos escrituras a MMIO seguidas, casi nunca hace falta barrera.
 *   - Entre escribir un descriptor y tocar el timbre, `barrera_escrituras()`.
 *   - Antes de LEER un estado que depende de algo que acabas de escribir,
 *     `barrera_total()` — es la única que impide ese adelanto.
 *
 * Poner `barrera_total()` en todas partes funciona y cuesta; poner la que toca
 * exige saber cuál, y para eso está este comentario.
 */
#ifndef SEMANTIC_BARRERAS_H
#define SEMANTIC_BARRERAS_H

/* `mfence` — nada de antes cruza, ni lecturas ni escrituras. La cara. */
void barrera_total() { __mfence(); }

/* `sfence` — las escrituras de antes están hechas antes que las de después. */
void barrera_escrituras() { __sfence(); }

/* `lfence` — las lecturas de antes están hechas. También corta la especulación,
 * que es para lo que se usa contra Spectre v1. */
void barrera_lecturas() { __lfence(); }

/* `pause` — dentro de un bucle de espera activa.
 *
 * No es una barrera: es un AVISO. Le dice al CPU que esto es una espera, así
 * evita la penalización de salida del bucle y —en un núcleo con dos hilos—
 * deja respirar al hermano. Un spin sin `pause` se come el hilo vecino. */
void respira() { __pause(); }

#endif /* SEMANTIC_BARRERAS_H */
