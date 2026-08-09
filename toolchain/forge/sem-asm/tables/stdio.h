/* stdio.h -- EL FORMATEADOR DE EJECUCION, que es lo que le faltaba a DOOM.
 *
 * == Donde se paraba el unity build ==
 *
 *     error: printf con formato calculado en tiempo de ejecucion no se compila
 *
 * El `printf` de BMO C recorre la plantilla **al compilar** y emite la salida
 * ya troceada: los literales van dentro de las instrucciones y cada `%` es una
 * llamada a su conversor. Es rapido y no necesita libreria, y por eso se hizo
 * asi. Pero exige que la plantilla sea un literal.
 *
 * Y hay codigo, mucho, donde no lo es:
 *
 *     printf(message, demoversion, ...);       g_game.c
 *     vfprintf(stderr, error, argptr);         i_system.c
 *     vsnprintf(buf, buf_len, s, args);        m_misc.c
 *
 * Las tres piden lo mismo: **un formateador que recorra la cadena mientras el
 * programa corre**. Eso es lo que hay aqui, y con el se completan de una vez
 * `vsnprintf`, `snprintf`, `sprintf`, `vprintf` y el `printf` de formato
 * variable -- todas son la misma funcion con otro destino.
 *
 * == Por que en C y no en el codegen ==
 *
 * La misma razon que da `archivo.h` de si misma. Un formateador con banderas,
 * anchura y precision escrito en opcodes serian cuatrocientas lineas de bytes
 * que nadie puede leer ni corregir. En C se lee, se prueba fila a fila en el
 * emulador, y **se arregla sin tocar el compilador**.
 *
 * Del compilador solo hace falta una pieza, y es de cuatro lineas:
 * `bmo_escribir`, que saca a la consola un buffer que no existia al compilar.
 *
 * == Lo que este formateador hace MEJOR que el de linea ==
 *
 * **La anchura se aplica.** El `printf` en linea lee `%7i` y se salta el 7 --
 * lo dice en su propio comentario-- porque para rellenar hay que saber cuantos
 * caracteres va a ocupar el numero ANTES de escribirlo, y sus conversores
 * escriben directamente a la consola. Aqui el numero se arma primero en un
 * array y luego se rellena, asi que una tabla alineada sale alineada.
 *
 * == Y lo que NO hace, dicho y no escondido ==
 *
 * - **`%f`, `%e`, `%g`**: no hay ruta de coma flotante aqui. Se escribe la
 *   conversion tal cual (`%f`) en vez de un numero inventado, para que se vea
 *   en la salida cual fue y no haya que adivinar.
 * - **Un `%u` o un `%x` de mas de 2^63**: la division de BMO C es con signo
 *   (`idiv`), asi que el decimal de un numero con el bit alto puesto sale mal.
 *   El hexadecimal SI sale bien -- va por desplazamientos y mascara justamente
 *   por esto. Un puntero de Ring 3 esta muy por debajo de ese tope.
 * - **`%n`**: no. Escribe en memoria del llamante a traves del formato, que es
 *   la conversion que sobra en todas las libc del mundo.
 */
#ifndef BMO_STDIO_H
#define BMO_STDIO_H

#include <bmo/bmo.h>
#include <bmo/archivo.h>
#include <stdarg.h>

/* -- Lo que un `<stdio.h>` de verdad trae consigo ----------------------
 *
 * No es adorno: `sha1.h` de DOOM declara `SHA1_Update(..., size_t len)` y no
 * incluye nada mas. En una libc real ese tipo llega por aqui, asi que un
 * `<stdio.h>` que no lo traiga rompe ficheros que no tienen ningun fallo.
 */
#ifndef BMO_SIZE_T
#define BMO_SIZE_T
typedef unsigned long size_t;
#endif

#ifndef NULL
#define NULL 0
#endif

#ifndef EOF
#define EOF (-1)
#endif

/* Los tres puntos de partida de `fseek`, con los valores de siempre. Aqui el
 * `desde` de `fseek` es un entero y estos son los tres que puede valer. */
#ifndef SEEK_SET
#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2
#endif

/* Los tres flujos. Valen `0` y eso NO es un descuido: `fprintf` y `vfprintf`
 * van a la consola pase lo que pase mientras la escritura a fichero no este
 * cableada, asi que el handle no se mira. El dia que se mire, estos tres pasan
 * a valer algo y el codigo de fuera no cambia una linea. */
FILE *stdin;
FILE *stdout;
FILE *stderr;

/* Saca a la consola `n` bytes de un buffer. La SINTETIZA el compilador: su
 * cuerpo es `bmo_lower::console::write_buffer`, el mismo emisor que usa el
 * `printf` en linea para las cadenas. Sin ella, la unica forma de imprimir
 * desde C algo que no se sabe al compilar seria un syscall por caracter. */
void bmo_escribir(char *bytes, unsigned long long n);

/* -- El DESTINO de un formateo -----------------------------------------
 *
 * Una sola estructura para los dos destinos posibles, porque la diferencia
 * entre `printf` y `snprintf` es exactamente esta y ninguna otra. Con dos
 * funciones separadas habria dos recorridos de la plantilla que mantener
 * iguales, y el dia que uno arregle un caso de esquina el otro se queda atras.
 *
 * `total` cuenta lo que se HABRIA escrito sin limite, que es lo que el estandar
 * manda devolver: por eso un programa puede llamar con `lim = 0` para preguntar
 * cuanto sitio necesita.
 */
struct BMO_SALIDA {
    char *dst;                    /* 0 -> a la consola */
    unsigned long long lim;       /* capacidad de dst, el cero final incluido */
    unsigned long long puestos;   /* cuantos hay ya en dst */
    unsigned long long total;     /* cuantos habria puesto sin limite */
    unsigned long long enbuf;     /* cuantos hay en buf (destino consola) */
    char buf[64];
};

void bmo_fmt_vaciar(struct BMO_SALIDA *s) {
    if (s->enbuf > 0) {
        bmo_escribir(s->buf, s->enbuf);
        s->enbuf = 0;
    }
}

/* Un byte al destino.
 *
 * [!] Al buffer del usuario se escribe dejando SIEMPRE sitio para el cero
 * final. Esa comparacion --`puestos + 1 < lim` y no `puestos < lim`-- es la
 * diferencia entre un `snprintf` y un desbordamiento de uno.
 */
void bmo_fmt_byte(struct BMO_SALIDA *s, int c) {
    s->total = s->total + 1;
    if (s->dst == 0) {
        s->buf[s->enbuf] = (char)c;
        s->enbuf = s->enbuf + 1;
        if (s->enbuf >= 64) {
            bmo_fmt_vaciar(s);
        }
        return;
    }
    if (s->lim > 0) {
        if (s->puestos + 1 < s->lim) {
            s->dst[s->puestos] = (char)c;
            s->puestos = s->puestos + 1;
        }
    }
}

void bmo_fmt_relleno(struct BMO_SALIDA *s, int n, int c) {
    while (n > 0) {
        bmo_fmt_byte(s, c);
        n = n - 1;
    }
}

/* Los digitos de `v` en `base`, escritos AL REVES en `num`. Devuelve cuantos.
 *
 * Salen del reves porque es como salen: dividiendo se obtiene primero el
 * digito menos significativo. Darles la vuelta aqui obligaria a un segundo
 * bucle; el llamante los recorre hacia atras y no cuesta nada.
 *
 * [!] La base 16 va por DESPLAZAMIENTO y mascara, no por division: la division
 * de BMO C es `idiv` --con signo-- y un puntero o una mascara con el bit 63
 * puesto saldrian negativos. El `& 15` despues del `>> 4` es lo que convierte
 * el `sar` que emite el compilador en el desplazamiento logico que hace falta.
 */
int bmo_fmt_digitos(unsigned long long v, int base, int mayus, char *num) {
    char *tabla;
    int n;
    int d;

    if (mayus) {
        tabla = "0123456789ABCDEF";
    } else {
        tabla = "0123456789abcdef";
    }
    n = 0;
    if (v == 0) {
        num[0] = '0';
        return 1;
    }
    while (v != 0) {
        if (base == 16) {
            d = (int)(v & 15);
            v = (v >> 4) & 0x0FFFFFFFFFFFFFFF;
        } else {
            d = (int)(v % 10);
            v = v / 10;
        }
        num[n] = tabla[d];
        n = n + 1;
    }
    return n;
}

/* -- El recorrido de la plantilla -------------------------------------- */

int bmo_formatear(char *dst, unsigned long long lim, const char *fmt, void *ap) {
    struct BMO_SALIDA s;
    unsigned long long *arg;
    unsigned long long v;
    long long sv;
    char num[24];
    char *str;
    int i;
    int c;
    int ancho;
    int prec;
    int izq;      /* la bandera '-' */
    int ceros;    /* la bandera '0' */
    int neg;
    int largo;
    int k;

    s.dst = dst;
    s.lim = lim;
    s.puestos = 0;
    s.total = 0;
    s.enbuf = 0;
    arg = (unsigned long long *)ap;

    i = 0;
    while (fmt[i] != 0) {
        c = (int)fmt[i] & 0xFF;
        if (c != '%') {
            bmo_fmt_byte(&s, c);
            i = i + 1;
            continue;
        }
        i = i + 1;

        /* Banderas. Solo las dos que cambian la salida de un numero; '+', ' '
         * y '#' se aceptan y se tiran, que es mejor que tomarlas por la
         * conversion y acusar a un caracter que era una bandera. */
        izq = 0;
        ceros = 0;
        while (fmt[i] == '-' || fmt[i] == '+' || fmt[i] == ' '
               || fmt[i] == '#' || fmt[i] == '0') {
            if (fmt[i] == '-') { izq = 1; }
            if (fmt[i] == '0') { ceros = 1; }
            i = i + 1;
        }

        /* Anchura, literal o pedida con '*'. */
        ancho = 0;
        if (fmt[i] == '*') {
            ancho = (int)(*arg);
            arg = arg + 1;
            if (ancho < 0) { izq = 1; ancho = -ancho; }
            i = i + 1;
        } else {
            while (fmt[i] >= '0' && fmt[i] <= '9') {
                ancho = ancho * 10 + ((int)fmt[i] - '0');
                i = i + 1;
            }
        }

        /* Precision. -1 significa "no la puso", que NO es lo mismo que 0:
         * `%.0d` de un cero no escribe nada, y `%d` escribe el cero. */
        prec = -1;
        if (fmt[i] == '.') {
            i = i + 1;
            prec = 0;
            if (fmt[i] == '*') {
                prec = (int)(*arg);
                arg = arg + 1;
                i = i + 1;
            } else {
                while (fmt[i] >= '0' && fmt[i] <= '9') {
                    prec = prec * 10 + ((int)fmt[i] - '0');
                    i = i + 1;
                }
            }
        }

        /* Modificadores de longitud: en BMO todo entero viaja en 64 bits, asi
         * que `%ld` y `%d` producen lo mismo y se saltan. */
        while (fmt[i] == 'l' || fmt[i] == 'h' || fmt[i] == 'z'
               || fmt[i] == 'j' || fmt[i] == 't' || fmt[i] == 'L') {
            i = i + 1;
        }

        c = (int)fmt[i] & 0xFF;
        if (c == 0) {
            /* Un '%' al final. Se escribe y se para: la plantilla se acabo. */
            bmo_fmt_byte(&s, '%');
            break;
        }
        i = i + 1;

        if (c == '%') {
            bmo_fmt_byte(&s, '%');
            continue;
        }

        if (c == 'c') {
            v = *arg;
            arg = arg + 1;
            if (izq == 0) { bmo_fmt_relleno(&s, ancho - 1, ' '); }
            bmo_fmt_byte(&s, (int)(v & 0xFF));
            if (izq) { bmo_fmt_relleno(&s, ancho - 1, ' '); }
            continue;
        }

        if (c == 's') {
            str = (char *)(*arg);
            arg = arg + 1;
            /* Un `%s` de un puntero nulo revienta en cualquier libc; aqui se
             * dice, porque un `.bex` que se cae no deja stack trace. */
            if (str == 0) { str = "(nulo)"; }
            largo = 0;
            while (str[largo] != 0) {
                if (prec >= 0 && largo >= prec) { break; }
                largo = largo + 1;
            }
            if (izq == 0) { bmo_fmt_relleno(&s, ancho - largo, ' '); }
            k = 0;
            while (k < largo) {
                bmo_fmt_byte(&s, (int)str[k] & 0xFF);
                k = k + 1;
            }
            if (izq) { bmo_fmt_relleno(&s, ancho - largo, ' '); }
            continue;
        }

        neg = 0;
        if (c == 'd' || c == 'i') {
            sv = (long long)(*arg);
            arg = arg + 1;
            if (sv < 0) {
                neg = 1;
                sv = -sv;
            }
            largo = bmo_fmt_digitos((unsigned long long)sv, 10, 0, num);
        } else if (c == 'u') {
            v = *arg;
            arg = arg + 1;
            largo = bmo_fmt_digitos(v, 10, 0, num);
        } else if (c == 'x' || c == 'p') {
            v = *arg;
            arg = arg + 1;
            largo = bmo_fmt_digitos(v, 16, 0, num);
        } else if (c == 'X') {
            v = *arg;
            arg = arg + 1;
            largo = bmo_fmt_digitos(v, 16, 1, num);
        } else {
            /* Conversion que no se entiende. Se escribe tal cual --`%f`,
             * `%q`-- y NO se consume argumento: inventar un numero seria
             * peor, y consumir descolocaria todos los que vienen detras. */
            bmo_fmt_byte(&s, '%');
            bmo_fmt_byte(&s, c);
            continue;
        }

        /* Precision en un entero = digitos MINIMOS, con ceros delante. */
        if (prec > largo) {
            k = prec - largo;
        } else {
            k = 0;
        }
        /* El '0' no se aplica si hay precision: lo dice el estandar y hay
         * codigo que se apoya en ello (`%08.3d`). */
        if (prec >= 0) { ceros = 0; }

        if (izq == 0 && ceros == 0) {
            bmo_fmt_relleno(&s, ancho - largo - k - neg, ' ');
        }
        if (neg) { bmo_fmt_byte(&s, '-'); }
        if (izq == 0 && ceros) {
            bmo_fmt_relleno(&s, ancho - largo - k - neg, '0');
        }
        bmo_fmt_relleno(&s, k, '0');
        k = largo;
        while (k > 0) {
            k = k - 1;
            bmo_fmt_byte(&s, (int)num[k] & 0xFF);
        }
        if (izq) {
            bmo_fmt_relleno(&s, ancho - largo - neg, ' ');
        }
    }

    if (s.dst == 0) {
        bmo_fmt_vaciar(&s);
    } else {
        if (s.lim > 0) {
            s.dst[s.puestos] = 0;
        }
    }
    return (int)s.total;
}

/* -- La familia, que ya es toda la misma funcion ------------------------ */

int vsnprintf(char *dst, unsigned long long lim, const char *fmt, void *ap) {
    return bmo_formatear(dst, lim, fmt, ap);
}

int vsprintf(char *dst, const char *fmt, void *ap) {
    /* Sin limite: es lo que pide el estandar y por lo que `sprintf` es
     * peligroso. Se le da un tope enorme en vez de ninguno para que un fallo
     * sea un truncamiento y no un paseo por la memoria. */
    return bmo_formatear(dst, 0x7FFFFFFF, fmt, ap);
}

int vprintf(const char *fmt, void *ap) {
    return bmo_formatear(0, 0, fmt, ap);
}

int snprintf(char *dst, unsigned long long lim, const char *fmt, ...) {
    return bmo_formatear(dst, lim, fmt, __va_list());
}

int sprintf(char *dst, const char *fmt, ...) {
    return bmo_formatear(dst, 0x7FFFFFFF, fmt, __va_list());
}

/* `vfprintf` y `fprintf` van a la consola pase lo que pase.
 *
 * No es pereza: hoy `fopen` ignora el modo y el camino de escritura a fichero
 * no esta cableado (lo dice `fwrite`, que devuelve 0 a proposito). Mandar a la
 * consola lo que iba a `stderr` es lo que un programa portado espera VER; el
 * dia que la escritura exista, esto mira el handle. */
int vfprintf(FILE *f, const char *fmt, void *ap) {
    return bmo_formatear(0, 0, fmt, ap);
}

int fprintf(FILE *f, const char *fmt, ...) {
    return bmo_formatear(0, 0, fmt, __va_list());
}

/* -- LEER de una cadena: `sscanf` --------------------------------------
 *
 * El otro lado del formateador, y hace falta por lo mismo: un fichero de
 * configuracion de DOOM se lee con `sscanf(strparm, "%x", &parm)`, y decidir
 * la base mirando la cadena es exactamente lo que hace `%i`.
 *
 * Lo que se compila: `%d %i %u %x %X %o %c %s %%`, la anchura, y los literales
 * del formato --incluido el `0x` de `sscanf(str, " 0x%x", r)`, que es como
 * `M_StrToInt` distingue las bases--. Un espacio en el formato se traga
 * CUALQUIER cantidad de blancos, que es lo que el estandar dice y lo que ese
 * ` %d` del principio esta pidiendo.
 *
 * Devuelve **cuantas asignaciones se hicieron**, que es lo que mira quien
 * llama: `sscanf(...) == 1` es la pregunta "lo entendio?".
 */
int bmo_es_blanco(int c) {
    if (c == ' ' || c == '\t' || c == '\n' || c == '\r') { return 1; }
    return 0;
}

/* El valor de un digito en la base dada, o -1 si no lo es. */
int bmo_valor_digito(int c, int base) {
    int v;
    v = -1;
    if (c >= '0' && c <= '9') { v = c - '0'; }
    if (c >= 'a' && c <= 'f') { v = c - 'a' + 10; }
    if (c >= 'A' && c <= 'F') { v = c - 'A' + 10; }
    if (v < 0 || v >= base) { return -1; }
    return v;
}

int vsscanf(const char *s, const char *fmt, void *ap) {
    unsigned long long *arg;
    int i;
    int j;
    int n;
    int base;
    int neg;
    int digitos;
    int ancho;
    int c;
    int d;
    long long v;
    int *pi;
    char *ps;

    arg = (unsigned long long *)ap;
    i = 0;
    j = 0;
    n = 0;

    while (fmt[i] != 0) {
        c = (int)fmt[i] & 0xFF;

        if (bmo_es_blanco(c)) {
            while (fmt[i] != 0 && bmo_es_blanco((int)fmt[i] & 0xFF)) { i = i + 1; }
            while (s[j] != 0 && bmo_es_blanco((int)s[j] & 0xFF)) { j = j + 1; }
            continue;
        }

        if (c != '%') {
            /* Un literal del formato TIENE que estar en la entrada. Si no
             * esta, se para: es lo que hace que ` 0x%x` no se trague un
             * numero decimal. */
            if (s[j] != fmt[i]) { return n; }
            i = i + 1;
            j = j + 1;
            continue;
        }

        i = i + 1;
        ancho = 0;
        while (fmt[i] >= '0' && fmt[i] <= '9') {
            ancho = ancho * 10 + ((int)fmt[i] - '0');
            i = i + 1;
        }
        while (fmt[i] == 'l' || fmt[i] == 'h' || fmt[i] == 'z') { i = i + 1; }
        c = (int)fmt[i] & 0xFF;
        if (c == 0) { return n; }
        i = i + 1;

        if (c == '%') {
            if (s[j] != '%') { return n; }
            j = j + 1;
            continue;
        }

        if (c == 'c') {
            if (s[j] == 0) { return n; }
            ps = (char *)(*arg);
            arg = arg + 1;
            ps[0] = s[j];
            j = j + 1;
            n = n + 1;
            continue;
        }

        if (c == 's') {
            while (s[j] != 0 && bmo_es_blanco((int)s[j] & 0xFF)) { j = j + 1; }
            if (s[j] == 0) { return n; }
            ps = (char *)(*arg);
            arg = arg + 1;
            d = 0;
            while (s[j] != 0 && bmo_es_blanco((int)s[j] & 0xFF) == 0) {
                if (ancho > 0 && d >= ancho) { break; }
                ps[d] = s[j];
                d = d + 1;
                j = j + 1;
            }
            ps[d] = 0;
            n = n + 1;
            continue;
        }

        /* A partir de aqui, un numero. */
        base = 10;
        if (c == 'x' || c == 'X') { base = 16; }
        if (c == 'o') { base = 8; }
        if (c == 'i') { base = 0; }
        if (c != 'd' && c != 'i' && c != 'u' && c != 'x' && c != 'X' && c != 'o') {
            /* Conversion que no se entiende: se para y se dice cuantas van. */
            return n;
        }

        while (s[j] != 0 && bmo_es_blanco((int)s[j] & 0xFF)) { j = j + 1; }
        neg = 0;
        if (s[j] == '-') { neg = 1; j = j + 1; }
        else if (s[j] == '+') { j = j + 1; }

        /* `%i` decide la base MIRANDO la cadena, que es toda su razon de ser:
         * `0x` es hexadecimal, un `0` delante es octal, y lo demas decimal. */
        if (base == 0) {
            base = 10;
            if (s[j] == '0') {
                base = 8;
                if (s[j + 1] == 'x' || s[j + 1] == 'X') {
                    base = 16;
                    j = j + 2;
                } else {
                    j = j + 1;
                }
            }
        } else if (base == 16) {
            /* Un `0x` delante de un `%x` es opcional y hay que comerselo. */
            if (s[j] == '0') {
                if (s[j + 1] == 'x' || s[j + 1] == 'X') { j = j + 2; }
            }
        }

        v = 0;
        digitos = 0;
        while (s[j] != 0) {
            d = bmo_valor_digito((int)s[j] & 0xFF, base);
            if (d < 0) { break; }
            if (ancho > 0 && digitos >= ancho) { break; }
            v = v * base + d;
            digitos = digitos + 1;
            j = j + 1;
        }
        /* Ni un digito = no se pudo convertir. Devolver el numero de
         * asignaciones HECHAS es lo que separa "no entendio" de "entendio un
         * cero", y hay codigo que se juega la configuracion en esa
         * diferencia. */
        if (digitos == 0) { return n; }
        if (neg) { v = -v; }
        pi = (int *)(*arg);
        arg = arg + 1;
        pi[0] = (int)v;
        n = n + 1;
    }
    return n;
}

int sscanf(const char *s, const char *fmt, ...) {
    return vsscanf(s, fmt, __va_list());
}

/* -- Las que el sistema TODAVIA no puede hacer ------------------------
 *
 * Existen y **contestan que no**, con el valor de fallo que el estandar define
 * para cada una. Es la misma decision que se tomo con `fwrite`: que existan
 * desbloquea la compilacion de quien las nombra --y son cuatro sitios de DOOM--
 * mientras que fingir que funcionaron daria un programa que cree haber borrado
 * su fichero temporal.
 *
 * El dia que `KIND_ARCHIVO` tenga borrado y renombrado, estas cambian aqui y en
 * ningun otro sitio.
 */
int remove(const char *ruta) {
    return -1;
}

int rename(const char *de, const char *a) {
    return -1;
}

/* No hay nada que vaciar: `bmo_escribir` llega a la consola en la propia
 * llamada, no hay buffer intermedio que se quede a medias. Devuelve exito
 * porque es verdad: lo que se escribio, escrito esta. */
int fflush(FILE *f) {
    return 0;
}

int fputc(int c, FILE *f) {
    char b[1];
    b[0] = (char)c;
    bmo_escribir(b, 1);
    return c;
}

int putchar(int c) {
    char b[1];
    b[0] = (char)c;
    bmo_escribir(b, 1);
    return c;
}

/* `fputs` NO pone salto de linea. `puts` SI. Es la diferencia mas olvidada de
 * la libc, y una salida corrida es lo que se ve cuando alguien las cambia. */
int fputs(const char *s, FILE *f) {
    int n;
    n = 0;
    while (s[n] != 0) { n = n + 1; }
    bmo_escribir((char *)s, (unsigned long long)n);
    return n;
}

int puts(const char *s) {
    int n;
    n = 0;
    while (s[n] != 0) { n = n + 1; }
    bmo_escribir((char *)s, (unsigned long long)n);
    bmo_escribir("\n", 1);
    return n + 1;
}

#endif /* BMO_STDIO_H */
