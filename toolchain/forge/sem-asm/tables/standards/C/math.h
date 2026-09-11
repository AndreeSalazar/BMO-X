/* math.h -- lo que hoy se puede hacer con la ruta SSE, y nada mas.
 *
 * == Por que este fichero es tan corto ==
 *
 * BMO C evalua coma flotante por SSE y ya sabe PASAR y DEVOLVER `double`, asi
 * que las funciones que son **una comparacion y un signo** salen solas. Las
 * demas --`sqrt`, `sin`, `cos`, `atan`, `pow`, `log`-- no son eso: son series
 * o instrucciones dedicadas, y escribirlas a medias daria numeros que parecen
 * bien y no lo estan.
 *
 * No estan, y se dice. Un `sin` que devolviera el argumento seria peor que un
 * error de compilacion: el programa correria y pintaria el mundo torcido.
 *
 * [!] `sqrtsd` es UNA instruccion de SSE2 y entraria por `intrinsics.toml` sin
 * escribir una serie -- ese es el camino cuando haga falta, y es una fila de
 * tabla, no un fichero de C.
 */
#ifndef BMO_MATH_H
#define BMO_MATH_H

/* Valor absoluto en coma flotante.
 *
 * Se escribe con una comparacion y una negacion, y no bajando a entero: en
 * coma flotante la negacion es cambiar el bit de signo, que tambien es lo
 * correcto para el cero. */
double fabs(double v) {
    if (v < 0.0) {
        return -v;
    }
    return v;
}

float fabsf(float v) {
    if (v < 0.0) {
        return -v;
    }
    return v;
}

#endif /* BMO_MATH_H */
