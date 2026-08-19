# docs/ -- el indice, y la regla de donde va cada cosa

> ## ★ LA LEY VA DELANTE: [`META-KERNEL_HARD.md`](../META-KERNEL_HARD.md)
>
> **Antes de escribir un documento nuevo aqui, se lee esa.** No es una
> formalidad: es la que dice que una regla sin numero al lado es una
> preferencia, que un eje sin juez es prosa, y que **el corte se elige por la
> pregunta que responde el fichero** (L6b). Esta carpeta esta ordenada por esa
> frase y por ninguna otra.
>
> Su otra mitad es [`CENSO_DE_EJES.md`](CENSO_DE_EJES.md), que vive aqui al lado
> y no en una subcarpeta **a proposito**: la ley dice *que exige cada
> componente* y el censo dice *por donde pasa el trabajo de verdad y que se
> puede TACHAR*. Son dos caras de lo mismo, asi que no tienen familia: tienen
> pareja.
>
> ## ★ Y TIENE HERMANA: [`META-APP_HARD.md`](../META-APP_HARD.md)
>
> Escrita el 2026-08-18, un anillo mas arriba y con la misma forma. La del
> kernel la firma **el silicio**; la de una app la firma **la superficie del
> sistema** -- que a su vez la firmo el silicio. Contesta *que exige BMO-X de
> algo que quiera ser una app, y que le devuelve a cambio*.
>
> Vive en la raiz y no en `componente/` a proposito: una app no es una pieza de
> esta maquina, es lo que la maquina existe para alojar. Es una LEY, no un
> capitulo.
>
> ## ★ Y SON TRES: [`META-SDK_HARD.md`](../META-SDK_HARD.md)
>
> Del mismo dia. La ley de **REX**, la libreria con la que se escribe una app --
> las nueve cabeceras `<bmo/...>` que ya existian y no tenian nombre. La firma
> **la ley de una app**, porque REX existe solo para que cumplirla no cueste
> escribirla siete veces.
>
> Contesta la pregunta que un SDK tiene que contestar antes de crecer: **cuando
> una comodidad es una cabecera y cuando es una operacion nueva.** Su indice
> vive al lado de los ficheros, en `toolchain/forge/sem-asm/tables/bmo/`.

---

## 0. ★★ DONDE VA UN DOCUMENTO NUEVO -- la pregunta antes del sitio

La primera decision no es en que subcarpeta va. Es **si va en `docs/`**.

La regla no la inventa este indice: la tiene escrita `QUE_DESBLOQUEA.md` sobre
si mismo, y es la buena --

> *"Vive en `docs/` y no en `toolchain/lang/cpp/` **a proposito**: la tesis del
> documento es que esto no es una pregunta sobre C++. Ponerlo dentro de C++ lo
> contradiria."*

```
   un documento sobre UNA PIEZA DE CODIGO      vive JUNTO A ESA PIEZA
   un documento sobre EL SISTEMA               vive en docs/
```

Por eso `PLAN_BANCA.md` esta en `toolchain/lang/cobol/`, `CPP_ABI.md` en
`toolchain/lang/cpp/` y `PLAN_VULKAN.md` en `platform/drivers/gpu/rdna4/`.
**No faltan de aqui: estan donde deben.** Moverlos a `docs/` seria decir que son
preguntas del sistema, y no lo son.

Y una vez decidido que si va en `docs/`, la subcarpeta sale de contestar **que
pregunta responde el fichero**:

| si el documento contesta... | va en | y hereda la forma de |
|---|---|---|
| *"que EXIGE esta pieza de quien la use"* | `componente/` | `META-KERNEL_HARD.md` |
| *"que copiar del mundo y que seria un error copiar"* | `maestro/` | `SMP_MAESTRO.md` |
| *"que casillas faltan, que las bloquea, como se sabe que quedo hecha"* | `plan/` | `PLAN_DOOM.md` |
| *"por que BMO-X hace esto distinto en vez de copiar"* | `identidad/` | `LA_RAM.md` |
| *"que teclear en el Ryzen y que tiene que salir"* | `metal/` | la hoja de la tanda anterior |

[!] **Si un documento contesta dos de esas preguntas, esta mal cortado.** Es A2
de la ley aplicada a la prosa: o son dos ficheros, o uno declara cual gana y el
otro lo acata por escrito.

---

## 1. `componente/` -- que EXIGE cada pieza

La forma de la ley aplicada a una sola pieza: no *"que hace BMO-X con el
teclado"* sino **que exige el teclado de quien quiera leerlo**. Los tres se
citan entre si por nombre y declaran que son la misma clase de documento.

| documento | componente de la ley | la cifra que lo ordena |
|---|---|---|
| [`LA_PUERTA_POR_DENTRO.md`](componente/LA_PUERTA_POR_DENTRO.md) | C1 CPU | una puerta = **945 ciclos**, y el handle son 236 de ellos |
| [`EL_COMPOSITOR_Y_EL_ESCANER.md`](componente/EL_COMPOSITOR_Y_EL_ESCANER.md) | C5 FRAMEBUFFER | volcar la pantalla = **27,6 ms** contra 16,7 de un frame |
| [`EL_TECLADO_EXIGE.md`](componente/EL_TECLADO_EXIGE.md) | C7 USB | **seis exigencias**, y el numero que dice cual fallo |
| [`EL_DISCO_EXIGE.md`](componente/EL_DISCO_EXIGE.md) | C6 DISCO | una busqueda de HDD = **59 millones de ciclos**; y la ranura 0 de 32 |

### ★★ La simetria, y sus ocho huecos

**La ley declara DOCE componentes y aqui hay CUATRO capitulos.** Eso no es una
carencia escondida: es la simetria que hace visible el hueco (L6c), y por eso se
escribe la lista entera en vez de solo lo que existe.

```
   C1  CPU            HAY capitulo
   C2  CACHE          falta -- y es el que la ley llama su carencia mas grande:
                      cuatro numeros de [LITERATURA] sin medir aqui
   C3  RAM/MMU        lo cubre identidad/LA_RAM.md, que es OTRA pregunta
   C4  BUS/MMIO       falta
   C5  FRAMEBUFFER    HAY capitulo
   C6  DISCO          HAY capitulo
   C7  USB            HAY capitulo
   C8  RELOJES        falta -- y es del que depende todo lo medido
   C9  IRQ            falta
   C10 ENERGIA        falta
   C11 FIRMWARE       falta
   C12 SMP            lo cubre maestro/SMP_MAESTRO.md, que es OTRA pregunta
```

★ **El capitulo que aparezca se escribe igual y al lado**, y se cita desde su
componente en la ley -- como hacen C1, C5 y C7. Un capitulo que la ley no cita
es un enlace de una sola direccion.

---

## 2. `maestro/` -- que copiar del mundo, y que seria un error copiar

Se escriben **antes de una sola linea de codigo**. Su trabajo, dicho por
`PYTHON_MAESTRO.md`, es *"que esta investigacion no haya que reconstruirla"*.
Los ocho declaran seguir el metodo de `SMP_MAESTRO.md`.

| documento | escrito | la pregunta que separa |
|---|---|---|
| [`SMP_MAESTRO.md`](maestro/SMP_MAESTRO.md) | 08-06 | que mitad de Cell copiar y que mitad seria un error |
| [`AUTOCURACION_MAESTRO.md`](maestro/AUTOCURACION_MAESTRO.md) | 08-08 | de INFORMAR un fallo a ACTUAR sobre el |
| [`AXION_MAESTRO.md`](maestro/AXION_MAESTRO.md) | 08-11 | el mando de los nucleos, por PERFIL |
| [`RED_MAESTRO.md`](maestro/RED_MAESTRO.md) | 08-11 | el limite que el dueno cree que quiere vs. el que le importa |
| [`AUDIO_MAESTRO.md`](maestro/AUDIO_MAESTRO.md) | 08-12 | del silencio a los gatitos, sin inventar un driver |
| [`PYTHON_MAESTRO.md`](maestro/PYTHON_MAESTRO.md) | 08-16 | que hace falta de verdad, y no son los 2 syscalls |
| [`SEGURIDAD_MAESTRO.md`](maestro/SEGURIDAD_MAESTRO.md) | 08-18 | integridad no es autoria, y que backdoor puede esconderse aqui |
| [`IPC_MAESTRO.md`](maestro/IPC_MAESTRO.md) | 08-18 | serializar no es enmarcar, y donde acaba un mensaje |

---

## 3. `plan/` -- casillas con bloqueante y prueba

El formato, dicho por `PLAN_ALMACENAMIENTO.md`: *"casillas ordenadas, cada una
con **que la bloquea** y **como se sabe que quedo hecha**"*.

| documento | de que |
|---|---|
| [`PLAN_DOOM.md`](plan/PLAN_DOOM.md) | de "BMO C compila 69 de 81" a "DOOM se juega" |
| [`PLAN_AUTOCURACION.md`](plan/PLAN_AUTOCURACION.md) | las casillas de su MAESTRO |
| [`PLAN_DIRECTOR.md`](plan/PLAN_DIRECTOR.md) | de compositor a administrador |
| [`PLAN_ALMACENAMIENTO.md`](plan/PLAN_ALMACENAMIENTO.md) | repartir la pila de disco |
| [`PLAN_MAQUETA.md`](plan/PLAN_MAQUETA.md) | como se construye el compilador de composicion |
| [`PLAN_LA_CARA_VIAJA.md`](plan/PLAN_LA_CARA_VIAJA.md) | la maquetacion como DATO, y que pasa si viaja |
| [`PLAN_SEGURIDAD.md`](plan/PLAN_SEGURIDAD.md) | las casillas de su MAESTRO, medidas contra el codigo |

★ **El par MAESTRO + PLAN es la simetria de esta carpeta**, y hoy la tienen
entera AUTOCURACION y SEGURIDAD. No es un defecto que a los demas les falte
--DOOM nunca necesito un maestro, y Python todavia no tiene casillas-- pero **el
dia que un maestro llegue a codigo, su plan va aqui y con este nombre**.

---

## 4. `identidad/` -- por que BMO-X lo hace distinto

La pregunta es *"por que no copiamos"*. `LA_RAM.md` lo dice en su cabecera:
*"para que BMO-X tenga identidad propia en esto. Windows y Linux ya tienen la
suya (...) copiarlas seria heredar sus deudas sin heredar sus motivos."*

| documento | la frase que lo ordena |
|---|---|
| [`LA_RAM.md`](identidad/LA_RAM.md) | la RAM no es donde vive el programa: es donde esta TRABAJANDO |
| [`EL_CONTRATO_DE_CARGA.md`](identidad/EL_CONTRATO_DE_CARGA.md) | el programa DECLARA, el sistema CONCEDE, el kernel solo COMPRUEBA |
| [`LIDERES.md`](identidad/LIDERES.md) | un aparato exclusivo va a UN proceso, que lo REPARTE |
| [`QUE_DESBLOQUEA.md`](identidad/QUE_DESBLOQUEA.md) | lo que desbloquea apps es la SUPERFICIE, no el lenguaje |
| [`ENTRAR_EN_SU_ECOSISTEMA.md`](identidad/ENTRAR_EN_SU_ECOSISTEMA.md) | tres caminos, y solo uno toca la identidad |
| [`LIENZO.md`](identidad/LIENZO.md) | ⚠ **SUPERADO** por `plan/PLAN_DIRECTOR.md` -- se conserva a proposito |

[!] `LIENZO.md` **no se borra y no se arregla**: su conclusion se cayo y el
propio documento dice por que se cayo. Un descarte con motivo se discute; uno sin
motivo es un agujero.

---

## 5. `metal/` -- ★★ REGLAS DURAS DE METAL QUE SE NECESITAN

**Esto no es un archivo de notas viejas.** Es lo que el metal exige para que una
tanda delante del Ryzen no sea *"a ver que pasa"*, y la regla que las cinco hojas
comparten esta escrita en la primera:

> *"Cada prueba dice **que afirma** y **como se cae**. Una prueba que solo puede
> salir bien no prueba nada -- si no se sabe de antemano que aspecto tiene el
> fallo, cualquier cosa que aparezca en pantalla se lee como exito."*

Y la segunda regla, que es de orden: **lo que no toca nada va primero, lo que no
se deshace va al final.**

| hoja | tanda | 
|---|---|
| [`VERIFICACION_METAL.md`](metal/VERIFICACION_METAL.md) | **2026-08-08** |
| [`VERIFICACION_METAL_0809.md`](metal/VERIFICACION_METAL_0809.md) | **2026-08-09** |
| [`PRUEBA_EN_METAL_0810.md`](metal/PRUEBA_EN_METAL_0810.md) | **2026-08-10** |
| [`PRUEBA_EN_METAL.md`](metal/PRUEBA_EN_METAL.md) | ⚠ **2026-08-12** -- el nombre no lo dice |
| [`PRUEBA_EN_METAL_0813.md`](metal/PRUEBA_EN_METAL_0813.md) | **2026-08-13** |

### ⚠ Las dos cosas que esta tabla existe para decir

**1. `PRUEBA_EN_METAL.md` no lleva fecha en el nombre y es del 08-12.** Las otras
cuatro si la llevan. El que abra la carpeta coge la que parece *"la actual"* y
esta cinco dias vieja -- un nombre que promete lo que no es, que es justo la
clase de fallo que la ley persigue. **Renombrarla a su fecha esta pendiente y
sin decidir**; mientras tanto, la fecha vive aqui.

**2. La tanda del 08-17 no tiene hoja.** Es la mas cargada del mes (E6, E7, el
presupuesto por perfil, el kernel de medida con interruptor) y lo que hay que
traer de vuelta vive hoy repartido entre los mensajes de commit. **Escribir
`metal/` para el 08-17 es la casilla abierta de esta carpeta.**

---

## 6. Los guardianes

Dos, y los dos existen por la misma razon: **una regla que nadie mide se
incumple sin que nada grite.**

### 6.1 `toolchain/tools/censo-modular/` -- el metro de L6a y de L7

```bash
python toolchain/tools/censo-modular/censo_modular.py --check
```

L6a pone un numero --*un modulo que pase de ~1.000 lineas se parte, no es una
sugerencia*-- y hasta el 2026-08-18 **nada lo comprobaba**. La prueba de que eso
importa esta en la propia bitacora: `gui/main.rs` **crecio 1.244 lineas** entre
el 08-04 y el 08-12 teniendo ya un plan escrito para partirlo. Nadie decidio
eso; paso un commit cada vez, y ningun paso era lo bastante grande como para
parar.

★ **No juzga el pasado: es un TRINQUETE.** Dieciocho ficheros incumplen L6a hoy,
y un guardian que fallara con los dieciocho se apagaria el primer dia. Asi que
compara contra `LINEA_BASE.txt` y solo dice NO a dos cosas: **un fichero nuevo
por encima de 1.000**, o **uno de la lista que crecio**. Encoger es noticia
buena, se anuncia y se vuelve a sellar. El arbol solo puede mejorar.

Y clasifica **la especie**, que es lo que dice cuanto cuesta el corte:
`CAJON` (media ~30 lineas por funcion: mover texto, demostrable con un hash),
`GIGANTE` (media 150+: el estado local tiene que volverse un struct primero, y
eso es diseno), `TABLA` y `mixto`.

**La otra mitad, `herencia.py`, contesta L7** y entra por la misma llamada: una
puerta, dos preguntas. Lee la generacion que cada crate declara en su cabecera
--`//! generacion: abuelo`-- y su `[dependencies]`, y dice NO cuando una
generacion depende de otra **mas alta**, o sea cuando el conocimiento sube.

★ Juzga **crates y no ficheros**, y el motivo esta en L7c de
[`META-KERNEL_HARD.md`](../META-KERNEL_HARD.md): en una cadena de llamadas la
dependencia se invierte --`entry.rs`, que es abuelo, hace `use super::dispatch`
y nombra al padre-- asi que un guardian que leyera los `use` habria condenado
codigo correcto el primer dia. Entre crates la relacion esta **declarada**, no
deducida. Hoy: 11 crates etiquetados, 6 aristas juzgadas, todas bajan.

### 6.2 `toolchain/tools/enlaces/enlaces.py` -- las citas

Un indice que envia a un fichero que no existe es peor que no tener indice.

```bash
python toolchain/tools/enlaces/enlaces.py --check
```

Barre **todo el arbol** --no solo `docs/`-- porque los documentos se citan desde
el kernel, desde los `Cargo.toml`, desde `build.ps1` y desde los ejemplos de C.
Corre dentro de `build.ps1`, y si no hay Python avisa y sigue: un portico que no
se puede levantar no debe cerrar la puerta.

**Lo que ya cazo el dia que se escribio**, y es la razon de que exista: catorce
citas rotas que nadie habia visto. Una apuntaba a AVANCES.md dentro de docs/,
cuando ese fichero vive en la raiz, y **no habia resuelto nunca** -- en un
documento cuyo trabajo entero es mandar al lector a otro sitio.

[!] Ese ejemplo va escrito **sin backticks a proposito**. El guardian no sabe
distinguir una cita de la **cita de una cita rota**, y tiene razon: si el
ejemplo tiene forma de ruta, es una ruta. Se cazo a si mismo en el comentario
que lo explica dentro de `build.ps1`.

### ⚠ Lo que hoy le sale en rojo, y no lo puede arreglar solo

```
   platform/drivers/storage/estratos/src/lib.rs:3
   platform/drivers/storage/estratos/src/objects.rs:3   -> platform/services/timeback/ESTRATOS.md
   toolchain/tools/estratos-fmt/src/main.rs:3           -> ESTRATOS.md
```

Las tres citan el **diseno completo de ESTRATOS** con numero de section
(*"section 10, paso 4"*, *"paso 4c del orden de construccion"*), y ese documento
**no esta en el repositorio** -- ni con ese nombre ni con otro. La carpeta
`platform/services/timeback/` tampoco existe: en `platform/services/` solo hay
`cabina-core`.

O el documento existe fuera del repo y hay que traerlo, o hay que corregir las
tres citas. **Lo que no se puede es dejarlo mudo**: tres ficheros de codigo dicen
que su diseno esta escrito en un sitio donde no hay nada.
