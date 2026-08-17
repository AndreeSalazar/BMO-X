/* monton.h -- EL ASIGNADOR DE RING 3, sobre un bloque de `KIND_MEMORIA`.
 *
 * == El hueco que tapa, con su numero ==
 *
 * Hasta el 2026-08-09 cada `malloc` de C era **una peticion al kernel**, y el
 * kernel da **cuatro por proceso**. El quinto devolvia 0. Para lo que se
 * escribio --DOOM pide su zona UNA vez y se la administra con `Z_Zone`-- eso
 * parecia suficiente, y la cuenta estaba mal hecha: el arranque de DOOM llama a
 * `malloc` **una docena de veces**. Solo `I_AtExit` son siete, uno por cada
 * funcion que se registra al salir; y ademas van `DG_ScreenBuffer`, la zona de
 * 6 MiB, el directorio de lumps del WAD, la paleta y las rutas.
 *
 * Con cuatro peticiones, DOOM muere en el quinto `malloc` con un `I_Error`.
 *
 * == Por que el kernel NO hace esto, y esta bien que no lo haga ==
 *
 * `ring0/obj/memoria.rs` lo dice en su cabecera: el kernel entrega **un bloque
 * grande, entero y contiguo**, y el reparto es POLITICA. La politica vive en
 * Ring 3, donde cada lenguaje puede traer la suya sin pedirle permiso a nadie.
 * Esto es la de C.
 *
 * == Las tres cosas que arregla de golpe ==
 *
 * 1. **Los mas de cuatro `malloc`.** Una peticion al kernel, N reparticiones.
 * 2. **`realloc`**, que devolvia 0 y decia por que: *"sin el tamano viejo,
 *    copiar es adivinar"*. Ahora el tamano viejo esta en la cabecera del
 *    bloque, a ocho bytes del puntero. Se escribe en tres lineas, como estaba
 *    prometido en `<stdlib.h>`.
 * 3. ** **El contrato de `fread`.** El kernel solo acepta escribir dentro de un
 *    bloque que el concedio, y `<bmo/archivo.h>` traduce el puntero a un
 *    desplazamiento contra `__bmo_bloque_base`. Con un `malloc` por peticion,
 *    solo el PRIMER bloque estaba publicado: leer un fichero a cualquier otro
 *    devolvia 0 **sin quejarse**. Con un solo bloque para todo, cualquier
 *    puntero del monton cae dentro del publicado. El contrato deja de depender
 *    del orden en que se pidio la memoria.
 *
 * == UNA sola arena, y es una decision ==
 *
 * El monton pide UN bloque y no vuelve a pedir. Cuando se acaba, `malloc`
 * devuelve 0.
 *
 * Podria pedir un segundo --el kernel da cuatro-- y no lo hace por el punto 3:
 * `fread` traduce contra la base de UN bloque, asi que una segunda arena
 * traeria de vuelta el fallo silencioso que este fichero acaba de quitar. Un
 * `malloc` que dice "no" es mejor que un `fread` que dice "lei cero".
 *
 * == Cuanto pide ==
 *
 * `BMO_MONTON_BYTES`, y **el programa lo declara**:
 *
 *     #define BMO_MONTON_BYTES (12 * 1024 * 1024)
 *     #include <stdlib.h>
 *
 * Por defecto **1 MiB**: lo bastante para un programa normal, y lo bastante
 * poco para que un `hola mundo` que llame a `malloc(32)` no se lleve por
 * delante ocho megas de RAM fisica contigua. Quien necesita mas lo dice, que es
 * la misma idea que la seccion `Manifest` de BEF persigue en grande.
 *
 * Y no se pide nada hasta el primer `malloc`: un programa que no reserva
 * memoria no gasta ni una pagina ni una de sus cuatro peticiones.
 *
 * == La forma, y por que esta ==
 *
 * Boundary tags con lista implicita: los bloques van pegados dentro de la
 * arena y se recorren sumando su tamano. Cada uno lleva 16 bytes de cabecera
 * --el tamano total y si esta libre-- y el reparto es **primer hueco que
 * sirve**.
 *
 * ** La fusion de huecos se hace AL BUSCAR, no al liberar**, y eso es lo que
 * quita el puntero al bloque anterior: fusionar en `free` obliga a saber quien
 * hay detras, y para eso hace falta o un enlace mas por bloque o recorrer la
 * arena entera desde el principio. Recorriendola ya en la busqueda, dos huecos
 * seguidos se ven solos y se juntan sin que nadie lleve un puntero de mas.
 *
 * El coste es lineal en el numero de bloques. Se dice porque es real: para los
 * quince `malloc` de DOOM no significa nada, y para un programa que reserve
 * cien mil trozos si. Ese dia lo que toca es una lista de libres por tamano, no
 * un parche aqui -- y la forma de saber que ha llegado ese dia es medirlo, no
 * suponerlo.
 */
#ifndef BMO_MONTON_H
#define BMO_MONTON_H

#ifndef BMO_MONTON_BYTES
#define BMO_MONTON_BYTES (1024 * 1024)
#endif

/* La cabecera de cada bloque: 16 bytes.
 *
 * Son 16 y no 12 para que la carga util quede alineada a 16 -- la arena empieza
 * en una frontera de pagina, asi que base+16 lo esta, y todos los tamanos se
 * redondean a 16. Un `double` mal alineado aqui no falla en x86, pero un dia
 * habra un `movaps` y entonces si. */
struct BMO_TROZO {
    /* Bytes que ocupa el bloque ENTERO, cabecera incluida. Es lo que se suma
     * para llegar al siguiente. */
    unsigned long long tam;
    /* 1 = libre, 0 = repartido. */
    unsigned long long libre;
};

/* Donde empieza la arena, y donde acaba. Cero hasta el primer `malloc`. */
unsigned long long __bmo_monton_ini;
unsigned long long __bmo_monton_fin;

/* Pide la arena al kernel. Devuelve 1 si hay monton, 0 si no lo hay.
 *
 * `bmo_bloque_pedir` es la peticion cruda a `KIND_MEMORIA` --la misma emision
 * que el `malloc` empotrado del compilador, con otro nombre-- y de paso publica
 * el handle y la base en `__bmo_bloque_cap` / `__bmo_bloque_base`, que es lo
 * que `<bmo/archivo.h>` necesita para que `fread` pueda escribir aqui. */
int bmo_monton_arranca() {
    unsigned long long base;
    struct BMO_TROZO *t;

    if (__bmo_monton_ini != 0) {
        return 1;
    }
    base = (unsigned long long)bmo_bloque_pedir(BMO_MONTON_BYTES);
    if (base == 0) {
        return 0;
    }
    __bmo_monton_ini = base;
    __bmo_monton_fin = base + BMO_MONTON_BYTES;

    /* Un solo hueco que lo ocupa todo. */
    t = (struct BMO_TROZO *)base;
    t->tam = BMO_MONTON_BYTES;
    t->libre = 1;
    return 1;
}

void *malloc(unsigned long long bytes) {
    unsigned long long p;
    unsigned long long pide;
    unsigned long long sobra;
    struct BMO_TROZO *t;
    struct BMO_TROZO *sig;
    struct BMO_TROZO *resto;

    if (bytes == 0) {
        return 0;
    }
    if (bmo_monton_arranca() == 0) {
        return 0;
    }

    /* Cabecera + carga util, redondeado a 16. */
    pide = bytes + 16;
    pide = (pide + 15) & 0xFFFFFFFFFFFFFFF0;
    /* Un tamano absurdo se desborda al sumarle 16 y daria un `pide` pequenito
     * que SI cabe: entonces se repartiria un bloque diminuto para una peticion
     * enorme y quien escriba se lleva por delante el monton entero. */
    if (pide < bytes) {
        return 0;
    }

    p = __bmo_monton_ini;
    while (p < __bmo_monton_fin) {
        t = (struct BMO_TROZO *)p;
        if (t->libre == 1) {
            /* Fusionar con los huecos que vengan detras, mientras los haya. */
            sig = (struct BMO_TROZO *)(p + t->tam);
            while ((p + t->tam) < __bmo_monton_fin && sig->libre == 1) {
                t->tam = t->tam + sig->tam;
                sig = (struct BMO_TROZO *)(p + t->tam);
            }
            if (t->tam >= pide) {
                sobra = t->tam - pide;
                /* Partir solo si lo que queda puede ser un bloque util. Un
                 * resto de 16 bytes seria una cabecera sin sitio para nada, y
                 * un resto de 0 seria una cabecera fuera de la arena. */
                if (sobra >= 32) {
                    resto = (struct BMO_TROZO *)(p + pide);
                    resto->tam = sobra;
                    resto->libre = 1;
                    t->tam = pide;
                }
                t->libre = 0;
                return (void *)(p + 16);
            }
        }
        p = p + t->tam;
    }
    /* No cabe. Se dice devolviendo 0, que es lo que el estandar define y lo que
     * quien llama esta obligado a mirar. Ver la regla 2 de `docs/identidad/LA_RAM.md`:
     * `malloc` no miente nunca. */
    return 0;
}

void free(void *p) {
    unsigned long long d;
    struct BMO_TROZO *t;

    if (p == 0) {
        return;
    }
    d = (unsigned long long)p;
    /* Un puntero que no salio de aqui no se toca. Es barato y evita que un
     * `free` de una direccion de pila escriba una "cabecera" encima de las
     * variables locales de alguien. */
    if (d < __bmo_monton_ini + 16 || d >= __bmo_monton_fin) {
        return;
    }
    t = (struct BMO_TROZO *)(d - 16);
    t->libre = 1;
    /* Y no se fusiona aqui: lo hace la busqueda del proximo `malloc`. Ver la
     * cabecera. */
}

/* Cuantos bytes de carga util tiene el bloque que empieza en `p`.
 *
 * Es lo que `realloc` no podia saber cuando cada `malloc` era una peticion al
 * kernel, y por eso devolvia 0. */
unsigned long long bmo_monton_tam(void *p) {
    unsigned long long d;
    struct BMO_TROZO *t;

    if (p == 0) {
        return 0;
    }
    d = (unsigned long long)p;
    if (d < __bmo_monton_ini + 16 || d >= __bmo_monton_fin) {
        return 0;
    }
    t = (struct BMO_TROZO *)(d - 16);
    return t->tam - 16;
}

/* -- Lo que el monton deja mirar, para que no haya que creerselo ------
 *
 * Regla 7 de `docs/identidad/LA_RAM.md`: lo que se declara, se cumple o se grita. Un
 * asignador que no sabe contarse a si mismo no puede prometer nada.
 */

/* Bytes libres SUMADOS. No es lo mismo que "cabe una peticion de este tamano":
 * pueden estar repartidos en huecos sueltos, y esa diferencia es justo la
 * fragmentacion. Por eso hay tambien `bmo_monton_hueco_mayor`. */
unsigned long long bmo_monton_libre() {
    unsigned long long p;
    unsigned long long suma;
    struct BMO_TROZO *t;

    suma = 0;
    p = __bmo_monton_ini;
    while (p != 0 && p < __bmo_monton_fin) {
        t = (struct BMO_TROZO *)p;
        if (t->libre == 1) {
            suma = suma + t->tam - 16;
        }
        p = p + t->tam;
    }
    return suma;
}

/* El hueco contiguo mas grande, ya fusionado. Este SI dice cuanto cabe. */
unsigned long long bmo_monton_hueco_mayor() {
    unsigned long long p;
    unsigned long long mejor;
    unsigned long long corrido;
    struct BMO_TROZO *t;

    mejor = 0;
    corrido = 0;
    p = __bmo_monton_ini;
    while (p != 0 && p < __bmo_monton_fin) {
        t = (struct BMO_TROZO *)p;
        if (t->libre == 1) {
            corrido = corrido + t->tam;
            if (corrido > mejor) {
                mejor = corrido;
            }
        } else {
            corrido = 0;
        }
        p = p + t->tam;
    }
    if (mejor < 16) {
        return 0;
    }
    return mejor - 16;
}

#endif /* BMO_MONTON_H */
