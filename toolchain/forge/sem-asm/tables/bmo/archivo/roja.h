/* archivo/roja.h -- abrir, LEER, ESCRIBIR y cerrar -- donde el kernel toca tu memoria
 *
 * Un CARRIL de `<bmo/archivo.h>` (L6g). La cabecera entera --que
 * explica por que existe esta pieza-- esta en la fachada; aqui va lo
 * que cambia de color.
 *
 * [carril]  ROJO         `fread` y `fwrite` traducen un puntero tuyo a un
 *                        desplazamiento contra `__bmo_bloque_base`, y el
 *                        kernel escribe AHI sin mirar tablas de paginas. Tocar
 *                        esta aritmetica mueve donde aterrizan los bytes
 * [cuesta]  DATO         es el camino de guardar: nada llega al disco hasta
 *                        `fclose`, asi que equivocarse aqui pierde el trabajo
 *                        de alguien
 * [riesgo]  AJENO SILENCIO UNICO
 *                        AJENO: el puntero lo escribio el programa, no esto.
 *                        SILENCIO: devolvia 0 sin escribir y DOOM murio
 *                        diciendo que su propio WAD no era un WAD. UNICO:
 *                        `fclose` vuelca entero o no vuelca nada
 */
#ifndef BMO_ARCHIVO_ROJA_H
#define BMO_ARCHIVO_ROJA_H

#include <bmo/bmo.h>
/* ** EL MONTON, y desde el 2026-08-13 es una dependencia de verdad.
 *
 * Antes esta cabecera solo LEIA `__bmo_bloque_cap` y `__bmo_bloque_base`, que
 * las publica la emision de `malloc` -- o sea que si el programa no llamaba a
 * `malloc` valian cero y `fread` no podia funcionar, lo cual era coherente.
 *
 * Ahora `fread` **pide su propio rebote**, asi que necesita `malloc` de verdad
 * y los limites del monton para saber si el destino cae dentro. Incluirlo aqui
 * es lo correcto: quien lee ficheros ya dependia del monton, y hasta hoy esa
 * dependencia era implicita y se rompia con un error a nueve capas de
 * distancia (`'__bmo_monton_ini' no esta declarado`). */
#include <stdlib.h>
#include <string.h>

#define BMO_ARCH_LEER     0x01
#define BMO_ARCH_TAMANO   0x03
#define BMO_ARCH_CERRAR   0x04
#define BMO_ARCH_LEER_EN  0x06
#define BMO_ARCH_SALTAR   0x07
#define BMO_ARCH_ESCRIBIR_DE 0x08
/* Abrir MI PROPIA imagen. Es una operacion de TAREA, no de archivo: se la pides
 * a `BMO_TAREA_ACTUAL` y te devuelve una capability de archivo. */
#define BMO_OP_MI_PAQUETE 0x25

/* -- La ruta, en paquetes de ocho -------------------------------------
 *
 * El kernel acumula la ruta llamada a llamada y corta en el primer byte cero.
 * Si la ruta mide un multiplo de ocho, el ultimo paquete va entero y hace falta
 * uno mas con el cero: el bucle lo hace solo porque `fin` solo se pone cuando
 * se ha VISTO el terminador, no cuando se llenan ocho.
 */
/* Empuja la ruta y nada mas. Separado de `bmo_abrir` porque hay DOS
 * operaciones que la consumen --abrir y crear-- y el empujado es identico:
 * tenerlo dos veces seria tener dos sitios donde equivocarse con el
 * terminador. */
void bmo_empujar_ruta(char *ruta) {
    int i;
    int k;
    int fin;
    unsigned long long p;
    unsigned long long c;

    i = 0;
    fin = 0;
    while (fin == 0) {
        p = 0;
        k = 0;
        while (k < 8) {
            c = (unsigned long long)ruta[i];
            c = c & 0xFF;
            if (c == 0) {
                fin = 1;
                k = 8;
            } else {
                p = p | (c << (k * 8));
                i = i + 1;
                k = k + 1;
            }
        }
        bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_RUTA, p, 0, 0);
    }
}

unsigned long long bmo_abrir(char *ruta) {
    bmo_empujar_ruta(ruta);
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_ARCHIVO_ABRIR, 0, 0, 0);
}

/* -- `FILE` -----------------------------------------------------------
 *
 * Tres campos y ninguno de adorno: el handle del archivo, el del BLOQUE donde
 * se puede leer, y donde empieza ese bloque en memoria. El tercero hace falta
 * para traducir el puntero que da el usuario a un desplazamiento dentro del
 * bloque, que es lo unico que el kernel acepta.
 */
struct BMO_FILE {
    unsigned long long cap;
    unsigned long long bloque;
    unsigned long long base;
    /* * DONDE VA EL CURSOR, contado aqui.
     *
     * El kernel tiene `SALTAR` pero **no una operacion que devuelva la
     * posicion**, asi que `ftell` no puede preguntarsela: hay que llevarla. La
     * actualizan `fread` y `fseek`, que son los dos unicos que la mueven.
     *
     * Llevar un espejo de un estado que vive en otro sitio es una cosa que sale
     * mal sola en cuanto aparece un tercero que lo mueva. Hoy no lo hay, y por
     * eso se puede -- el dia que exista, esto se cambia por una operacion del
     * kernel y no por otro sitio donde apuntar. */
    unsigned long long pos;
};
typedef struct BMO_FILE FILE;

/* * El bloque que reparte `malloc`, y su handle.
 *
 * Las escribe EL COMPILADOR: la emision de `malloc` guarda aqui el handle que
 * le devolvio el kernel y la base del bloque, justo antes de devolver el
 * puntero. Antes ese handle se tiraba --se usaba para pedir la base y adios-- y
 * sin el `fread` no puede existir, porque el kernel solo acepta escribir en un
 * bloque si le dices CUAL.
 *
 * Van declaradas aqui y no en el compilador a proposito: si un programa no
 * incluye esta cabecera, estas globales no existen y `malloc` no emite ni un
 * byte para publicarlas. Quien no lee ficheros no paga por los que si.
 *
 * Valen 0 hasta el primer `malloc`, y eso es lo correcto: antes de pedir
 * memoria no hay bloque del que hablar. */
/* ** SE MUDARON a `<bmo/bloque.h>` el 2026-08-19, y el motivo esta alli: las
 * necesita tambien `<bmo/superficie.h>`, que hasta ese dia las leia sin
 * traerlas -- o sea que una app que solo queria una ventana no compilaba. */
#include <bmo/bloque.h>

/* Monta un `FILE` sobre una capability de archivo ya conseguida.
 *
 * Existe porque hay DOS formas de conseguirla y solo una es por ruta: la otra
 * es `BMO_OP_MI_PAQUETE`, que te da la TUYA sin nombrarla. Compartir esta cola
 * es lo que evita tener dos sitios donde rellenar el mismo struct -- y uno de
 * los dos olvidandose de un campo. */
FILE *bmo_archivo_de(unsigned long long cap) {
    FILE *f;
    if (cap == 0) return 0;
    f = (FILE *)malloc(32);
    if (f == 0) return 0;
    f->cap = cap;
    f->bloque = __bmo_bloque_cap;
    f->base = __bmo_bloque_base;
    f->pos = 0;
    return f;
}

/* ** EL MODO MANDA, y hasta hoy se ignoraba.
 *
 * Decia *"aceptar `w` para luego no escribir seria la clase de promesa que aqui
 * no se hace"*, y era coherente mientras no se pudiera escribir. Ya se puede.
 *
 * `w` y `a` crean; cualquier otra cosa abre para leer. La `b` de `"wb"` se
 * ignora y esta bien: aqui no hay traduccion de saltos de linea que evitar, un
 * fichero son bytes y ya.
 *
 * [!] **`a` (anadir) se comporta como `w`**, o sea que TRUNCA. El kernel abre
 * un archivo de escritura con un buffer vacio y lo vuelca entero al cerrar; no
 * hay forma de decirle "empieza con lo que ya habia". Se acepta la letra para
 * que un programa portado compile, y se dice aqui que hace otra cosa -- que es
 * mejor que rechazarla y que peor que cumplirla.
 *
 * El modo de LECTURA de un `FILE` ya abierto no se guarda porque no hace falta:
 * el kernel lo fijo al abrir y contesta `None` --que aqui es 0-- a la operacion
 * que no corresponde. Pedirle bytes a un archivo de escritura no es un error de
 * permisos, es una pregunta que ese objeto no responde. */
FILE *fopen(char *ruta, char *modo) {
    unsigned long long cap;
    char m;

    m = 0;
    if (modo != 0) {
        m = modo[0];
    }
    if (m == 'w' || m == 'W' || m == 'a' || m == 'A') {
        /* La ruta se empuja igual; lo que cambia es la operacion del final. */
        bmo_empujar_ruta(ruta);
        cap = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_ARCHIVO_CREAR, 0, 0, 0);
    } else {
        cap = bmo_abrir(ruta);
    }
    return bmo_archivo_de(cap);
}

/* **Mi propia imagen**, sin decir donde esta.
 *
 * El kernel se acuerda de por donde entro este proceso, asi que no hay ruta que
 * escribir ni que acertar. Es la diferencia entre pedir por NOMBRE y tener por
 * DERECHO -- quien puede escribir una ruta puede escribir otra, y aqui no se
 * escribe ninguna.
 *
 * Devuelve 0 si el kernel no recuerda de donde salio, que es lo que le pasa a
 * los binarios que el propio kernel embebe. */
FILE *bmo_mi_imagen() {
    return bmo_archivo_de(bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_MI_PAQUETE, 0, 0, 0));
}

/* El rebote de `fread`. Vive en el monton porque tiene que estar donde el
 * kernel puede escribir; se pide una sola vez y no se suelta. 4 KiB porque el
 * caso que existe --una cabecera, un descriptor, un registro-- es de decenas
 * de bytes, y lo grande ya va directo. */
#define BMO_FREAD_REBOTE 4096
unsigned long long __bmo_rebote = 0;

/* Devuelve ELEMENTOS leidos, como `fread` de verdad -- no bytes.
 *
 * ** EL DESTINO PUEDE SER LA PILA, y hasta el 2026-08-13 no podia.
 *
 * El kernel solo escribe dentro de un bloque que el mismo concedio: la
 * comprobacion es una resta contra lo que entrego, no un recorrido de tablas de
 * paginas. Eso es una decision buena y se conserva -- contrato en vez de
 * comprobacion, ver la cabecera de este fichero.
 *
 * Lo que estaba mal era la consecuencia. Esta funcion traducia `dst` a un
 * desplazamiento y, si `dst` no caia dentro, **devolvia CERO sin escribir
 * nada**. La limitacion estaba escrita ahi arriba, con su `[!]` y todo. Y no
 * basto:
 *
 *     wadinfo_t header;                              // w_wad.c:141, la PILA
 *     W_Read(wad_file, 0, &header, sizeof(header));  // -> fread(&header,1,12,f)
 *     if (strncmp(header.identification, "IWAD", 4))
 *         I_Error("Wad file %s doesn't have IWAD or PWAD id");
 *
 * `fread` contestaba 0, `header` se quedaba con la basura que hubiera en la
 * pila, y DOOM moria diciendo que su propio WAD no era un WAD. Un fallo que no
 * se parece en nada a su causa: el fichero estaba perfecto y abierto.
 *
 * [!] Y es la SEGUNDA vez que este mismo fichero lo paga. Cuatro parrafos mas
 * abajo esta la historia de `fseek` ignorando `SEEK_END` *"con el comentario
 * 'solo SEEK_SET por ahora, y se dice'"*, y la frase que quedo escrita fue
 * **decirlo no bastaba**. Volvio a no bastar. Un limite documentado sigue
 * siendo un limite: si el estandar dice que `fread` escribe donde le apuntes,
 * `fread` escribe donde le apuntes.
 *
 * == Como, sin tocar el kernel ni el ABI ==
 *
 * Si el destino esta fuera del monton, se lee a un rebote que SI esta dentro y
 * se copia. Cuesta un `memcpy` de decenas de bytes en el unico caso donde antes
 * costaba el programa entero, y el camino rapido --leer a un `malloc`, que es
 * como se cargan los lumps-- no paga nada: sigue siendo la misma llamada. */
unsigned long long fread(void *dst, unsigned long long tam,
                        unsigned long long n, FILE *f) {
    unsigned long long desde;
    unsigned long long leidos;
    unsigned long long total;
    unsigned long long hechos;
    unsigned long long trozo;
    unsigned char *salida;
    if (f == 0 || tam == 0) return 0;
    total = tam * n;
    if (total == 0) return 0;

    /* Camino rapido: el destino ya esta en el monton, o sea que el kernel lo
     * conoce y puede escribirlo directamente. */
    if ((unsigned long long)dst >= __bmo_monton_ini &&
        (unsigned long long)dst + total <= __bmo_monton_fin) {
        desde = (unsigned long long)dst - f->base;
        leidos = bmo_valor(f->cap, BMO_ARCH_LEER_EN, f->bloque, desde, total);
        /* El cursor avanza por lo que se leyo DE VERDAD, no por lo que se
         * pidio. Sumar `tam*n` haria que `ftell` mintiera justo al final del
         * fichero, que es donde se le pregunta. */
        f->pos = f->pos + leidos;
        return leidos / tam;
    }

    /* Camino con rebote: el destino es la pila, un global, o cualquier sitio
     * que el kernel no concedio. */
    if (__bmo_rebote == 0) {
        __bmo_rebote = (unsigned long long)malloc(BMO_FREAD_REBOTE);
        if (__bmo_rebote == 0) return 0;
    }
    salida = (unsigned char *)dst;
    hechos = 0;
    while (hechos < total) {
        trozo = total - hechos;
        if (trozo > BMO_FREAD_REBOTE) { trozo = BMO_FREAD_REBOTE; }
        desde = __bmo_rebote - f->base;
        leidos = bmo_valor(f->cap, BMO_ARCH_LEER_EN, f->bloque, desde, trozo);
        if (leidos == 0) { break; }
        memcpy(salida + hechos, (void *)__bmo_rebote, leidos);
        f->pos = f->pos + leidos;
        hechos = hechos + leidos;
        /* Lectura corta: el fichero se acabo, y parar aqui es lo correcto --
         * seguir pidiendo daria vueltas sin avanzar. */
        if (leidos < trozo) { break; }
    }
    return hechos / tam;
}

int fclose(FILE *f) {
    if (f == 0) return -1;
    bmo_codigo(f->cap, BMO_ARCH_CERRAR, 0, 0, 0);
    free(f);
    return 0;
}

/* Escribe `n` elementos de `tam` bytes. Devuelve ELEMENTOS escritos.
 *
 * ** ESCRIBE DE VERDAD desde el 2026-08-09. Antes devolvia 0 a proposito --el
 * camino de creacion existia en el kernel y no estaba cableado hasta aqui-- y
 * eso es lo que dejaba a DOOM sin guardar partida.
 *
 * Va por `ARCH_OP_ESCRIBIR_DE`, el espejo de `LEER_EN`: un bloque de golpe. Con
 * `ARCH_OP_ESCRIBIR`, que mete siete bytes por llamada, una partida de DOOM
 * serian decenas de miles de llamadas al sistema.
 *
 * [!] **El origen tiene que salir de `malloc`**, exactamente como el destino de
 * `fread` y por lo mismo: el kernel solo sabe hablar de un bloque que el
 * concedio, y comprobar el rango es una resta contra lo que entrego. Un
 * `fwrite` desde un array de la PILA sale un desplazamiento enorme, el kernel
 * lo rechaza y esto devuelve 0.
 *
 * [!] **Nada llega al disco hasta `fclose`.** El kernel acumula en un buffer que
 * crece y lo vuelca entero al cerrar. Un proceso que muere con el archivo
 * abierto no deja nada -- y eso es lo correcto: guardar lo escrito hasta la
 * mitad seria inventar un fichero que su autor nunca dio por terminado. */
unsigned long long fwrite(const void *src, unsigned long long tam,
                          unsigned long long n, FILE *f) {
    unsigned long long desde;
    unsigned long long puestos;

    if (f == 0 || tam == 0 || n == 0) {
        return 0;
    }
    desde = (unsigned long long)src - f->base;
    puestos = bmo_valor(f->cap, BMO_ARCH_ESCRIBIR_DE, f->bloque, desde, tam * n);
    /* El cursor avanza por lo que entro DE VERDAD. Un archivo de escritura no
     * tiene mas cursor que su longitud, asi que esto es ademas lo que `ftell`
     * contesta -- y es lo que un programa espera de un `ftell` tras escribir. */
    f->pos = f->pos + puestos;
    return puestos / tam;
}

#endif /* BMO_ARCHIVO_ROJA_H */
