/* prestado.h -- MEMORIA QUE VIAJA SIN COPIARSE, las dos direcciones.
 *
 * == El hueco que tapa, y por que era el mas gordo ==
 *
 * BMO-X es de copia cero, y REX publicaba **la mitad**:
 *
 *     MEM_OP_OFRECER     estaba, dentro de <bmo/superficie.h>   PRESTAR
 *     TASK_OP_TOMAR      NO estaba                              TOMAR
 *     PRESTADO_OP_*      NO estaban                             medirlo y soltarlo
 *
 * O sea que **una app de C podia prestar memoria y no podia recibirla**. La
 * podia prestar, ademas, solo de una forma: como superficie, y solo a quien la
 * lanzo. El camino de vuelta --recibir un bloque de otro proceso sin copiarlo--
 * no tenia cabecera ninguna.
 *
 * Salio contando la cobertura (paso 4 de `docs/plan/PLAN_REX.md`), no
 * buscandolo: es lo que pasa cuando el hueco es un numero en vez de una
 * sensacion.
 *
 * == Que es un prestamo, en una linea ==
 *
 * Un proceso ofrece un trozo de SU bloque a un TID concreto; ese, y solo ese,
 * lo toma. **El mapeo ocurre dentro de la llamada de quien toma**, en su propio
 * espacio -- por eso se TOMA y no se coloca: nadie escribe en las tablas de
 * paginas de otro que no esta corriendo.
 *
 * Y no se copia ni un byte. Las mismas paginas fisicas quedan en los dos
 * espacios; lo que cambia es quien puede verlas.
 *
 * == Como se usa: el que presta ==
 *
 *     char *caja = (char *)malloc(64 * 1024);
 *     bmo_prestar_del_monton(caja, 64 * 1024, tid_del_otro);
 *
 * == Y el que recibe ==
 *
 *     BMO_PRESTADO p;
 *     if (bmo_prestado_tomar(&p)) {
 *         p.bytes_ptr[0] = 7;
 *         bmo_prestado_soltar(&p);
 *     }
 *
 * == ** LO QUE HAY QUE PREGUNTAR CADA VEZ: si el dueno sigue vivo ==
 *
 * `bmo_prestado_dueno` **cruza la puerta cada llamada, a proposito**. Es un
 * estado que vive en el kernel y que cambia sin avisar: el proceso que presto
 * puede morirse en cualquier momento, y entonces contesta `0`.
 *
 * Guardarlo en el struct y leerlo de ahi seria un ESPEJO --el error que
 * `<bmo/archivo/amarilla.h>` ya cuenta que sale mal solo-- y aqui el espejo
 * miente en el caso exacto que importa: **componer la memoria de un muerto**.
 * Sin poder preguntar, una app muerta y una app pensando son la misma cosa.
 *
 * == Lo que esta cabecera NO hace ==
 *
 * - **No avisa.** No hay senal cuando el dueno muere: se pregunta.
 * - **No busca.** `TOMAR` coge lo que haya para ti; no se elige entre varios.
 * - **No presta a cualquiera.** El destinatario es un TID concreto y solo el
 *   puede tomarlo. Ofrecer al aire no existe, y esta bien que no exista.
 */
#ifndef BMO_PRESTADO_H
#define BMO_PRESTADO_H

#include <bmo/bmo.h>
/* Los dos numeros con los que se nombra el bloque propio: `OFRECER` presta
 * `base + desde`, y sin ellos no hay ni base ni desde. */
#include <bmo/bloque.h>
/* ** Y EL MONTON, que es una dependencia de verdad y conviene decirla.
 *
 * La necesita `bmo_prestar_del_monton` para comprobar que el puntero cae
 * DENTRO de la arena antes de restar. Es coherente --prestar memoria significa
 * que tenias memoria, o sea que ya llamaste a `malloc`-- pero tiene un precio
 * que en REX no se puede esquivar:
 *
 * [!] **Una cabecera se paga por INCLUIRLA, no por usarla** (ver
 * `<bmo/pantalla.h>`). Un programa que SOLO toma prestado no llama a `malloc`
 * ni una vez y aun asi se lleva el asignador dentro del `.bex`. Se dice aqui en
 * vez de descubrirlo midiendo. */
#include <stdlib.h>

/* -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
 *
 * Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
 * toco. La ley esta en `META-KERNEL_HARD.md`; el juez, en
 * `toolchain/tools/contrato/contrato.py`.
 *
 * [carril]  ROJO         mete las paginas de OTRO proceso en tu espacio y te
 *                        da un puntero a ellas. Un `desde` o un `bytes` mal
 *                        calculados prestan lo que no era, y quien lo toma no
 *                        tiene forma de saberlo
 * [cuesta]  DATO         lo prestado es el trabajo de alguien: los pixeles de
 *                        una ventana, un buffer de audio, un fichero a medio
 *                        leer. Prestar de mas lo ensena; soltar mal lo pierde
 * [riesgo]  AJENO SILENCIO
 *                        AJENO: los bytes los escribe el otro proceso, que
 *                        puede morirse mientras los lees. SILENCIO: `TOMAR`
 *                        contesta 0 cuando no hay nada, igual que cuando la
 *                        ranura se agoto -- y quien no mire escribira en 0
 */

/* Ofrecer un trozo del bloque propio. Operacion sobre `KIND_MEMORIA`.
 *
 * [!] Tambien lo define `<bmo/superficie.h>`, que fue el primero que lo
 * necesito. Se guarda con `#ifndef` en vez de mudarlo: mover un numero de sitio
 * no cambia nada para quien compila y **rompe a quien lea la cabecera vieja**,
 * y las dos definiciones dicen lo mismo -- R13 lo comprueba. */
#ifndef BMO_MEM_OFRECER
#define BMO_MEM_OFRECER 0x03
#endif

/* Tomar lo que otro haya ofrecido. Operacion de TAREA: no hace falta handle,
 * porque lo que se pregunta es "hay algo para MI?". */
#define BMO_OP_TOMAR 0x1C

/* ** QUIEN ME LANZO, y esto es parte de PRESTAR aunque no lo parezca.
 *
 * Un prestamo va a un TID concreto -- ofrecer al aire no existe. Asi que sin
 * una forma de nombrar al destinatario, la mitad de prestar no se puede
 * escribir, y el unico TID que un programa de C puede conseguir HOY es el de su
 * padre.
 *
 * [!] Y ahi hay un hueco que conviene ver: `TAREA_OP_TID` existe en el ABI y
 * **no esta en REX**, asi que a un HIJO que uno mismo lanza no se le puede
 * prestar nada -- no por falta de prestamo, sino por no saber su nombre. La
 * punta corta del zero copy no es tomar: es a quien.
 *
 * Se guarda con `#ifndef` por lo mismo que `MEM_OFRECER`: `<bmo/superficie.h>`
 * ya lo traia, y las dos definiciones dicen lo mismo -- R13 lo comprueba. */
#ifndef BMO_OP_MI_PADRE
#define BMO_OP_MI_PADRE 0x26
#endif

/* -- Operaciones sobre el handle `KIND_PRESTADO` ----------------------- */
/* Donde quedo, en MI espacio. */
#define BMO_PRESTADO_BASE   0x01
/* Cuantos bytes son. */
#define BMO_PRESTADO_BYTES  0x02
/* El TID de quien lo presto, o 0 si ya no vive. Ver la nota de arriba. */
#define BMO_PRESTADO_DUENO  0x03
/* Devolverlo: se desmapea, y la ranura queda libre para el siguiente. */
#define BMO_PRESTADO_SOLTAR 0x04

/* Un prestamo recibido.
 *
 * ** `dueno` NO esta en el struct, y es la decision de diseno de este fichero.
 * Los otros tres campos son fijos mientras el prestamo viva; ese cambia solo. */
struct BMO_PRESTADO {
    unsigned long long cap;        /* el handle; 0 = no tienes nada */
    unsigned char *bytes_ptr;      /* donde quedo, ya en TU espacio */
    unsigned long long bytes;      /* cuanto */
};
typedef struct BMO_PRESTADO BMO_PRESTADO;

/* -- El que PRESTA ----------------------------------------------------- */

/* El TID de quien me lanzo, o `0` si nadie (el shell de Ring 0 no compone).
 *
 * Es el destinatario del caso normal: una app le presta a quien la puso en
 * pantalla. Que devuelva `0` no es un fallo -- es haberse lanzado a mano. */
unsigned long long bmo_mi_padre() {
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_MI_PADRE, 0, 0, 0);
}

/* Ofrece `bytes` del bloque propio, a partir de `desde`, al TID `destino`.
 *
 * `desde` va contra la base del BLOQUE, que es lo unico que el kernel conoce --
 * la misma resta que hace `fread`. Si tienes un puntero y no un desplazamiento,
 * usa `bmo_prestar_del_monton`, que la hace por ti.
 *
 * `0` = ofrecido. */
unsigned long long bmo_prestar(unsigned long long bloque,
                               unsigned long long desde,
                               unsigned long long bytes,
                               unsigned long long destino) {
    return bmo_codigo(bloque, BMO_MEM_OFRECER, desde, bytes, destino);
}

/* Lo mismo, con un puntero que salio de `malloc`. `0` = ofrecido.
 *
 * ** Comprueba que el puntero es del monton ANTES de restar. Sin eso, un
 * puntero de la pila da un `desde` enorme --o negativo dado la vuelta-- y lo
 * que se ofrece es un trozo del bloque que no tiene nada que ver. El kernel lo
 * rechazaria por rango, pero *"el kernel lo rechaza"* no es una respuesta que
 * quien llama pueda leer: esto contesta antes y con un numero suyo. */
unsigned long long bmo_prestar_del_monton(void *p, unsigned long long bytes,
                                          unsigned long long destino) {
    unsigned long long d;

    if (p == 0 || bytes == 0) {
        return 1;
    }
    d = (unsigned long long)p;
    if (d < __bmo_bloque_base || d < __bmo_monton_ini || d >= __bmo_monton_fin) {
        return 1;
    }
    return bmo_prestar(__bmo_bloque_cap, d - __bmo_bloque_base, bytes, destino);
}

/* -- El que RECIBE ----------------------------------------------------- */

/* Toma lo que haya para ti. `1` = lo tienes, `0` = no hay nada.
 *
 * El struct se deja a cero cuando contesta `0`: un programa que no mire el
 * retorno se lleva un puntero nulo --que falla donde se usa-- en vez de basura
 * de la pila que parece una direccion buena. */
int bmo_prestado_tomar(BMO_PRESTADO *p) {
    if (p == 0) {
        return 0;
    }
    p->cap = 0;
    p->bytes_ptr = 0;
    p->bytes = 0;

    p->cap = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_TOMAR, 0, 0, 0);
    if (p->cap == 0) {
        return 0;
    }
    p->bytes_ptr = (unsigned char *)bmo_valor(p->cap, BMO_PRESTADO_BASE, 0, 0, 0);
    p->bytes = bmo_valor(p->cap, BMO_PRESTADO_BYTES, 0, 0, 0);
    return 1;
}

/* El TID de quien te lo presto, o `0` si ya no vive.
 *
 * ** CRUZA LA PUERTA CADA VEZ, y no se guarda. Ver la cabecera: es el unico
 * dato del prestamo que cambia sin avisar, y cachearlo seria no distinguir una
 * app muerta de una app pensando -- que es justo para lo que existe. */
int bmo_prestado_dueno(BMO_PRESTADO *p) {
    if (p == 0 || p->cap == 0) {
        return 0;
    }
    return (int)bmo_valor(p->cap, BMO_PRESTADO_DUENO, 0, 0, 0);
}

/* Sigue vivo el que presto? `1` / `0`. Es `bmo_prestado_dueno` con nombre de
 * pregunta, porque el 90% de las veces lo que se quiere saber es eso. */
int bmo_prestado_vive(BMO_PRESTADO *p) {
    if (bmo_prestado_dueno(p) != 0) {
        return 1;
    }
    return 0;
}

/* Devolverlo. `1` = devuelto.
 *
 * ** Y HAY QUE DEVOLVERLO. Las ranuras de prestamo son contadas: abrir y cerrar
 * ventanas sin soltar las agota, y a partir de ahi ninguna app vuelve a recibir
 * nada hasta reiniciar. Es el mismo fallo que la pantalla tuvo hasta el
 * 2026-09-01 -- la unica forma de soltar era morirse.
 *
 * El puntero se pone a cero a proposito: esas paginas ya no son tuyas, y un
 * puntero que sigue apuntando ahi es una trampa que salta mucho despues. */
int bmo_prestado_soltar(BMO_PRESTADO *p) {
    unsigned long long r;

    if (p == 0 || p->cap == 0) {
        return 0;
    }
    r = bmo_codigo(p->cap, BMO_PRESTADO_SOLTAR, 0, 0, 0);
    p->cap = 0;
    p->bytes_ptr = 0;
    p->bytes = 0;
    if (r == 0) {
        return 1;
    }
    return 0;
}

/* Cabe este byte en lo que se presto?
 *
 * Existe por la misma razon que `bmo_pantalla_cabe`: la comprobacion de indice
 * escrita UNA vez y en el sitio donde se sabe el tamano de verdad. Aqui ademas
 * el tamano lo eligio OTRO proceso, asi que suponerlo es peor todavia. */
int bmo_prestado_cabe(BMO_PRESTADO *p, unsigned long long i) {
    if (p == 0 || p->bytes_ptr == 0) {
        return 0;
    }
    if (i >= p->bytes) {
        return 0;
    }
    return 1;
}

#endif /* BMO_PRESTADO_H */
