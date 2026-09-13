/* orquesta.h -- PEDIR UNA PARTE A LA ORQUESTA: varios nucleos, una faena.
 *
 * == Que es, en una linea ==
 *
 * Tu programa NOMBRA una parte del catalogo (`bmo-orquesta`), deja escritos sus
 * numeros en el atril y dice *tocad*. El kernel reparte la faena entre los
 * nucleos que esten en pie y **vuelve cuando esta hecha**.
 *
 * == ** Lo que NO es, y es la regla entera ==
 *
 * No es "ejecuta mi funcion en otro nucleo". Un obrero ejecuta PARTES DEL
 * KERNEL, escritas y probadas, nunca codigo de tu programa: un puntero a
 * funcion de Ring 3 corriendo con privilegio de kernel es una escalada con
 * otro nombre. Por eso aqui hay un catalogo cerrado y no un `spawn`.
 *
 * == Como se usa ==
 *
 *     int n;
 *     n = bmo_orquesta_escalar(dst, 1918, 1011, lienzo, 360, 360);
 *     if (n == 0) {
 *         // no se pudo: hazlo tu. El motivo esta en CABINA.
 *     }
 *
 * == ** Lo que la puerta comprueba, y por eso es segura ==
 *
 * - **Solo tu memoria.** Destino y origen tienen que caer DENTRO de un bloque
 *   de tu proceso: el de `malloc` lo es. Una direccion ajena no se rechaza por
 *   prohibida: es que la funcion que traduce no sabe nombrarla.
 * - **El tamano entero.** Si el rango se sale del bloque, no se recorta: se
 *   rechaza.
 * - **Cuantos nucleos lo decide la maquina.** Pides `0` y el perfil elige; con
 *   un solo nucleo en pie, lo hace ese y contesta `1`.
 *
 * [!] Y BLOQUEA mientras dura. Es a proposito: tu espacio no puede cambiar
 * debajo de doce obreros escribiendo en el.
 */
#ifndef BMO_ORQUESTA_H
#define BMO_ORQUESTA_H

#include <bmo/bmo.h>

/* -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
 *
 * Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
 * toco. La ley esta en `META-KERNEL_HARD.md`; el juez, en
 * `toolchain/tools/contrato/contrato.py`.
 *
 * [carril]  ROJO         pone a varios nucleos a escribir en TU memoria a la
 *                        vez. Un destino o un tamano mal dichos no fallan:
 *                        escriben donde dijiste
 * [cuesta]  DATO         lo que se escribe es tu imagen o tu buffer; un encargo
 *                        mal armado es un fotograma con basura
 * [riesgo]  ESPEJO SILENCIO
 *                        ESPEJO: los numeros de parte viven en `bmo-orquesta` y
 *                        se repiten aqui (la prueba `los_numeros_son_los_de_rex`
 *                        los ata). SILENCIO: `TOCAR` contesta 0 al rechazar, y
 *                        el motivo va a CABINA, no aqui
 */

/* Las dos operaciones de TAREA. */
#define BMO_OP_ATRIL 0x31
#define BMO_OP_TOCAR 0x32

/* Los campos del atril. */
#define BMO_ATRIL_DESTINO 0
#define BMO_ATRIL_ORIGEN  1
#define BMO_ATRIL_TOTAL   2
#define BMO_ATRIL_DATO    3

/* El catalogo. El numero es contrato: no se renumera nunca. */
#define BMO_PARTE_LLENAR   1
#define BMO_PARTE_EXPANDIR 2
#define BMO_PARTE_ESCALAR  3

/* Poner un numero en el atril. 1 si entro. */
int bmo_atril(unsigned long long campo, unsigned long long valor) {
    return (int)bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_ATRIL, campo, valor, 0);
}

/* TOCAD. Cuantos atriles tocaron, o 0 si el encargo no paso. `atriles = 0`
 * deja que decida la maquina, que es lo normal. */
int bmo_tocar(unsigned long long parte, unsigned long long atriles) {
    return (int)bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_TOCAR, parte, atriles, 0);
}

/* ** ESCALAR: `origen` (src_ancho x src_alto) llevado ENTERO a `destino`
 * (dst_ancho x dst_alto), al factor entero que cabe, centrado y con negro
 * alrededor. Es la geometria de `bmo_orquesta::Escala`.
 *
 * Contesta cuantos atriles tocaron, o 0 si no se hizo -- y entonces NADA se
 * escribio y te toca hacerlo a ti. Un destino mas pequeno que el origen se
 * rechaza aqui mismo, sin cruzar la puerta: escalar hacia abajo es otra parte.
 *
 * El alto del destino va en TOTAL --son las filas que se reparten-- y los otros
 * tres tamanos se empaquetan en DATO, 16 bits cada uno. */
int bmo_orquesta_escalar(unsigned int *destino, int dst_ancho, int dst_alto,
                         unsigned int *origen, int src_ancho, int src_alto) {
    unsigned long long dato;
    if (src_ancho <= 0 || src_alto <= 0) return 0;
    if (dst_ancho < src_ancho || dst_alto < src_alto) return 0;
    if (dst_ancho > 65535 || dst_alto > 65535) return 0;
    dato = (unsigned long long)src_ancho;
    dato = dato | ((unsigned long long)src_alto << 16);
    dato = dato | ((unsigned long long)dst_ancho << 32);
    bmo_atril(BMO_ATRIL_DESTINO, (unsigned long long)destino);
    bmo_atril(BMO_ATRIL_ORIGEN, (unsigned long long)origen);
    bmo_atril(BMO_ATRIL_TOTAL, (unsigned long long)dst_alto);
    bmo_atril(BMO_ATRIL_DATO, dato);
    return bmo_tocar(BMO_PARTE_ESCALAR, 0);
}

#endif /* BMO_ORQUESTA_H */
