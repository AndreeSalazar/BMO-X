/* monton/roja.h -- la arena y el reparto -- boundary tags EN BANDA, sin red
 *
 * Un CARRIL de `<bmo/monton.h>` (L6g). La cabecera entera --que
 * explica por que existe esta pieza-- esta en la fachada; aqui va lo
 * que cambia de color.
 *
 * [carril]  ROJO         las cabeceras de 16 bytes van PEGADAS delante de cada
 *                        trozo y no hay pagina de guarda entre vecinos. Quien
 *                        toca el reparto toca donde aterriza todo lo que el
 *                        programa reserve despues
 * [cuesta]  DATO         repartir dos veces el mismo trozo es dos duenos de un
 *                        byte, y el segundo no se entera
 * [riesgo]  AJENO SILENCIO
 *                        AJENO: `free` y `bmo_monton_tam` reciben punteros que
 *                        escribio otro. SILENCIO: pisar una cabecera no da
 *                        fault -- sigue. Es exactamente lo que le paso al
 *                        bloque 1336 con la fila 200 de DOOM
 */
#ifndef BMO_MONTON_ROJA_H
#define BMO_MONTON_ROJA_H

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

#endif /* BMO_MONTON_ROJA_H */
