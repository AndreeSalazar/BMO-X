/* pantalla.h -- LA PANTALLA ENTERA, en C.
 *
 * == Por que esta cabecera llega la ultima siendo el caso mas viejo ==
 *
 * REX tenia `<bmo/superficie.h>` para dibujar EN UNA VENTANA desde el 23-08, y
 * no tenia nada para tomar el panel entero -- que es lo que hacian los dos
 * programas mas antiguos del arbol. Las cuatro operaciones del framebuffer no
 * aparecian en `tables/` ni una sola vez, asi que cada uno se las inventaba:
 *
 *     doomgeneric_bmo.c:198   #define FB_BASE   0x01
 *     raycaster_C.c:75        #define FB_BASE   0x01
 *
 * ** Dos copias de un numero del kernel, en ficheros donde ningun guardian
 * mira. REX daba la llave --`PANTALLA_RECLAMAR`, que si estaba en
 * `<bmo/bmo.h>`-- y no daba la puerta.
 *
 * Se descubrio el 2026-09-01 cruzando las 337 constantes del ABI contra las de
 * REX: los `FB_OP_*` salieron en la lista de "esta en el contrato y no tiene
 * cabecera", y esa lista es el plan `docs/plan/PLAN_REX.md`.
 *
 * == Y SOLTARLA, que tampoco se podia ==
 *
 * `bmo_sonido_soltar` existia. `bmo_pantalla_soltar` no, **y la operacion del
 * kernel llevaba tiempo cableada**. O sea que desde C se tomaba la pantalla y
 * la unica forma de devolverla era morirse. Ver `<bmo/bmo.h>`, donde estan los
 * dos numeros al lado de sus gemelas.
 *
 * == Como se usa ==
 *
 *     BMO_PANTALLA p;
 *     if (bmo_pantalla_abrir(&p) == 0) {
 *         printf("la pantalla la tiene otro\n");
 *         return 1;
 *     }
 *     p.pixeles[y * p.paso + x] = 0x00FF8800;
 *     bmo_pantalla_cerrar(&p);
 *
 * [!] `paso` y NO `ancho` para indexar. Son distintos en cuanto el panel tiene
 * relleno al final de la fila, y usar `ancho` sale bien en el monitor donde
 * coinciden y torcido en el primero donde no.
 *
 * ** EL STRUCT LO PONE QUIEN LLAMA, y por eso va por `&p` en vez de devolverse
 * como hace `<bmo/superficie.h>`. Aquella tiene que pedir memoria de todas
 * formas --los pixeles son suyos-- y esta no: los pixeles ya estan mapeados por
 * el kernel. Devolver un puntero obligaria a un `malloc`, y con el a arrastrar
 * `<stdlib.h>` y el monton entero a un programa que a lo mejor no reserva nada.
 *
 * ** Lo que eso ahorra es una DEPENDENCIA, no bytes. Y la diferencia importa
 * porque los bytes no se ahorran de ninguna manera:
 *
 * [!] **En REX una cabecera se paga por INCLUIRLA, no por usarla.** Medido el
 * 2026-09-01: incluir esta y no llamar a nada son **1.795 bytes** en el `.bex`.
 * La cabecera trae el cuerpo --no hay `libbmo.so`-- y no hay enlazador detras
 * que pode lo que nadie llama, asi que *"incluye solo lo que necesitas"* aqui
 * es literal y no un consejo de estilo. Es una propiedad de REX entera, se
 * descubre midiendo, y hasta hoy no estaba escrita en ninguna parte.
 *
 * == Lo que esta cabecera NO hace ==
 *
 * - **No dibuja.** Da la direccion, el tamano y el paso; la linea y el
 *   rectangulo son de quien pinta.
 * - **No compone.** Si el DIRECTOR esta vivo, `reclamar` contesta 0 y hay que
 *   mirarlo: la pantalla tiene UN dueno. Un programa que quiera funcionar en
 *   los dos sitios prueba primero `<bmo/superficie.h>` y se cae aqui.
 * - **No convierte el formato.** Lo dice y ya; ver `formato`.
 */
#ifndef BMO_PANTALLA_H
#define BMO_PANTALLA_H

#include <bmo/bmo.h>

/* -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
 *
 * Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
 * toco. La ley esta en `META-KERNEL_HARD.md`; el juez, en
 * `toolchain/tools/contrato/contrato.py`.
 *
 * [carril]  ROJO         lo que sale de aqui es una direccion cruda y el
 *                        numero con el que se indexa. No hay red detras: el
 *                        kernel mapeo N bytes y nadie comprueba el indice
 * [cuesta]  TAREA        un `paso` o una `base` mal leidos escriben fuera del
 *                        mapeo, y eso es un #PF que mata la tarea. BMO sigue,
 *                        y el kernel recupera la pantalla del muerto
 * [riesgo]  AJENO SILENCIO
 *                        AJENO: los cuatro numeros los saco el kernel de la
 *                        tabla que dejo el firmware, no los calcula esto.
 *                        SILENCIO: `reclamar` contesta 0 cuando la tiene otro,
 *                        y quien no lo mira escribe en la direccion 0 -- un #PF
 *                        que no se parece en nada a "no te toca"
 */

/* -- Las cuatro operaciones sobre `KIND_FRAMEBUFFER` -------------------
 *
 * Los mismos numeros que `bmo_abi::syscalls::surface::FB_OP_*`. Se repiten aqui
 * porque C no puede importar de Rust, y **por eso el plan de REX tiene un paso
 * para que un juez compruebe que las dos listas dicen lo mismo**: dos copias de
 * un numero sin nadie que las compare es como empieza el fallo que solo aparece
 * en metal.
 *
 * Cada una devuelve UN valor, y los campos que van juntos viajan empaquetados
 * en vez de gastar una puerta por numero. */
#define BMO_FB_BASE   0x01
/* `(ancho << 32) | alto`, en pixeles. */
#define BMO_FB_DIMS   0x02
/* `(paso << 32) | formato`. ** El paso va en PIXELES, no en bytes: es el mismo
 * numero que usa el kernel, y convertirlo en la frontera seria inventar una
 * unidad distinta a cada lado. */
#define BMO_FB_STRIDE 0x03
/* Bytes mapeados en total, ya redondeados a pagina. Es el unico numero con el
 * que se puede saber si un indice cae dentro. */
#define BMO_FB_BYTES  0x04

/* -- El formato de pixel, tal como lo cuenta el firmware ---------------
 *
 * [!] Y con la honestidad que toca: **hoy todo lo que hay escrito supone 32
 * bits con el byte alto sin usar** --`0x00RRGGBB`-- y funciona en el Ryzen. El
 * campo se publica para que el dia que una placa conteste otra cosa se pueda
 * MIRAR en vez de adivinar, no porque ya haya un convertidor detras.
 *
 * El `0` es lo que contesta un firmware que no lo dijo. No significa "sin
 * pixeles": significa que nadie lo declaro. */
#define BMO_PANTALLA_FMT_SIN_DECIR 0
#define BMO_PANTALLA_FMT_BGR       1
#define BMO_PANTALLA_FMT_RGB       2

/* Todo lo que hace falta saber de un panel, leido de una vez.
 *
 * Son cuatro puertas, y se cruzan UNA vez al arrancar. Tener una funcion por
 * campo saldria a dos puertas por `ancho` y `alto` --que viven en el mismo
 * numero-- y eso es pagar dos veces por una respuesta. */
struct BMO_PANTALLA {
    unsigned long long cap;      /* el handle; 0 = no la tienes */
    unsigned int *pixeles;       /* donde empieza, ya en TU espacio */
    int ancho;
    int alto;
    int paso;                    /* EN PIXELES. Con relleno, != ancho */
    int formato;                 /* BMO_PANTALLA_FMT_* */
    unsigned long long bytes;    /* lo mapeado, para saber que cabe */
};
typedef struct BMO_PANTALLA BMO_PANTALLA;

/* Reclamar la pantalla. `0` si la tiene otro -- y hay que mirarlo.
 *
 * Es EXCLUSIVA: mientras un proceso la tenga, el kernel deja de dibujar y
 * ningun otro la consigue. No es un reparto, es una cesion. */
unsigned long long bmo_pantalla_reclamar() {
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_PANTALLA_RECLAMAR, 0, 0, 0);
}

/* Devolverla **siguiendo vivo**. `0` = hecho.
 *
 * El kernel desmapea las paginas del proceso antes de dar por buena la
 * devolucion: seguir vivo con la pantalla de otro mapeada seria poder escribir
 * en una pantalla que ya no es tuya. */
unsigned long long bmo_pantalla_soltar() {
    return bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_PANTALLA_SOLTAR, 0, 0, 0);
}

/* Reclamar y leer los cuatro numeros. `1` = la tienes, `0` = la tiene otro.
 *
 * El struct se deja a cero cuando contesta `0`, para que un programa que no
 * mire el retorno se lleve un puntero nulo --que falla donde se usa-- en vez de
 * basura de la pila que parece una direccion buena. */
int bmo_pantalla_abrir(BMO_PANTALLA *p) {
    unsigned long long dims;
    unsigned long long paso;

    if (p == 0) {
        return 0;
    }
    p->cap = 0;
    p->pixeles = 0;
    p->ancho = 0;
    p->alto = 0;
    p->paso = 0;
    p->formato = 0;
    p->bytes = 0;

    p->cap = bmo_pantalla_reclamar();
    if (p->cap == 0) {
        return 0;
    }
    p->pixeles = (unsigned int *)bmo_valor(p->cap, BMO_FB_BASE, 0, 0, 0);
    dims = bmo_valor(p->cap, BMO_FB_DIMS, 0, 0, 0);
    paso = bmo_valor(p->cap, BMO_FB_STRIDE, 0, 0, 0);
    p->bytes = bmo_valor(p->cap, BMO_FB_BYTES, 0, 0, 0);
    p->ancho = (int)(dims >> 32);
    p->alto = (int)(dims & 0xFFFFFFFF);
    p->paso = (int)(paso >> 32);
    p->formato = (int)(paso & 0xFFFFFFFF);
    return 1;
}

/* Soltarla y dejar el struct inservible. `1` = devuelta.
 *
 * Se pone `cap` y `pixeles` a cero A PROPOSITO: a partir de aqui esas paginas
 * ya no son del proceso, y un puntero que sigue apuntando ahi es una trampa
 * que salta mucho despues y lejos. */
int bmo_pantalla_cerrar(BMO_PANTALLA *p) {
    unsigned long long r;

    if (p == 0 || p->cap == 0) {
        return 0;
    }
    r = bmo_pantalla_soltar();
    p->cap = 0;
    p->pixeles = 0;
    if (r == 0) {
        return 1;
    }
    return 0;
}

/* -- ** CEDER LA PANTALLA UN RATO, Y VOLVER --------------------------
 *
 * == El problema, con numeros ==
 *
 * Una app a pantalla completa repinta sus pixeles decenas de veces por
 * segundo. DOOM a 28 fps con escala x5 reescribe 1600x1000 cada 35 ms, asi que
 * **cualquier cosa que el kernel escriba dentro de ese rectangulo vive 35
 * milisegundos**. No es que el kernel no pinte: es que pierde la carrera.
 *
 * Y esto no se arregla con el kernel reservandose una franja. Eso seria meter
 * politica de pantalla en Ring 0, que es exactamente lo que `KIND_LIENZO`
 * hacia y por lo que se quito -- ver `obj/loan.rs` y la pregunta del dueno:
 * *"Ring 3 no puede administrar eso el?"*.
 *
 * == Como lo resuelve Linux, que es de donde sale esto ==
 *
 * No reservando nada. El kernel da el framebuffer ENTERO al cliente grafico y
 * no se guarda un pixel; `printk` mientras corre X no se ve, y va al anillo
 * (`dmesg`) y al cable. Lo que hay es **una forma de irse a mirar y volver**:
 *
 *     Ctrl+Alt+F1     cambio de terminal virtual
 *     drmDropMaster   el cliente suelta el display
 *     drmSetMaster    y lo recupera al volver
 *
 * ** BMO-X ya tenia las tres piezas que importan --el klog, el cable y la
 * pantalla azul-- y le faltaba solo esta: **soltar sin morir**.
 *
 * == Como se usa ==
 *
 *     if (tecla_de_consola) {
 *         if (p.cap != 0) { bmo_pantalla_ceder(&p); }
 *         else            { bmo_pantalla_recuperar(&p); }
 *     }
 *
 * Mientras esta cedida, `p.pixeles` vale 0 y `p.cap` vale 0: la app **no puede
 * dibujar aunque lo intente**, y eso es a proposito. Un puntero que sigue
 * apuntando a una pantalla que ya no es tuya es la trampa que salta despues y
 * lejos.
 *
 * [!] Y ceder NO repinta nada. La pantalla se queda con el ultimo fotograma
 * hasta que alguien escriba: el panel del kernel pinta cuando hay algo que
 * decir, y `F11` vuelca el klog encima. Eso es lo que hay que mirar.
 */

/* Suelta la pantalla y deja la app CIEGA pero viva. `1` = cedida. */
int bmo_pantalla_ceder(BMO_PANTALLA *p) {
    if (p == 0 || p->cap == 0) {
        return 0;
    }
    if (bmo_pantalla_soltar() != 0) {
        return 0;
    }
    p->cap = 0;
    p->pixeles = 0;
    return 1;
}

/* La vuelve a pedir y remide el panel. `1` = otra vez es tuya.
 *
 * ** Se REMIDE y no se restaura lo guardado: entre ceder y volver, la pantalla
 * pudo cambiar de manos, y una anchura vieja se indexa igual de bien y pinta
 * en el sitio que no es. */
int bmo_pantalla_recuperar(BMO_PANTALLA *p) {
    return bmo_pantalla_abrir(p);
}

/* Cabe este pixel en lo que el kernel mapeo?
 *
 * ** Existe porque `RANGECHECK` de DOOM enseno lo que cuesta no tenerla: un
 * guardia que se equivoca en el borde no protege el 99% de los casos, protege
 * todos menos el unico que pasa. Aqui la cuenta se hace UNA vez y bien, contra
 * `bytes` --que es lo mapeado de verdad-- y no contra `alto * paso`, que es lo
 * que uno supone. */
int bmo_pantalla_cabe(BMO_PANTALLA *p, int x, int y) {
    unsigned long long i;

    if (p == 0 || p->pixeles == 0) {
        return 0;
    }
    if (x < 0 || y < 0 || x >= p->ancho || y >= p->alto) {
        return 0;
    }
    i = (unsigned long long)y * (unsigned long long)p->paso
      + (unsigned long long)x;
    if ((i + 1) * 4 > p->bytes) {
        return 0;
    }
    return 1;
}

#endif /* BMO_PANTALLA_H */
