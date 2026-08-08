/* scroll.h -- una ventana que se mueve sobre un historial, en C.
 *
 * == Que problema resuelve ==
 *
 * Una rejilla de salida guarda N filas y ensena M. Lo que sale por arriba se
 * pierde para siempre a menos que alguien lleve la cuenta de CUANTO se ha
 * subido. Esa cuenta es todo lo que hay aqui: un entero, `vista`, que dice
 * cuantas filas hacia atras esta mirando el usuario.
 *
 *     vista = 0   -> se ve lo ultimo (el fondo del historial)
 *     vista = 5   -> se ve cinco filas mas atras
 *
 * == Por que son funciones puras y no un objeto ==
 *
 * El historial vive donde lo ponga quien llama: aqui no hay heap ni un buffer
 * escondido. Estas funciones no tocan memoria -- reciben la `vista` de ahora y
 * devuelven la de despues. Eso las hace probar sin arrancar nada, que es la
 * unica forma de saber que el tope funciona sin encender la maquina.
 *
 * == Lo que evita cada tope ==
 *
 * El clamp de los dos extremos no es cosmetico. Sin el:
 *   - pasarse por arriba ensena filas en blanco, y parece que se ha perdido
 *     todo el historial;
 *   - pasarse por abajo deja `vista` en negativo, y la siguiente escritura
 *     pinta por encima de lo que ya habia.
 *
 * El convenio de signo es el mismo que usa el compositor: **positivo = hacia
 * atras en el tiempo**. Si aqui fuera al reves, la rueda giraria al contrario
 * segun quien la lea.
 */
#ifndef BMO_SCROLL_H
#define BMO_SCROLL_H

#include <bmo/entrada.h>

/* Cuantas filas mueve una muesca de rueda.
 *
 * Tres. Una sola se queda corta --hay que girar una eternidad para recorrer una
 * pantalla-- y una pagina entera se pasa: pierdes el hilo de lo que estabas
 * leyendo. Es el paso de cualquier terminal. */
#define BMO_SCROLL_MUESCA 3

/* La nueva `vista` tras mover `filas`, ya topada en los dos extremos.
 *
 * `guardadas` es el historial entero, `visibles` la ventana. El tope de arriba
 * es `guardadas - visibles`: subir mas seria pedir filas que nunca existieron.
 *
 * Si el historial no llena la ventana todavia (`guardadas <= visibles`), el
 * unico sitio valido es el 0 -- y eso se calcula, no se supone. */
int bmo_scroll_mover(int vista, int filas, int guardadas, int visibles) {
    int tope;
    int nueva;
    tope = guardadas - visibles;
    if (tope < 0) {
        tope = 0;
    }
    nueva = vista + filas;
    if (nueva < 0) {
        nueva = 0;
    }
    if (nueva > tope) {
        nueva = tope;
    }
    return nueva;
}

/* La nueva `vista` tras `muescas` de rueda. Positivo = hacia arriba (al
 * pasado), que es la direccion en la que la gente espera ver lo viejo. */
int bmo_scroll_rueda(int vista, int muescas, int guardadas, int visibles) {
    return bmo_scroll_mover(vista, muescas * BMO_SCROLL_MUESCA, guardadas, visibles);
}

/* La nueva `vista` tras una tecla. Las que no son RePag/AvPag no mueven nada.
 *
 * * Una pagina es `visibles - 1`, no `visibles`. La fila que se solapa es lo
 *   que deja seguir leyendo: saltar la pantalla entera corta una frase justo
 *   por el borde y obliga a volver atras.
 *
 * Esto funciona **pase lo que pase con el raton**, que es la razon de que
 * exista ademas de la rueda: un teclado siempre hay. */
int bmo_scroll_tecla(int vista, int tecla, int guardadas, int visibles) {
    int pagina;
    pagina = visibles - 1;
    if (pagina < 1) {
        pagina = 1;
    }
    if (tecla == BMO_TECLA_REPAG) {
        return bmo_scroll_mover(vista, pagina, guardadas, visibles);
    }
    if (tecla == BMO_TECLA_AVPAG) {
        return bmo_scroll_mover(vista, 0 - pagina, guardadas, visibles);
    }
    if (tecla == BMO_TECLA_INICIO) {
        return bmo_scroll_mover(vista, guardadas, guardadas, visibles);
    }
    if (tecla == BMO_TECLA_FIN) {
        return bmo_scroll_mover(vista, 0 - guardadas, guardadas, visibles);
    }
    return vista;
}

/* La primera fila del historial que toca dibujar, dada la `vista`.
 *
 * Es la cuenta que se equivoca sola cuando se escribe a mano en el sitio de
 * pintar: el fondo del historial es `guardadas - visibles`, y `vista` lo corre
 * hacia atras. Tenerla aqui es lo que hace que el dibujo no la reinvente. */
int bmo_scroll_primera(int vista, int guardadas, int visibles) {
    int base;
    base = guardadas - visibles - vista;
    if (base < 0) {
        base = 0;
    }
    return base;
}

/* Se esta mirando el pasado?
 *
 * Sirve para poner un aviso en pantalla. No es un adorno: una ventana que
 * ensena el pasado sin decirlo se confunde con una que se ha colgado, y la
 * reaccion normal a eso es reiniciar la maquina. */
int bmo_scroll_en_historial(int vista) {
    return vista > 0;
}

#endif /* BMO_SCROLL_H */
