/* superficie.h -- una app dibuja en SU memoria, y el DIRECTOR la compone.
 *
 * == El cambio de modelo, en una frase ==
 *
 * Hoy la pantalla es EXCLUSIVA: `prestar_pantalla` se la quita al compositor y
 * se la da al hijo entera. Por eso un programa no puede vivir en una caja --
 * no hay caja, hay relevo, y mientras el hijo corre el escritorio no existe.
 *
 * Una SUPERFICIE le da la vuelta: el programa pide memoria, **dibuja ahi**, y
 * se la OFRECE al DIRECTOR. El DIRECTOR la pega dentro de un marco con sus tres
 * botones. La pantalla no cambia de dueno ni una vez.
 *
 * == ** PANTALLA COMPLETA DEJA DE SER "TE DOY EL HARDWARE" ==
 *
 * Y esa es la parte que importa. En Windows, pantalla completa exclusiva
 * entrega el dispositivo: si el juego se cuelga, te llevas la maquina.
 *
 * Aqui, pantalla completa es **que el DIRECTOR no te dibuje el borde**. Sigue
 * componiendo, asi que Alt+Tab sigue, el conmutador sigue, y `Ctrl+Alt+ESC`
 * --que vive en Ring 0 y no depende de que nadie este vivo-- sigue. Un juego
 * colgado a pantalla completa se cierra con el teclado en vez de con el boton
 * de reset.
 *
 * == Por que la descripcion viaja DENTRO de la memoria ==
 *
 * El kernel presta BYTES: `loan::offer` mueve un rango y no sabe --ni tiene por
 * que saber-- que hay pixeles dentro. Meterle ancho y alto seria ensenarle al
 * kernel lo que es una imagen para que se lo repita a otro.
 *
 * Asi que la superficie **se describe a si misma**: los primeros 32 bytes del
 * bloque son la cabecera y los pixeles van detras. Es la misma decision que
 * `BRES` dentro de la seccion de recursos y que `BICO` dentro del paquete --
 * en este sistema, **el dato dice lo que es**, y quien lo transporta no
 * necesita entenderlo.
 *
 * Consecuencia practica: esto NO pide ni una operacion nueva del kernel.
 * `MEM_OP_OFRECER` ya existe y ya basta.
 *
 * == La disposicion ==
 *
 * ```text
 *    0..4    "BSUP"
 *    4..8    ancho  en pixeles (u32)
 *    8..12   alto   en pixeles (u32)
 *   12..16   stride en PIXELES, no en bytes (u32)
 *   16..20   formato: 0 = BGRA de 32 bits, que es el del framebuffer (u32)
 *   20..24   SECUENCIA: sube cada vez que el dibujo esta entero (u32)
 *   24..28   BUZON: donde empieza, en bytes desde el principio. 0 = no hay
 *   28..32   BUZON: cuantas ranuras (potencia de 2)
 *   32..     los pixeles, y detras de ellos el buzon si se pidio
 * ```
 *
 * == EL BUZON: el camino de vuelta, y por que no cuesta un syscall ==
 *
 * Una superficie deja que una app ENSENE. El buzon deja que la TOQUES.
 *
 * Y va aqui dentro, en el mismo bloque, por el mismo motivo que la cabecera:
 * el kernel presta BYTES y no tiene por que saber que hay dentro. El bloque
 * que la app ofrece se mapea en el DIRECTOR con **derecho de escritura** --lo
 * concede la propia app al ofrecerlo, `RIGHT_READ | RIGHT_WRITE`-- asi que el
 * DIRECTOR puede dejar una tecla ahi con un `mov`, igual que ya lee un pixel
 * con un `mov`.
 *
 * ** CERO PUERTAS POR TECLA, Y NINGUNA OPERACION NUEVA DEL KERNEL. Una puerta
 * son 969 ciclos (`docs/componente/LA_PUERTA_POR_DENTRO.md`); para una
 * calculadora eso es gratis, pero para algo que siga al teclado a sesenta por
 * segundo es el precio equivocado.
 *
 * == Es OPCIONAL, y eso es lo que decide quien se queda las teclas ==
 *
 * `bmo_superficie_crear` no pide buzon: una app que solo ensena --un reloj, un
 * medidor-- no lo necesita, y el DIRECTOR **no le manda teclas**, o sea que el
 * escritorio las conserva. Pedirlo es decir *"yo se leer"*, y el foco solo se
 * le puede dar a quien lo dijo.
 *
 * == La forma del buzon ==
 *
 * ```text
 *    +0   CABEZA (u32)   la escribe el DIRECTOR
 *    +4   COLA   (u32)   la escribe la app
 *    +8   PUNTERO: x en los 16 bajos, y en los 16 altos.  El DIRECTOR
 *    +12  BOTONES en el byte 0, y DENTRO en el byte 1.    El DIRECTOR
 *    +16  las ranuras, de 8 bytes cada una
 * ```
 *
 * == ** POR QUE EL PUNTERO NO ES UNA RANURA, Y EL CLIC SI ==
 *
 * Son dos cosas de naturaleza distinta y meterlas en la misma cola las rompe a
 * las dos:
 *
 * ```text
 *    un CLIC      es un HECHO.  Paso, y si se pierde no vuelve a pasar
 *    la POSICION  es un ESTADO. Solo importa la ultima, y las de antes
 *                 no valen nada
 * ```
 *
 * Un anillo de 64 ranuras con una posicion que cambia sesenta veces por segundo
 * se llena en un segundo, y entonces **el DIRECTOR descarta las NUEVAS** --que
 * es lo correcto para un hecho-- o sea que la app acabaria leyendo donde estuvo
 * el raton hace un segundo y creyendose que es ahora. Un buzon lleno de
 * posiciones no es lento: **miente**.
 *
 * Por eso la posicion se PISA en un sitio fijo. La app lee la ultima y no hay
 * cola que se atrase; y si lee dos veces seguidas sin que el raton se mueva, le
 * contesta lo mismo, que es exactamente lo que un estado tiene que hacer.
 *
 * Un escritor y un lector, cada uno con su indice: no hace falta cerrojo, por
 * la misma razon que no lo hace falta la secuencia. Vacio es `cabeza == cola`;
 * lleno es que la cabeza alcanzaria a la cola, y entonces **el DIRECTOR
 * descarta** en vez de esperar -- un compositor que espere a una app colgada es
 * una app rota llevandose el escritorio.
 *
 * ** Y una ranura es un evento CRUDO, el mismo `unsigned long long` que
 * devuelve `bmo_entrada_evento`. No hay formato nuevo que aprender: el codigo
 * que ya sabe leer una tecla de la pantalla exclusiva sirve tal cual dentro de
 * una ventana.
 *
 * ** LA SECUENCIA ES EL UNICO CAMPO QUE NO ES OBVIO, y es el que hace que esto
 * funcione sin cerrojos.
 *
 * El DIRECTOR lee la superficie mientras la app la escribe: son dos procesos
 * sobre la misma memoria y no hay quien los pare. Sin nada mas, el compositor
 * pegaria medio fotograma viejo y medio nuevo -- el desgarro de toda la vida.
 *
 * La regla es de una linea: **la app sube `secuencia` cuando el dibujo esta
 * entero, y el DIRECTOR solo repinta cuando ve un numero distinto del que
 * pego la ultima vez.** Un fotograma a medias no cambia el numero, asi que no
 * se pinta; y el peor caso es ensenar el anterior un fotograma mas, que es
 * exactamente lo que uno quiere que pase.
 *
 * No es un cerrojo y no debe serlo: un cerrojo entre dos procesos deja al
 * compositor esperando a una app que se colgo -- y entonces una app rota se
 * lleva el escritorio, que es justo lo que este diseno existe para impedir.
 *
 * == Como se usa ==
 *
 *     BMO_SUPERFICIE *s = bmo_superficie_crear(640, 400);
 *     for (;;) {
 *         dibujar_en(bmo_superficie_pixeles(s), 640, 400);
 *         bmo_superficie_lista(s);     // el dibujo esta entero
 *         bmo_ceder();
 *     }
 *
 * -- ** ESTE FICHERO SE PARTIO (L6g, nivel 3) -------------------------
 *
 * Tenia DOS MASAS con costes distintos, y la regla de L6e dice que eso
 * es un fichero mal cortado: *el corte va justo por donde cambia el
 * coste*. Cada mitad vive ahora en `bmo/superficie/` con su semaforo,
 * su `[cuesta]` y su `[riesgo]`.
 *
 * **Fuera no cambia nada.** `#include <bmo/superficie.h>` sigue trayendo
 * lo mismo que traia: esto es la fachada, igual que un `mod.rs` que
 * re-exporta. Incluir un carril suelto tambien vale.
 *
 *     roja.h     pedir el bloque, escribir su cabecera y OFRECERLO al DIRECTOR
 *     amarilla.h decodificar lo que el DIRECTOR escribio: eventos y puntero
 *
 * [carril]  ROJO         el reparto, y hereda el color del carril que manda
 * [cuesta]  DATO         hereda de `roja.h`: lo que se ofrece al DIRECTOR es
 *                        lo que el DIRECTOR se cree
 * [riesgo]  AJENO UNICO
 *                        hereda de `roja.h`: la cabecera la lee otro proceso,
 *                        y ofrecer no se deshace
 *
 */
#ifndef BMO_SUPERFICIE_H
#define BMO_SUPERFICIE_H

#include <bmo/superficie/roja.h>
#include <bmo/superficie/amarilla.h>

#endif /* BMO_SUPERFICIE_H */
