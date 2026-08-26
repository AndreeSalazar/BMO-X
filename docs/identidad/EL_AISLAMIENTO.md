# EL AISLAMIENTO -- por que una app que revienta no es lo mismo que una pantalla azul

> Escrito el **2026-08-26**. Tres preguntas del dueno, y la primera es la que
> manda:
>
> 1. *"necesito saber por que me sorprendio la pantalla azul"*
> 2. *"no es lo mismo tener una vulnerabilidad a que mis apps fallen asi, no?"*
> 3. *"cuales son los elementos que son LISTAS de raiz que no se pueden
>    modificar, y cuales si"*
>
> La segunda pregunta ya trae la respuesta dentro: **no, no es lo mismo.** Este
> documento existe para que la diferencia se pueda decir con precision, porque
> mientras las dos cosas se llamen *"fallo"* no hay forma de saber cual de las
> dos se acaba de ver.

---

# 1. LAS TRES COSAS QUE SE PARECEN Y NO LO SON

```text
   1. UNA APP FALLA          la tarea muere. BMO sigue vivo
                             -> el aislamiento FUNCIONANDO

   2. EL KERNEL FALLA        pantalla azul, y a los 20 segundos reinicia
                             -> aqui no hay aislamiento POSIBLE

   3. UNA VULNERABILIDAD     alguien de fuera decide lo que hace tu maquina
                             -> otra cosa completamente
```

## 1.1 -- Los dos primeros son FIABILIDAD. El tercero es AUTORIDAD

Y esa es la frase que separa las tres:

> **Un fallo es que tu codigo se equivoque. Una vulnerabilidad es que el codigo
> de OTRO decida.**

Son ejes distintos y no se cambian el uno por el otro:

| | se puede caer | se puede colar alguien |
|---|---|---|
| una app que revienta cada minuto | si | **no necesariamente** |
| un kernel que lleva un ano sin caerse | no | **puede estar lleno de agujeros** |

★★ **La calculadora se murio y BMO sigue vivo: eso fue una app fallando y el
aislamiento haciendo su trabajo.** Es la linea de CABINA:

```text
   FAULT ring3: CPL3: tarea eliminada, BMO sigue vivo  =40010A3
```

**Ni una sola de las tres veces hubo vulnerabilidad.** No habia bytes de nadie
de fuera en juego: era la memoria de BMO, leida por BMO.

---

# 2. LA PANTALLA AZUL: QUE ES, EXACTAMENTE

No es una metafora ni un homenaje. Es una funcion de este arbol:
`plat/faults.rs::pantalla_de_fallo`.

```rust
   const FALLO_FONDO: u32 = 0x0011_3A6E;   // el azul de BMO
   const FALLO_SEGUNDOS: u64 = 20;         // y luego REINICIA
```

Y lo que escribe en pantalla es literalmente la respuesta a la pregunta:

> **"A Ring 0 fault cannot be isolated: the kernel is the floor everything else
> stands on."**

## 2.1 -- Solo aparece por UNA razon

**Un fallo en Ring 0.** No hay ningun otro camino que la pinte. Si sale, el
procesador estaba ejecutando codigo del kernel cuando algo se rompio.

*** Y por eso no se puede aislar: **aislar es que alguien de fuera te recoja.**
Cuando el que falla ES el que recoge, no queda nadie por encima. Un `#GP` en el
manejador de faults no tiene a quien avisar.

```text
   una app falla   ->  el KERNEL la mata y sigue        (hay quien recoja)
   el kernel falla ->  no hay nadie encima              (no lo hay)
```

## 2.2 -- Y por que reinicia en vez de quedarse quieta

Decision escrita en el propio fichero: *un kernel congelado obliga a alguien a
levantarse y pulsar el boton; y si pasa mientras nadie mira, la maquina se queda
muerta hasta que alguien la encuentre.* Los 20 segundos son para poder
**fotografiarla**, que aqui la foto es el depurador.

---

# 3. POR QUE SALIO ESTA VEZ, Y ES LO QUE MAS SORPRENDE

★★ **La app murio BIEN. Lo que se cayo fue el enterrador.**

La cadena, en orden:

```text
   1. `calcgui.bex` salta donde no debe            <- fallo de la APP
   2. el kernel lo caza, mata la tarea y lo apunta <- el AISLAMIENTO funciona
   3. el kernel desmonta la memoria del muerto     <- `destroy_address_space`
   4. ahi encuentra una entrada de tabla corrupta
   5. calcula una direccion NO CANONICA con ella
   6. #GP en Ring 0                                <- LA PANTALLA AZUL
```

*** El paso 2 es el sistema haciendo exactamente lo que promete. **El paso 6 es
un fallo de fiabilidad en el codigo de limpieza**, y no tiene nada que ver ni
con la calculadora ni con COBOL ni con una app maliciosa.

> Un kernel que se cae desmontando a un muerto no deja autopsia: se lleva por
> delante al que la iba a escribir.

## 3.1 -- Lo que se hizo con eso (y ya esta en el arbol)

| commit | que cambio |
|---|---|
| `c41b44e8` | los cuatro niveles del recorrido comprueban que la direccion sea alcanzable **antes** de usarla. Deja de matar y **dice el nivel y el valor** |
| `2c6eb01e` | ese valor no cabia en la fila de 80 columnas y salia cortado -- ahora el numero no cede nunca |
| `51df22b6` | y ademas dice **en que tabla y en que casilla**, que es lo que separa "el marco no es una tabla" de "la entrada esta mal escrita" |

★ Resultado medido en el Ryzen: **la misma situacion que antes daba pantalla
azul ahora da tres lineas rojas y la maquina sigue en pie.** Eso es una
degradacion elegante, no una reparacion: **la causa de la corrupcion sigue sin
encontrarse**, y el proximo arranque trae los numeros que la senalan.

## 3.2 -- [!] Y la coincidencia incomoda que hay que decir

El censo de [`RING3_MAESTRO.md`](../maestro/RING3_MAESTRO.md) partio el kernel en
tres clases y llamo **clase A** a la que no puede bajar a Ring 3: paginacion,
traps, cambio de contexto.

**El unico `#GP` de Ring 0 que se ha pagado de verdad esta en clase A.** Bajar
drivers a Ring 3 --que es lo correcto y lo que se va a hacer-- **no habria
evitado esta pantalla azul.** La sorpresa vino de donde no se estaba mirando, y
un plan que no diga eso es un plan que se vende a si mismo.

---

# 4. LOS SIETE MUROS: QUE SEPARA RING 0 DE RING 3 EN x86-64

Esto es lo que el dueno pidio analizar. **Ninguno es una convencion de
software**: los siete los hace cumplir el silicio, y cada uno esta encendido en
un sitio concreto de este arbol.

| # | muro | que impide | donde vive aqui |
|---|---|---|---|
| 1 | **CPL = 3** | ejecutar instrucciones privilegiadas (`lgdt`, `mov cr3`, `hlt`, `in`/`out`) | `iretq` a un `CS` con RPL=3 -- `plat/trap.rs` |
| 2 | **Bit U/S de la pagina** | que Ring 3 lea o escriba una pagina del kernel | `PTE_USER` en `mm/vmm.rs` |
| 3 | **NX + W^X** | ejecutar datos | `EFER.NXE` en `s1_cpu` + `PTE_NX` en `vmm.rs` |
| 4 | **SMEP** | que **Ring 0** ejecute una pagina de Ring 3 | `CR4.SMEP`, `s1_cpu` |
| 5 | **SMAP** | que **Ring 0** lea o escriba memoria de Ring 3 sin querer | `CR4.SMAP`; solo la autopsia lo levanta, con `stac`/`clac` |
| 6 | **UMIP** | que Ring 3 lea `SGDT`/`SIDT`/`SLDT`/`STR` y aprenda direcciones del kernel | `CR4.UMIP`, `s1_cpu` |
| 7 | **La puerta unica** | entrar al kernel por cualquier sitio que no sea `syscall` | `syscall/entry.rs`, y `TSS.RSP0` da la pila |

★★ **Los cuatro que empiezan por el 3 son los que casi nadie enciende, y este
sistema los tiene los cuatro.** Se confirmaron en metal el 25-08 con la orden
`ext`: cuatro filas en `Yes`.

★ Fijate en lo que protegen el 4 y el 5, porque es al reves de lo que uno
espera: **no protegen al kernel de la app. Protegen al kernel DE SI MISMO.**
SMEP y SMAP existen porque la forma clasica de convertir un bug del kernel en
una vulnerabilidad es hacer que el kernel salte a codigo del atacante o lea sus
datos creyendolos suyos.

## 4.1 -- Y el octavo, que no es del silicio: las CAPABILITIES

Los siete de arriba dicen **que memoria** puede tocar cada uno. No dicen nada de
**que puede pedir**.

```text
   los 7 muros    aislan la MEMORIA      -> lo hace el procesador
   capabilities   reparten la AUTORIDAD  -> lo hace BMO
```

Una app con todos los muros puestos podria aun asi pedir *"reinicia la maquina"*
si el sistema se lo permitiera. Por eso existe C4 --la autoridad-- y por eso
`sonda.bex` prueba que un `.bex` lanzado por el escritorio **no puede lanzar otro
ni reiniciar**.

## 4.2 -- ⚠ Y LO QUE NINGUNO DE LOS OCHO PARA

Esta es la parte honesta, y es la que responde a *"que las apps no me
sorprendan"*:

| | quien lo para | estado |
|---|---|---|
| una app que lee la memoria de otra app | muro 2 | ✅ |
| una app que escribe en el kernel | muro 2 | ✅ |
| una app que ejecuta sus propios datos | muro 3 | ✅ |
| una app que se cuelga en un bucle | **nadie todavia** | ⚠ no hay expulsion por tiempo real probada |
| una app que pide memoria sin parar | el presupuesto (`syscall/presupuesto.rs`) | parcial |
| **una TARJETA que escribe donde no debe (DMA)** | **NADIE. Hace falta una IOMMU** | ⛔ |
| un bug del propio kernel | **nadie, por definicion** | ⛔ **esto es la pantalla azul** |

*** **Las dos ultimas filas son las unicas que pueden dar una pantalla azul**, y
por eso son las que importan. Todo lo demas ya esta cubierto por el silicio.

---

# 5. LAS LISTAS DE RAIZ: QUE NO SE PUEDE MOVER, Y QUE SI

La pregunta del dueno era *"cuales son los elementos que son LISTAS de raiz que
no se pueden modificar y cuales si, para poder mejorar sin romper"*.

## 5.1 -- CONGELADO. Tocar esto rompe todo lo que ya existe

| que | donde | por que no se mueve |
|---|---|---|
| **Las dos puertas** `INVOKE` y `WAIT` | `platform/abi/bmo-abi/src/syscalls/` | son la forma del sistema. Un tercer syscall existio y **se retiro** el 10-08 |
| **Los numeros de operacion ya asignados** | `surface/*.rs` | un numero retirado **no se recicla**: un binario viejo que lo llame falla diciendolo |
| **El formato `BEF1`** | `bmo-bex-gate` -- `MAGIC`, `CABECERA=48`, `ENTRADA=48` | lo lee el cargador desde el disco |
| **`BMO_ABI_VERSION` mayor** | `bmo-abi/src/lib.rs` | subirlo declara incompatibilidad **a proposito** |
| **Las siete `R-APP`** | `META-APP_HARD.md` | son el contrato que viaja con la app |

## 5.2 -- CRECE SIN ROMPER. Aqui es donde se "mejora"

| que | la regla que lo gobierna |
|---|---|
| **operaciones nuevas** sobre capabilities | `R-REX3`: *comodidad es cabecera, autoridad es operacion* |
| **kinds de capability nuevos** (`KIND_MMIO`, `KIND_RED`...) | no tocan las puertas |
| **secciones nuevas del `.bex`** | el header ya lo dice: *"una seccion que no entiendo es data inerte"* |
| **cabeceras de REX** | comodidad; compilan hacia dentro del `.bex` |
| **la version MENOR del ABI** | declarada aditiva |

★★ **Y ese es el mecanismo entero de "mejorar sin romper": lo aditivo va en el
menor, lo que rompe va en el mayor, y lo retirado no se recicla.** No hace falta
inventar nada mas.

## 5.3 -- NO ES LEY, aunque lo parezca

★ Distincion que ya esta en [`EL_FUERO.md`](../../EL_FUERO.md) 2.6b y conviene
repetir porque se lee al reves con facilidad:

```text
   las reglas del CONTRATO   viajan: sin ellas tu app NO CORRE
   las reglas de la CASA     no viajan: son como se mantiene ESTE arbol
```

El limite de 1.000 lineas por modulo (L6a), el ASCII, la herencia de crates: **son
de la casa.** Los guardianes leen `git ls-files` de este repositorio. **La app de
un tercero no esta aqui, asi que no la miran nunca.**

---

# 6. LA COMPATIBILIDAD: si, es el eslabon

> *"y la parte MAS importante COMPATIBILIDAD = ese mismo es el eslabon, no?"*

**Si**, y conviene decir por que con precision, porque es una consecuencia de
todo lo anterior y no un tema aparte.

## 6.1 -- Lo que una empresa compra no es el codigo: es la promesa

Una empresa que construye sobre BMO-X no esta comprando 70.000 lineas de kernel.
Esta comprando **una lista de cosas que prometes no romper**. Y la aritmetica es
brutal:

```text
   una superficie GRANDE y estable   -> caro de mantener, facil de adoptar
   una superficie PEQUENA y estable  -> barato de mantener, hay que aprender
   una superficie GRANDE e inestable -> nadie construye encima
   una superficie PEQUENA e inestable-> ni eso
```

★★ **BMO-X eligio la segunda, y esa eleccion ES el producto.** Dos puertas
congeladas y 93 operaciones aditivas es una promesa que cabe en una pagina --
y una promesa que cabe en una pagina es una promesa que se puede cumplir diez
anos.

## 6.2 -- Y la compatibilidad **se comprueba en la puerta**, no se confia

El cargador ya rechaza, con nombre propio, un `.bex` que:

```text
   OtraVersionDelAbi                 no es de esta version
   ExtensionDeCpuQueNoSePreserva     pide un bit del CPU que el kernel no guarda
                                     en un cambio de contexto
   PideAlgoQueNadieImplementa        una bandera que este sistema no tiene
   NoEsEjecutable / TablaFueraDelFichero / DemasiadasSecciones
```

*** La fila del CPU es la mejor de esa lista, y merece leerse dos veces: *"un
bit que no conozco es una parte del estado del procesador que no se que existe y
que por tanto NO voy a preservar."* **Eso convierte una corrupcion silenciosa en
un "no" con nombre**, que es la definicion practica de compatibilidad honesta.

## 6.3 -- ⚠ Y UNA GRIETA ENCONTRADA HOY, en ese mismo eslabon

`bmo-abi` declara la regla:

```rust
   /// Major versions are incompatible; minor versions are additive.
   pub const fn supports_abi(required: (u8, u8)) -> bool { ... }
```

Y el cargador que de verdad decide --`bmo-bex-gate`, que **no puede depender de
`bmo-abi`** porque lo consumen dos mundos que no se ven-- implementa **otra**:

```rust
   if !((abi_mayor == 1 || abi_mayor == 2) && abi_menor == 0) {
       return Err(Falta::OtraVersionDelAbi);
   }
```

*** **`abi_menor == 0` no es aditivo: es exacto.** El dia que el ABI suba a
`2.1` --el primer dia que se "mejore" de la forma que la seccion 5.2 declara
segura-- un `.bex` compilado contra `2.1` sera **rechazado por el cargador**
mientras el propio ABI dice que deberia entrar.

★ La grieta no ha hecho dano todavia porque **nadie ha subido el menor nunca**.
Es exactamente el perfil de fallo que este arbol ya conoce: dos sitios que dicen
lo mismo, uno se queda atras, y **el dia que se separan no lo nota nadie**.

[!] Y no se arregla haciendo que el gate dependa de `bmo-abi` --su cero
dependencias es el motivo por el que puede vivir en Ring 0-- sino **declarando
el menor maximo como constante suya y comprobando que las dos coinciden**. Es la
misma forma que el guardian del contrato de Ring 0 que ya corre en `build.ps1`.

---

# 7. EL RESUMEN, EN CUATRO LINEAS

```text
   una app falla         el aislamiento funciona. Es lo que se vio, tres veces
   la pantalla azul      el kernel fallo LIMPIANDO al muerto. No fue la app
   una vulnerabilidad    no hubo: no habia bytes de nadie de fuera en juego
   la compatibilidad     es el eslabon, y hoy tiene una grieta de una linea
```
