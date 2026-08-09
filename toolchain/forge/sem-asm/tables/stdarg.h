/* stdarg.h -- los argumentos variadicos, y por que aqui son un PUNTERO.
 *
 * == El problema que resuelve, dicho con el codigo que lo pide ==
 *
 *     void M_snprintf(char *buf, size_t n, const char *s, ...) {
 *         va_list args;
 *         va_start(args, s);
 *         M_vsnprintf(buf, n, s, args);   <-- AQUI
 *         va_end(args);
 *     }
 *
 * Esa linea es el patron entero de la familia `v*` de C, y es lo que hace
 * `I_Error`, `M_snprintf` y medio DOOM. La lista de argumentos **se le pasa a
 * otra funcion**.
 *
 * BMO C tenia `__va_arg(i)`: el i-esimo argumento sin nombre. Sirve dentro de
 * la funcion que declara el `...`, porque describe una posicion en SU marco de
 * pila. En cuanto viaja a otra funcion el numero ya no apunta a nada: la
 * funcion de destino tiene su propio marco y su propio `rbp`.
 *
 * Asi que `va_list` tiene que ser una DIRECCION, y lo es: `__va_list()`.
 *
 * == Y aqui eso sale barato ==
 *
 * BMO C pasa los argumentos **por la pila, de derecha a izquierda**, asi que
 * los variadicos quedan seguidos en memoria detras de los que tienen nombre:
 * la lista variadica ya ES un array. El `va_list` de C es su primera casilla, y
 * `va_arg` es avanzar una.
 *
 * En la convencion de registros de SysV esto es una estructura de tres campos
 * (dos cursores y un area de salvado) y un prologo que vuelca seis registros.
 * La convencion mas vieja vuelve a salir ganando, igual que le paso a
 * `__va_arg`.
 *
 * [!] `va_start` **se come el `last`**. El estandar lo pide para calcular donde
 * empiezan los variadicos; aqui esa cuenta la hace el compilador, que sabe
 * cuantos parametros con nombre tiene la funcion. Se acepta el argumento y no
 * se mira, para que el codigo de fuera compile sin tocar una linea.
 */
#ifndef BMO_STDARG_H
#define BMO_STDARG_H

/* Una casilla de la lista. Todo argumento variadico viaja en ocho bytes --
 * enteros, punteros y caracteres promovidos-- asi que el array es homogeneo. */
typedef unsigned long long *va_list;

#define va_start(ap, last) ((ap) = (va_list)__va_list())
#define va_arg(ap, type)   ((type)(*(ap)++))
#define va_end(ap)         ((ap) = (ap))
#define va_copy(d, s)      ((d) = (s))

#endif /* BMO_STDARG_H */
