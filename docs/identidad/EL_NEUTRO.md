# EL NEUTRO -- lo que el orquestador NO orquesta

> Capitulo de identidad: *por que BMO-X hace esto distinto en vez de copiar*.
> Hermano de [`EL_ORQUESTAL.md`](EL_ORQUESTAL.md), y su contrapeso exacto.
>
> Lo nombro el dueno el **2026-09-07**, hablando de la GPU:
>
> > *"considero que si hablamos de GPU, ese ya no vive en RING 0 ni RING 3.
> > Vive en **Neutro**. Ese mismo es por algo, para facilitar"*
>
> Tenia razon, y la palabra abre un agujero que ya estaba abierto y no tenia
> nombre.

---

## 0. ★★ LA CORRECCION, antes de nada

La intuicion es correcta pero hay que partirla en dos, porque las dos mitades
viven en sitios opuestos:

```text
   el DRIVER del PSP    Ring 0. MMIO, memoria fisica, sin red de seguridad
   el PSP               NEUTRO. Ni Ring 0 ni Ring 3, y no por elegirlo
```

** Confundirlas seria decir que escribir el driver es gratis porque el aparato
es neutro. **No lo es**: el driver paga el precio entero de Ring 0.

---

## 1. Que es NEUTRO, con su definicion y no con su nombre

Un nombre sin definicion es prosa (L0). Asi que:

> **NEUTRO es lo que ejecuta codigo que BMO-X no escribio, no puede leer y no
> puede detener -- y que alcanza la RAM sin pasar por el orquestador.**

Tres condiciones, y hacen falta las tres:

```text
   1. EJECUTA        tiene su propio procesador y su propio programa
   2. ES OPACO       ese programa no se puede leer, ni parar, ni auditar
   3. ALCANZA LA RAM por DMA, y el DMA no obedece a ninguna capability
```

★ Un aparato que solo cumple 1 y 2 --un microcontrolador de teclado, por
ejemplo-- no es neutro: es un periferico. **Lo que lo hace neutro es la
tercera.** El teclado te manda bytes; el neutro te ESCRIBE en la memoria.

---

## 2. El censo: quien es neutro en esta maquina, hoy

No es una lista de futuro. Es lo que hay dentro del Ryzen **ahora mismo**:

| quien | ejecuta lo suyo? | alcanza la RAM? | BMO-X lo cuenta? |
|---|---|---|---|
| el **AHCI** (disco) | si | **si**, DMA | por `find_ahci` |
| la **NIC** (red) | si | **si**, DMA | por `find_nic` |
| el **xHCI** (USB) | si | **si**, DMA | por `find_xhci` |
| la **GPU** y su **PSP** | si, firmware firmado | **si**, y masivamente | ★ desde el 07-09, por `dev/portero.rs` |
| el **microcodigo del CPU** | si | si | no, y no se puede |
| el **SMM** del firmware | si | si | no, y no se puede |

** Los tres primeros ya estaban ahi el primer dia. **El neutro no llego con la
GPU: la GPU solo lo hizo evidente.**

---

## 3. ⚠ Y AQUI ESTA EL PRECIO (L3): la LEY DEL CELO TIENE UN AGUJERO

[`EL_ORQUESTAL.md`](EL_ORQUESTAL.md) dice la frase que define esta casa:

> *"multiplexar es ser generoso; **orquestar es ser CELOSO**. Nada se da, todo
> se presta."*

Y el celo funciona: una tarea de Ring 3 no toca un byte que no se le haya
prestado, y `autoridad.rs` no delega jamas. **Con el neutro no funciona nada de
eso.**

```text
   una tarea de Ring 3   pide, y el orquestador decide
   un aparato neutro     escribe, y el orquestador se entera despues
```

*** Se escribe aqui en voz alta porque es la unica forma de que sea un HECHO
CONOCIDO y no una sorpresa:

> **El orquestador es celoso con Ring 3 y CIEGO con el neutro.**

Lo que cerraria el agujero tiene nombre y no esta puesto: una **IOMMU**, que es
la MMU del lado de los aparatos. Hasta que exista, el celo acaba en el borde de
la RAM que el CPU direcciona.

---

## 4. ★★ Y ESTO NO ES TEORIA: es el caso del 07-09

Tres cosas que parecian tres se explican con esta sola palabra.

### 4.1 La azul de la purga

La purga devuelve marcos al asignador. Un aparato neutro que todavia tenga un
marco programado como destino de DMA **sigue escribiendo en el**, y ese marco ya
es de otro. Es la pista 1.5 de `docs/metal/PRUEBA_EN_METAL_0907.md`, y ahora
tiene categoria: **no es un fallo de la purga, es el limite del celo**.

### 4.2 El xHC que se murio, y su frase exacta

`dev/usb/salud.rs` define el bit del controlador averiado asi:

> *"HSE (bit 2, error de sistema -- **tipicamente un DMA a memoria que no puede
> tocar**)"*

★ Eso es **el neutro quejandose**. Un aparato neutro escribiendo donde ya no
debe, dicho por el propio aparato. La seccion 1.6 de la hoja lo trata como una
pista de USB; con esta palabra se ve que es la misma que la 1.5.

### 4.3 El asignador que podia colgarse

Y la 2.4 cierra el triangulo: `alloc_frame` daba vueltas sin salida con el
cerrojo en la mano. Tres sintomas --teclado mudo, azul, maquina muerta-- **con
una sola frontera detras**.

---

## 5. Por que el dueno dijo "para facilitar", y tenia razon

Su frase fue *"ese mismo es por algo, para facilitar"*. Lo que facilita, dicho
con precision:

```text
   [x] deja de haber que decidir en que anillo "va" un aparato.
       No va en ninguno, y eso es una respuesta, no un hueco
   [x] el driver deja de heredar la pregunta: el driver es Ring 0 y punto
   [x] el agujero del celo pasa a ser CONTABLE en vez de sorpresa:
       `placa=funciones:interesan:sincodigo` ya cuenta a los neutros
   [x] y da el criterio para el dia de la IOMMU: lo que hay que meter
       detras de ella es exactamente esta lista
```

⚠ **Lo que NO facilita**: no quita ni una linea de trabajo del driver, y no hace
mas seguro nada. Nombrar un agujero no lo tapa. Lo unico que compra es que deje
de confundirse con otros tres.

---

## 6. Su juez, porque un eje sin juez es prosa (L0)

```text
   quien juzga     `bmo-mmio-juicio`, y su veto `PisaRam`
   que juzga       que un rango cedido a Ring 3 no pise RAM usable
   su numero       `placa=...:sincodigo` -- aparatos neutros sin codigo
   su hueco        NO juzga el DMA que un neutro hace por su cuenta,
                   porque no hay con que. Ese es el trabajo de la IOMMU
```

★ **El juez existe y esta cableado, pero juzga la mitad de arriba** -- lo que
BMO-X cede a Ring 3. La mitad de abajo --lo que el aparato se toma-- no tiene
juez, y esa frase es el estado real del sistema, escrita para que no haya que
descubrirla otra vez con una pantalla azul delante.

---

## 7. Lo que este documento NO afirma

```text
   [ ] no dice que el neutro sea inseguro. Dice que NO ESTA VIGILADO,
       que es una afirmacion mas pequena y comprobable
   [ ] no propone una IOMMU. La nombra como lo unico que cerraria esto
   [ ] no cambia ni una linea de codigo. Es una CATEGORIA, no un mecanismo
   [ ] y no aparece en el ABI: Ring 3 no tiene que saber que existe
```

> El neutro no es un anillo nuevo. Es el nombre de donde acaba la ley.
