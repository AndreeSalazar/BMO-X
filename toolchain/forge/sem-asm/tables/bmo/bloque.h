/* bloque.h -- QUE BLOQUE DEL KERNEL ES EL DEL MONTON, y donde empieza.
 *
 * == Por que existe esta cabecera, que son dos lineas ==
 *
 * El kernel solo acepta escribir en un bloque **si le dices cual**, y las
 * operaciones que cruzan la frontera no hablan de punteros: hablan de un
 * handle y de un desplazamiento contra la base de ese bloque.
 *
 *     fread            escribe en `base + desde`      <bmo/archivo.h>
 *     MEM_OFRECER      presta `base + desde`, N bytes <bmo/superficie.h>
 *
 * Las dos necesitan los mismos dos numeros, y hasta el 2026-08-19 los declaraba
 * `<bmo/archivo.h>`. O sea que **una app que solo quisiera una ventana no
 * compilaba**: `superficie.h` leia `__bmo_bloque_cap` sin traerlo, y el error
 * que salia era *"no esta declarado"* sobre un simbolo con dos guiones bajos
 * que el programa no habia escrito nunca.
 *
 * Se descubrio portando `ray.bex` a una ventana, que es el paso 2b de
 * `docs/plan/PLAN_DIRECTOR.md`: el primer programa que pidio superficie sin
 * pedir ficheros.
 *
 * == Y por que NO viven en el compilador ==
 *
 * Porque asi **quien no las usa no las paga**. `publicar_bloque` (ver
 * `codegen/frame.rs`) comprueba si la global existe antes de emitir nada: un
 * programa que no incluya ni esta cabecera ni las dos que la traen no gasta ni
 * un byte en publicarlas.
 *
 * Valen 0 hasta el primer `malloc`, y eso es lo correcto: antes de pedir
 * memoria no hay bloque del que hablar.
 *
 * -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
 *
 * Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
 * toco. La ley esta en `META-KERNEL_HARD.md`; el juez, en
 * `toolchain/tools/contrato/contrato.py`.
 *
 * [carril]  ROJO         son los DOS numeros con los que el kernel decide
 *                        DONDE escribir; tocarlos arrastra a `fread` y a
 *                        `MEM_OFRECER` a la vez
 * [cuesta]  DATO         una `base` equivocada no da fault: hace que el kernel
 *                        escriba el fichero de alguien en el sitio que no es
 * [riesgo]  ESPEJO       las escribe el CODEGEN (`publicar_bloque`) y las leen
 *                        DOS cabeceras. Si el compilador deja de publicarlas,
 *                        aqui valen 0
 */
#ifndef BMO_BLOQUE_H
#define BMO_BLOQUE_H

unsigned long long __bmo_bloque_cap;
unsigned long long __bmo_bloque_base;

#endif /* BMO_BLOQUE_H */
