/* imagen.h -- pintar una imagen que tu app lleva DENTRO.
 *
 * == Por que esto hacia falta, y que desbloquea ==
 *
 * Una app podia llevar una imagen dentro de su `.bex` desde que `bmo-pack`
 * escribe la seccion de recursos, y **no sabia pintarla**. El unico que
 * descifraba un `BICO` era el DIRECTOR, y solo a 16x16 porque es lo que mide un
 * icono del escritorio: *"escalar un icono de otro tamano es una decision de
 * aspecto que no toca aqui"*. Tenia razon -- ahi no tocaba. Toca aqui.
 *
 * Con esto, un tercero que quiera imagenes en su programa ya puede, HOY, sin
 * esperar a que exista un decodificador de PNG.
 *
 * == Lo que cuesta, y donde esta el ahorro de verdad ==
 *
 * ```text
 *    CERO RAM hasta que la pides   el cargador SALTA la seccion de recursos:
 *                                  una imagen dentro de tu `.bex` no ocupa
 *                                  memoria hasta que la lees
 *    4 bytes por pixel en disco    `BICO` son pixeles en crudo, sin comprimir
 *    un test de bit por pixel      el alfa a cero se SALTA. No hay mezcla
 *    RECORTA contra tu superficie  lo que no cabe no se dibuja, no se escribe
 *                                  fuera
 * ```
 *
 * ** NO HAY DECODIFICADOR, Y ESO NO ES UNA CARENCIA DE ESTE FICHERO: es el
 * formato. `BICO` es **una imagen sin decodificador** -- ocho bytes de cabecera
 * y pixeles tal cual. Cuesta sitio en disco y no cuesta ni una linea de
 * descompresion, ni un buffer temporal, ni un fallo de formato malformado que
 * alguien pueda usar contra ti. Para un icono, una vineta o una textura
 * pequena es el intercambio correcto; para una foto de dos megapixeles no, y
 * entonces lo que falta es un decodificador de verdad y no otra cabecera.
 *
 * == El alfa es un BIT, no un canal ==
 *
 * Si el byte alto es cero, el pixel no se dibuja; si no, se dibuja entero. No
 * hay mezcla con el fondo, y eso es una decision y no una simplificacion: una
 * mezcla de verdad son tres multiplicaciones por pixel y por fotograma, y el
 * sitio de eso --si algun dia hace falta-- es un compositor, no la cabecera con
 * la que una app pinta su logo. El DIRECTOR hace exactamente lo mismo con los
 * iconos, y por el mismo motivo: *"no hay mezcla, o esta o no esta"*.
 *
 * == Como se usa ==
 *
 *     #include <bmo/paquete.h>
 *     #include <bmo/superficie.h>
 *     #include <bmo/imagen.h>
 *
 *     PAQUETE *p = paquete_mio();
 *     unsigned long long n = paquete_leer(p, "logo", buf, TOPE);
 *     bmo_imagen_pinta(px, 640, 640, 400, 10, 10, buf, (int)n);
 *
 * [carril]  VERDE        escribe en la memoria de la PROPIA app, y recorta
 *                        contra lo que ella declara
 * [cuesta]  NADA         una imagen mal dibujada se ve y se arregla
 * [riesgo]  ESPEJO       el formato `BICO` lo ESCRIBE `Nuevo-Bico` en
 *                        `build/ejemplos.ps1` y lo leen DOS sitios mas: el
 *                        DIRECTOR (`scene/launcher.rs`) y esto. Tres lectores
 *                        de los mismos bytes, y el orden B,G,R,A del fichero
 *                        es lo unico que los mantiene de acuerdo
 */
#ifndef BMO_IMAGEN_H
#define BMO_IMAGEN_H

/* `BICO`, el formato entero:
 *
 *     0..4   "BICO"
 *     4..6   ancho (u16 little-endian)
 *     6..8   alto  (u16)
 *     8..    ancho*alto pixeles, u32 little-endian cada uno
 *
 * ** Un pixel son los cuatro bytes B, G, R, A EN ESE ORDEN en el fichero, y por
 * eso leidos como `u32` little-endian salen `0xAARRGGBB`: el byte bajo es el
 * azul y el alto el alfa. Quitandole el alfa queda `0x00RRGGBB`, que es
 * exactamente el color que quiere una superficie -- no hay conversion. */
#define BMO_BICO_CABECERA 8

/* Los dos numeros de la cabecera, o -1 si esto no es un BICO.
 *
 * Se pregunta ANTES de pintar y no se adivina: un bufer que no empieza por
 * "BICO" interpretado como pixeles pinta ruido, y un ruido con forma de imagen
 * es mas dificil de diagnosticar que un cero. */
int bmo_imagen_ancho(const unsigned char *b, int n) {
    if (b == 0 || n < BMO_BICO_CABECERA) {
        return -1;
    }
    if (b[0] != 'B' || b[1] != 'I' || b[2] != 'C' || b[3] != 'O') {
        return -1;
    }
    return (int)b[4] + (int)b[5] * 256;
}

int bmo_imagen_alto(const unsigned char *b, int n) {
    if (bmo_imagen_ancho(b, n) < 0) {
        return -1;
    }
    return (int)b[6] + (int)b[7] * 256;
}

/* **Estan TODOS los pixeles que la cabecera promete?** 1 si si.
 *
 * Es la comprobacion que separa "una imagen" de "los bytes que llegaron". Un
 * recurso cortado a la mitad tiene la cabecera intacta y la mitad de los
 * pixeles: sin esto se leeria pasado el final del bufer, que es el fallo que
 * este fichero NO puede permitirse -- lo que viene de un paquete lo escribio
 * otro. */
int bmo_imagen_completa(const unsigned char *b, int n) {
    int w;
    int h;
    w = bmo_imagen_ancho(b, n);
    h = bmo_imagen_alto(b, n);
    if (w <= 0 || h <= 0) {
        return 0;
    }
    if (n < BMO_BICO_CABECERA + w * h * 4) {
        return 0;
    }
    return 1;
}

/* Un pixel del BICO, ya en `0xAARRGGBB`. Sin comprobar nada: lo hace quien
 * llama, una vez, con `bmo_imagen_completa`. */
unsigned int bmo_imagen_pixel(const unsigned char *b, int i) {
    int o;
    o = BMO_BICO_CABECERA + i * 4;
    return (unsigned int)b[o]
         + ((unsigned int)b[o + 1] << 8)
         + ((unsigned int)b[o + 2] << 16)
         + ((unsigned int)b[o + 3] << 24);
}

/* **Pintar la imagen a escala ENTERA**, recortada contra tu superficie.
 *
 * `stride` es el paso en PIXELES (el que declara tu cabecera `BSUP`); `ancho` y
 * `alto` son los de tu superficie, y son el recorte. `escala` multiplica cada
 * pixel a un cuadrado de ese lado: 1 la deja igual, 2 la pinta al doble.
 *
 * Devuelve 1 si la imagen era valida, 0 si no. Que no se viera ni un pixel
 * --por estar entera fuera-- NO es un fallo y contesta 1 igual: la imagen
 * estaba bien, el sitio no.
 *
 * ** SOLO ESCALA ENTERA, y es a proposito. Media escala obliga a decidir que
 * pasa entre dos pixeles --mezclar, repetir, o dejar huecos-- y eso es una
 * decision de aspecto que esta cabecera no puede tomar por ti. Con un entero no
 * hay nada que decidir y no hay ni una multiplicacion por pixel de destino. */
int bmo_imagen_pinta_escala(unsigned int *px, int stride, int ancho, int alto,
                            int x, int y, const unsigned char *b, int n,
                            int escala) {
    int w;
    int h;
    int sx;
    int sy;
    int dx;
    int dy;
    int fx;
    int fy;
    unsigned int c;

    if (px == 0 || stride <= 0 || escala < 1) {
        return 0;
    }
    if (bmo_imagen_completa(b, n) == 0) {
        return 0;
    }
    w = bmo_imagen_ancho(b, n);
    h = bmo_imagen_alto(b, n);

    sy = 0;
    while (sy < h) {
        sx = 0;
        while (sx < w) {
            c = bmo_imagen_pixel(b, sy * w + sx);
            /* El byte alto a cero = transparente, y se salta. Sin esto una
             * imagen redonda se pinta dentro de su cuadro negro. */
            if ((c >> 24) != 0) {
                fy = 0;
                while (fy < escala) {
                    dy = y + sy * escala + fy;
                    if (dy >= 0 && dy < alto) {
                        fx = 0;
                        while (fx < escala) {
                            dx = x + sx * escala + fx;
                            if (dx >= 0 && dx < ancho && dx < stride) {
                                px[dy * stride + dx] = c & 0x00FFFFFF;
                            }
                            fx = fx + 1;
                        }
                    }
                    fy = fy + 1;
                }
            }
            sx = sx + 1;
        }
        sy = sy + 1;
    }
    return 1;
}

/* Lo mismo, tamano natural. */
int bmo_imagen_pinta(unsigned int *px, int stride, int ancho, int alto,
                     int x, int y, const unsigned char *b, int n) {
    return bmo_imagen_pinta_escala(px, stride, ancho, alto, x, y, b, n, 1);
}

#endif /* BMO_IMAGEN_H */
