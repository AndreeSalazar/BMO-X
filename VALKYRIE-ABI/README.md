# VALKYRIE-ABI (V-ABI)

> **El estandar de la superficie de BMO-X.** Lo que se le PROMETE a quien
> escriba una app, con un nombre, un numero de version y cuatro jueces que lo
> cobran en cada build.
>
> Creada el **2026-09-06**, cuando el dueno pregunto si habia que inventar un
> POSIX propio. La respuesta fue que no: ya estaba construido y le faltaban dos
> cosas que no son codigo -- **un nombre y un numero**.

---

## 0. ** POR QUE ESTA CARPETA NO TIENE ANILLO

Es la pregunta que ordena todo lo demas, y tiene una respuesta exacta:

> **Un anillo es un nivel de privilegio del CPU. V-ABI no tiene ni una
> instruccion en la maquina de destino.**

Nada de esta carpeta viaja dentro de `BOOTX64.EFI`. No se carga, no se mapea, no
consume un marco. Corre en **el anfitrion**, en tiempo de build, **antes de que
la maquina arranque** -- y por eso no esta ni en Ring 0 ni en Ring 3: no esta en
la maquina.

### Y no es una casualidad de implementacion: es la unica posicion valida

```text
   Ring 0     IMPLEMENTA   el kernel escribe la operacion y su numero
   Ring 3     CONSUME      la app la pide por ese numero, a traves de REX
   V-ABI      JUZGA        que los dos digan EL MISMO numero
```

Los dos lados de una frontera no pueden certificarla:

* **Si viviera en Ring 0**, el kernel seria juez y parte. Un guardian dentro del
  acusado dice que si el dia que el acusado cambie.
* **Si viviera en Ring 3**, una app estaria certificando al kernel. Lo mismo, del
  otro lado, y ademas con menos privilegio del que necesita para mirar.

Fuera de los dos, en una maquina que solo tiene dos anillos, **significa fuera de
la maquina**. Por eso el estandar es un fichero de texto y un juez en Python, y
no un modulo del kernel.

### La consecuencia practica, que es la que se cobra

**Se puede romper V-ABI sin arrancar.** Es la unica ley de este arbol que se
comprueba entera sin pisar el Ryzen, y por eso su deuda se caza en **69 segundos**
y no en un viaje al metal. Compara con lo que cuesta comprobar cualquier cosa de
Ring 0 -- una hoja de pruebas, una tarde, y una foto.

---

## 1. ** LO QUE ESTA CARPETA NO PUEDE HACER, DICHO ANTES QUE LO QUE SI

L3 dice que toda regla trae su sacrificio. Este es el de V-ABI, y es grande:

> **El estandar juzga lo que esta ESCRITO, nunca lo que CORRE.**

R13 demuestra que REX y el ABI dicen el mismo numero para `PANTALLA_RECLAMAR`.
**No demuestra que el kernel reclame la pantalla.** Un sello verde con un kernel
que no hace nada es perfectamente posible, y no seria un fallo del sello: seria
el sello contestando la pregunta que le toca.

Quien comprueba que la operacion HACE lo que dice es otro: el banco de pruebas,
las hojas de `docs/metal/` y el Ryzen. **V-ABI cubre la mitad barata**, y decirlo
aqui arriba es lo que impide que el `Compliant` del build se lea como mas de lo
que es.

---

## 2. EL MAPA DE ESTA CARPETA -- una categoria por fichero

| fichero | categoria | que contesta |
|---|---|---|
| **`VERSION.txt`** | la version | que numero de estandar es este, y por que se llama VALKYRIE |
| **`LEY.md`** | la ley | que dice cada una de R13-R16, que sacrifica y como dice que NO |
| **`FRONTERA.txt`** | el alcance | que NO es superficie de una app, por prefijo y con su dueno |
| **`ESPEJO.txt`** | la prueba | las 98 constantes que REX y el ABI escriben las dos veces |
| **`COBERTURA.txt`** | el trinquete | cuanto del ABI que es de app tiene cabecera. Solo sube |

Los cinco son **datos sellados o prosa**. Ninguno se ejecuta.

---

## 3. LO QUE NO ESTA AQUI, Y POR QUE

**Los jueces se quedaron en `toolchain/tools/contrato/`**, y el corte no es
estetico. `contrato.py` y sus tres modulos juzgan **R1 a R17**, y solo R13-R16
son este estandar:

```text
   R1  R2         la taxonomia de kinds: kernel contra ABI
   R3  R4  R5     las operaciones, y que no se repitan
   R6  R7  R8     L6e, L6f, L6g -- el coste, el riesgo, el juez nombrado
   R9  R10        los carriles y el semaforo de RING 0
   R11 R12        el semaforo y los carriles de REX -- modularidad, no superficie
   ----------------------------------------------------------------
   R13 R14 R15 R16   V-ABI                      <- solo estas cuatro
   ----------------------------------------------------------------
   R17            el semaforo de `fundamentals`
```

Traerse los jueces enteros diria que R6 --el coste declarado de un fichero de
Ring 0-- forma parte de lo que se le promete a un tercero. **Es falso**, y una
carpeta que afirma de mas es peor que no tener carpeta.

```text
   VALKYRIE-ABI/   lo que se PROMETE      version, ley, frontera, espejo, cobertura
   contrato/       lo que lo COMPRUEBA    los jueces, que juzgan R1-R17
```

** Y por eso esta carpeta vive **en la raiz** y no dentro de `toolchain/`: un
estandar que vive dentro de la herramienta que lo comprueba se lee como una
salida de esa herramienta. Es al reves -- **el juez sirve al estandar**, y la
posicion en el arbol tiene que decirlo sin que nadie lo explique.

### Tampoco esta REX, y esa es una ley vieja

REX --las diez cabeceras `<bmo/...>`-- se queda en
`toolchain/forge/sem-asm/tables/bmo/`. `META-SDK_HARD.md` seccion 6 ya lo escribio:
`tables/` es **la puerta de los terceros**, y sacar REX a una carpeta bonita le
quitaria a un creador la capacidad de tapar una pieza con `$BMO_MODS` sin
bifurcar el repo. **El estandar se muda; la implementacion no.**

---

## 4. EL SELLO

Cuando R13-R16 pasan y `VERSION.txt` dice una version, el build lo imprime lo
ultimo:

```text
   BMO-X Engine [V-ABI v1.0 / R13-R16 Compliant]
   98 pareja(s) ABI<->REX, 98 de 196 constantes de app, 0 numero(s) inventados por una app
```

La segunda linea es lo que hace falsable a la primera. **Un `Compliant` sin
cifras al lado es un eslogan**, y L0 dice que un eje sin juez es prosa.

El sello **se gana**, y las cuatro ramas fallan distinto a proposito:

| situacion | que sale |
|---|---|
| R13-R16 limpias | el sello, con sus cifras |
| R13-R16 con quejas | el build muere antes y no se llega |
| falta `VERSION.txt`, o dice `1.0-rc` | `V-ABI SIN SELLAR`, en amarillo |
| falta `python` | `sin python no hay nada que sellar`, en amarillo |

** La ultima es la que importa. Sin `python` el contrato **no corre y el build
termina en verde igual**: es la unica tarde en la que un sello mentiroso se
colaria en una captura de pantalla. Ahora lo dice.

---

## 5. [!] V-ABI NO ES `BMO_ABI_VERSION`

El arbol tiene los dos numeros, y confundirlos cuesta caro:

```text
   BMO_ABI_VERSION (2,0)   platform/abi/bmo-abi/src/lib.rs
                           el BINARIO -- contra que enlaza un `.bex`, y que
                           major incompatible lo rechaza al cargar

   V-ABI v1.0              VALKYRIE-ABI/VERSION.txt
                           el ESTANDAR -- que superficie se le promete a un
                           tercero, y con que suite se comprueba
```

Misma separacion que POSIX.1-2017 --la edicion del estandar-- tiene con el
`soname` de la libc que lo implementa. **Que uno vaya por 2.0 y el otro arranque
en 1.0 no es una incoherencia: es que uno lleva contando desde antes.**

---

## 6. COMO SE MUEVE LA VERSION

```text
   1.0 -> 1.1   entra una cabecera nueva en REX; la cobertura sube. Aditivo,
                y no rompe a nadie.
   1.x -> 2.0   se BORRA o cambia de significado algo que ya se publicaba.
                Eso rompe a un tercero, y por eso cuesta un major.
```

v1.0 congela **la forma, no los valores**, y no promete que la cabecera sea
BUENA: mide que exista y que cuadre.

---

Ver [`LEY.md`](LEY.md) (las cuatro reglas, una a una),
[`META-SDK_HARD.md`](../META-SDK_HARD.md) (que es REX y que no entra en el),
[`META-KERNEL_HARD.md`](../META-KERNEL_HARD.md) (L0, L1, L3 y L4, que son las
que obligan a que esto tenga jueces y no prosa) y
[`EL_FUERO.md`](../EL_FUERO.md) (el reparto entero: que se concede y que se
exige).
