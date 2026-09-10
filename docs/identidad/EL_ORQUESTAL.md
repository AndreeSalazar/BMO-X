# EL ORQUESTAL -- la palabra, y por que el orquestador es CELOSO

> **BMO-X no es un SO. Tampoco es un RTOS, ni un unikernel, ni un exokernel, ni
> ninguna otra caja prestada.** Es **BMO: Bare Metal Orquestal**, y esta hoja
> dice que significa eso con mecanismos al lado, no con adjetivos.
>
> Escrita el **2026-09-06**. El `README.md` ya decia en ingles que un SO
> *multiplexa* y BMO-X *orquesta*; lo que faltaba era la mitad incomoda: **por
> que un orquestador tiene que ser celoso, y que se paga por serlo.**

---

## 1. POR QUE RECHAZAR UNA CATEGORIA NO ES SOBERBIA

Es lo contrario. **Una categoria es una promesa que se hace sola**: el que la
oye rellena los huecos con lo que esa palabra le dio la ultima vez.

```text
   "es un SO"      -> entonces correra mi programa, tendra POSIX, tendra hilos
   "es un RTOS"    -> entonces me daras el peor caso en microsegundos
   "es Linux pero" -> entonces puedo portar cualquier cosa
```

Ninguna de las tres es verdad aqui, y **una expectativa que no se cumple es un
informe de fallo que no se puede cerrar** -- porque el fallo no esta en el
codigo, esta en la palabra. Decir *"es BMO"* obliga a preguntar que es, y esa
pregunta se puede contestar. Decir *"es un SO"* no obliga a nada, y por eso
sale caro.

---

## 2. MULTIPLEXAR ES SER GENEROSO. ORQUESTAR ES SER CELOSO.

Un sistema operativo **multiplexa**, y multiplexar es una forma de generosidad:

> Le miente a cada programa diciendole que la maquina entera es suya. Memoria
> infinita, un CPU para el solo, ficheros que siempre estan. La mentira es util
> --lleva cincuenta anios siendolo-- y tiene un precio que no se factura: **para
> mantenerla, el sistema tiene que darlo todo por defecto** y despues quitarlo
> con comprobaciones.

Un orquestador **no da nada. Presta.**

```text
   SO              tienes todo, y te comprobamos cuando pidas de mas
   BMO             tienes lo que DECLARASTE, sabes que es prestado, y el
                   prestamo se acaba cuando el titular quiere
```

** Y ahi esta el celo, que no es un rasgo de caracter: es la unica forma de que
la frase del titular --*"esta maquina me obedece solo a mi"*-- sea algo mas que
una intencion. **Un kernel generoso puede ser capturado por un programa. Uno
celoso, no**, porque nunca solto nada que no pueda recuperar.

---

## 3. ** DONDE ESTA ESCRITO EL CELO

L1 dice que toda regla nombra a su componente y su numero. El celo no es una
actitud del documento: son sitios del arbol, y cada uno se puede abrir.

| el celo | el mecanismo | donde |
|---|---|---|
| **La maquina vuelve, siempre** | `Ctrl+Alt+ESC` devuelve teclado *y* pantalla al kernel desde cualquier programa, comprobado en el unico punto de Ring 0 por el que pasa toda tecla | `core/gato/` |
| **Y vuelve ENTERA** | LA PURGA cierra todo Ring 3, fuerza la recogida y **lo demuestra** -- no vuelve el titular de la pantalla: vuelven todos | `core/purga.rs` |
| **La pantalla es un prestamo** | devolverla **termina** el prestamo, y terminar el prestamo cierra la app. No hay estado intermedio en el que la tenga a medias | `obj/fb/` |
| **La autoridad no viaja** | `EJECUTAR` y `REINICIAR` piden autoridad, se fija **al nacer** y solo desde Ring 0. Un `.bex` que lanzo el escritorio no puede lanzar otro ni reiniciar | `task/autoridad.rs` |
| **El NO llega antes que el si** | LA REGLA 7: el kernel rechaza una carga **antes de reservar el primer marco**, comparando lo declarado contra lo que hay | `task/admitir.rs` |
| **Cada marco tiene titular** | el asignador sabe decir *"ese marco NO es tuyo"*, y la azul dice si esta ENTREGADO | `mm/phys/` |
| **Pedir tiene tope** | el presupuesto de la puerta: una app que pide sin parar se queda sin turno, no sin maquina | `syscall/presupuesto.rs` |
| **Y cuando algo cae, se sabe de quien fue** | LA MORGUE: la azul dejo de decir *"de NADIE VIVO"* y dice de quien era la pila | `core/autopsy.rs` |

### La frase que resume el celo entero

Esta en la cabecera de `task/autoridad.rs`, y se escribio resolviendo otra cosa:

> *"Asi que no se resolvio la delegacion: **se quito**. La autoridad no viaja."*

Un SO habria construido un sistema de delegacion, con reglas para prestarla y
reglas para revocarla, y habria pasado diez anios tapando escaladas. **Aqui la
respuesta a un mecanismo dificil de acotar fue no tenerlo.** Eso es el celo:
no la vigilancia, sino **la negativa a repartir**.

---

## 4. LO QUE EL CELO CUESTA (L3)

Toda regla trae su sacrificio, y este tiene tres y son reales:

* **No corre software ajeno.** No hay POSIX, no hay libc, no hay enlazador
  dinamico. Lo que se ejecuta aqui se compilo aqui, y eso deja fuera casi todo
  el software que existe.
* **Todo lo que un programa necesita hay que declararlo antes.** No hay pedir en
  caliente lo que no se pidio al nacer, asi que un caso que no se previo no se
  resuelve con un `if`: se resuelve cambiando lo que la app declara.
* **El titular de la maquina tiene poder absoluto sobre lo que corre**, y el
  software que exige opacidad --DRM de kernel, anti-cheat-- **se auto-excluye**.
  No es un fallo de compatibilidad: es la consecuencia directa de que la maquina
  obedezca a una sola persona.

** Un celo sin estos tres escritos al lado seria propaganda. Con ellos es una
posicion.

---

## 5. LAS CAJAS QUE SE RECHAZAN, Y QUE PROMETE CADA UNA

| caja | lo que la palabra promete | por que no |
|---|---|---|
| **Sistema operativo** | POSIX, multiplexar, correr lo que sea | la superficie son **2 syscalls**, no trescientas; y no multiplexa: reparte |
| **RTOS** | un peor caso en microsegundos, herencia de prioridad | **no esta medido**, y decir *tiempo real* sin el numero es la mentira mas cara del gremio |
| **Unikernel** | UNA app enlazada con el kernel | aqui corren varias, aisladas, y una puede morir sin llevarse la maquina |
| **Exokernel** | ensenar el hardware y que la app lo gestione | es el primo mas cercano y aun asi no: un exokernel no compone un escritorio ni **presta** una pantalla |
| **Microkernel** | poco dentro, servicios fuera | **el linaje SI se hereda** -- 2 syscalls, servicios en Ring 3, capabilities. Ver la seccion siguiente |
| **"Linux pero pequeno"** | portar cualquier cosa | no hay nada que portar sin recompilar, y ese es el punto entero |

---

## 6. LO QUE SI SE ACEPTA: EL LINAJE

Rechazar la caja no es negar de donde viene. `META-KERNEL_HARD.md` ya lo dejo
escrito y esta hoja no lo cambia:

> **No es un microkernel: lleva su linaje** --2 syscalls, servicios en Ring 3,
> capabilities-- **pero la palabra la pone la ley**: una regla solo existe aqui
> si al lado tiene el componente que la exige y el numero con el que la exige.

La diferencia entre heredar un linaje y ser de una categoria es la misma que hay
entre *"esto viene de ahi"* y *"esto se comporta como aquello"*. Lo primero es
historia y es cierto. Lo segundo es una promesa, y aqui no se firma.

---

## 7. Y SOBRE EL RTOS, EL MATIZ HONESTO

No es que falte la maquinaria. Es que **falta una medida** y hay **una decision
tomada en contra**:

```text
   prioridad estricta al elegir tarea    SI   `choose_next`, sin envejecimiento
   expulsion por temporizador            SI   quantum 4 ticks, 8 la de delante
   reloj monotono que no da la vuelta    SI   LATIDO_OP_CUENTA
   coste de la puerta, medido            SI   969 ciclos
   -------------------------------------------------------------------------
   LATENCIA PEOR CASO, MEDIDA            NO   y es UN arranque
   herencia de prioridad                 NO   sin ella la inversion se la come
   el tiempo publicado a las apps        NO   casilla 5b de PLAN_REX.md
```

Y la decision en contra esta escrita en `task/scheduler/verde.rs`, al elegir
quantum y no prioridad para la ventana de delante:

> *La prioridad es un ORDEN --y un orden estricto excluye--; el quantum es un
> REPARTO.* **El foco no decide QUIEN corre. Decide CUANTO.**

Subirle la prioridad a la app de delante le ganaria el turno al DIRECTOR, y
entonces sus pixeles dejarian de componerse: **la ventana de delante seria la
primera en dejar de refrescarse**, que es el efecto contrario al que la regla
busca.

** Un RTOS elige lo contrario a proposito, y hace bien: no compone un escritorio.
Esa frase es la que separa a BMO de un RTOS, y no es una carencia -- es la
eleccion.

---

Ver [`EL_AISLAMIENTO.md`](EL_AISLAMIENTO.md) (los ocho muros, y **lo que ninguno
para**), [`EL_CONTRATO_DE_CARGA.md`](EL_CONTRATO_DE_CARGA.md) (*el programa
DECLARA, el sistema CONCEDE, el kernel solo COMPRUEBA*),
[`META-KERNEL_HARD.md`](../../META-KERNEL_HARD.md) (la ley, y el linaje),
[`EL_FUERO.md`](../../EL_FUERO.md) (lo que se concede y lo que se exige) y
[`VALKYRIE-ABI/README.md`](../../VALKYRIE-ABI/README.md) (la superficie que se
promete, y por que no tiene anillo).
