# PYTHON MAESTRO -- que hace falta para que Python corra en BMO-X

> Escrito el **2026-08-16**, antes de una sola linea de codigo, con el mismo
> criterio que `RED_MAESTRO.md` y `AUDIO_MAESTRO.md`: su trabajo es que esta
> investigacion no haya que reconstruirla.
>
> Pedido por Eddi: *"reconstruir Python... investigar TODOS los datos originales
> de CPython... pero haciendo pureza para BMO-X porque uso 2 syscall... la
> portabilidad es el motivo... BMO-X debo hacer crecer mas en superficies"*.

---

## 0. La pregunta, y la precision que necesita

La intuicion de Eddi tiene **tres partes, y dos son correctas**:

1. ✅ **"La portabilidad es el motivo, y C solo es su lado."** Correcto, y ya
   estaba medido en `QUE_DESBLOQUEA.md`: *lo que desbloquea aplicaciones es la
   superficie del sistema, no el lenguaje*. Python lo lleva mas lejos que C
   porque **su runtime le pide al sistema cosas que C nunca pidio**.
2. ✅ **"BEF enmascara y no le importa el lenguaje."** Correcto, y mas de lo que
   parece -- ver la seccion 5, donde resulta que **la mitad del trabajo de
   empaquetar Python ya esta hecha y nadie se dio cuenta**.
3. ⚠ **"Los 2 syscalls hacen a Python puro."** Aqui hay que ser exacto, porque
   de esto depende no perder meses: **los 2 syscalls no son el problema de
   Python, ni son la parte dificil.** Un `.bex` de Python llamaria exactamente
   a lo mismo que uno de C. Lo dice ya `PROPOSITO.md` sobre C++:
   *"la dificultad esta dentro del lenguaje"*.

   Lo que Python rompe no es la superficie: es **el supuesto de que el
   compilador sabe el tamano y la forma de cada valor**. C, COBOL, Ada y C++ son
   estaticos. `a + b` en Python no se puede convertir en un `add` sin saber en
   ejecucion que son `a` y `b`. Por eso **Python no es un quinto frontend sobre
   el mismo backend**: es un frontend **mas un runtime**, y el runtime es el
   trabajo.

### ★★★ "Hay que abrir un syscall para Python?" -- NO. Cero.

La pregunta salio el 16-08 y va aqui arriba porque va a volver.

Se confunden dos cosas que el diseno separa a proposito:

| | crece? |
|---|---|
| **Las 2 puertas** (`INVOKE`, `WAIT`) | **congeladas.** Python no las toca |
| **Las operaciones** dentro de `INVOKE` (~49 hoy) | crecen por **filas de tabla** -- para eso existen |

`OP_INFO` esta disenado literalmente para esto, y lo dice su propio comentario:
*"Dos operaciones y una TABLA de campos, en vez de una operacion por dato: asi
anadir 'cuantos programas se han lanzado' es **una fila**, no un numero de
syscall nuevo."*

**El presupuesto entero de Python contra la superficie:**

- **Syscalls nuevos: 0.**
- **Operaciones nuevas: 1** -- `stat` de un fichero FAT32 desde Ring 3.
  (La fecha real se creia que faltaba y **ya estaba**: ver la tabla de la
  seccion 2.)
- **Intrinsecos nuevos: 1** -- y ★ **`RDRAND` no es privilegiado**: lo ejecuta
  Ring 3 directamente. Es **una fila de `intrinsics.toml` y CERO kernel**. Ni
  siquiera pasa por la puerta.
- **El tope de 4 bloques: posiblemente 0 cambios** -- una arena grande y el
  monton que ya existe.

**Total: dos filas de tabla.**

### Y la prueba de fuego que evita lo "exclusivo de Python"

> **Si algo es EXCLUSIVO de Python, no entra en el sistema. Si lo piden otros
> tres, es que ya faltaba.**

- **La fecha real**: ✅ **ya existe** -- `INFO_FECHA`. Se creia que faltaba, y
  esa es justo la clase de error que un censo evita: la piden `os.stat`,
  ESTRATOS, CABINA y cualquier cierre de COBOL, y **ya estaban servidos**.
- **La entropia**: la piden la firma de verdad (hoy es integridad y no
  autenticidad, porque no hay clave), ESTRATOS y la red.
- **`stat` / listar desde Ring 3**: lo pide el lanzador del escritorio, que hoy
  lee el paquete a mano.

Ni una es de Python. **Python es el que las encontro**, igual que una sonda
encontro `<strings.h>`.

★★ **Y la medida PROTEGIO la superficie**: si la puerta hubiera costado 50
ciclos, la tentacion habria sido meter el runtime por `INVOKE` -- y eso si
habrian sido decenas de operaciones nuevas, exclusivas de Python, dentro del
kernel. **Los 2.570 ciclos son lo que hace que eso no se pueda ni plantear.**

★★★ **BMO-X no tiene que saber que Python existe.** Igual que no sabe que existe
COBOL: carga un `.bex` y ese `.bex` llama a las dos puertas. Es el principio de
Devour_System -- *"el kernel nunca sabe que existio lo ajeno"*.

---

## 1. Que es CPython de verdad -- el censo

CPython no es un interprete: son **cinco subsistemas** que la gente confunde con
uno solo. El reparto del arbol:

| Carpeta | Que es | Peso |
|---|---|---|
| `Objects/` | **el modelo de objetos** -- dict, list, str, int, type... | ~60 ficheros `.c`, el mas gordo `unicodeobject.c` (~16k lineas) |
| `Python/` | el nucleo: `ceval.c` (el bucle), `compile.c`, `import.c`, `marshal.c`, `dtoa.c` | ~50 ficheros |
| `Parser/` | el parser PEG. `parser.c` esta **generado a maquina** (~40k lineas) | ~15 ficheros |
| `Modules/` | la libreria estandar en C (`_io`, `_sre`, `posixmodule`, `math`...) | ~200 ficheros |
| `Lib/` | la libreria estandar en **Python** | ~250 modulos utiles + una suite de tests mayor que todo lo anterior |

⚠ **Los numeros de arriba son de orden de magnitud, no medidos aqui.** La cifra
exacta es un `wc -l` sobre el tarball, y esa es la primera tarea de la fase 0 --
igual que `c-gen` mide la brecha de C con sondas en vez de opinar sobre ella.

### Lo que hay que entender de dentro, y por que importa aqui

- **`PyObject` son 16 bytes**: un contador de referencias y un puntero al tipo.
  *Todo* valor de Python es un puntero a uno de esos en el monton. Un `int`
  pequeno tambien. Esto decide el consumo de memoria del interprete entero.
- **`PyTypeObject` es una tabla de punteros a funcion** (`tp_dealloc`,
  `tp_getattro`, `tp_call`, `tp_iternext`, mas las subtablas de numero /
  secuencia / mapa). El despacho dinamico es *leer una ranura y llamar*.
  ★ **Es exactamente la vtabla que BMO C++ ya sabe emitir**, y por eso el paso 5
  de C++ (`d24b96c1`) es mas relevante para Python de lo que parecia.
- **Dos recolectores, no uno**: conteo de referencias (el principal) **mas** un
  recolector de ciclos generacional (`gcmodule.c`, 3 generaciones) para los
  contenedores. Sin el segundo, `a.b = a` fuga para siempre.
- **`obmalloc.c`**: arenas de 1 MiB pedidas por `mmap`, divididas en pools de
  4 KiB, divididos en bloques de 8..512 bytes por clase de tamano. Por encima de
  512 bytes cae a `malloc`.
  ★★ **Y aqui esta la primera buena noticia del documento**: la **PEP 445**
  (`PyMem_SetAllocator`) permite **sustituir el asignador entero desde fuera,
  sin tocar una linea de CPython**. O sea que el monton de BMO
  (`<bmo/monton.h>`, escalon 1 de `LA_RAM.md`) se enchufa por un hueco que
  CPython dejo a proposito.
- **El bucle**: desde 3.12 el interprete se **genera** desde `Python/bytecodes.c`
  con `Tools/cases_generator`. Y desde 3.11 hay *interprete adaptativo
  especializante* (PEP 659) con caches en linea dentro del propio bytecode.
  ★ El despacho usa `computed goto` (`&&etiqueta`, extension de GCC) **cuando el
  compilador la tiene, y cae a un `switch` cuando no**. BMO C no la tiene, y no
  hace falta: el camino del `switch` es codigo soportado de CPython, no un
  parche.
- **`importlib` viene CONGELADO dentro del binario** (`Python/frozen_modules/`):
  el arranque del sistema de importacion es bytecode empotrado como arrays de C.
  ★ Consecuencia enorme para BMO: **un Python minimo puede arrancar sin tocar el
  disco ni una vez**.
- **El hash de las cadenas es SipHash con semilla aleatoria**, sacada del sistema
  al arrancar. ★ Pero `PYTHONHASHSEED=0` la desactiva -- o sea que **la falta de
  entropia no bloquea el dia uno**, solo hay que decirlo.
- **`Python/dtoa.c`** (el algoritmo de David Gay) hace `repr(float)` y
  `float("0.1")` exactos. Es **autocontenido**: solo pide dobles IEEE754 y
  enteros de 64 bits correctos. Los dos los tiene BMO C.
- **`--without-threads` se ELIMINO en 3.7.** La API `PyThread_*` tiene que
  existir. Con un solo hilo puede ser trivial, pero hay que escribirla:
  `Python/thread_bmo.h`, hermano de `thread_pthread.h`.

### ★★ El hallazgo que ordena el plan entero

**La frontera con el sistema operativo vive en dos ficheros y medio.**

```text
   Modules/posixmodule.c    ~15k lineas   os.open/read/stat/listdir/...
   Python/fileutils.c                     rutas, codificacion, descriptores
   Python/thread_*.h                      hilos y candados
```

Todo lo demas de `Objects/` y `Python/` es C portatil que no sabe donde corre.
O sea que **"portar CPython" no es tocar 600 ficheros: es escribir uno
(`bmomodule.c`), un cabecero de hilos, y conseguir que un compilador se coma el
resto**. Eso cambia la forma del problema por completo.

---

## 2. Lo que CPython le pide al SISTEMA -- la tabla que decide

Esta es la respuesta util a *"que le falta a BMO-X"*, y sirva o no Python, **cada
hueco de esta tabla desbloquea mas cosas que Python**.

| CPython pide | Por donde entra | BMO-X hoy | Veredicto |
|---|---|---|---|
| abrir / leer / escribir / seek / cerrar | `posixmodule` | `TASK_OP_ARCHIVO_ABRIR/CREAR` + `ARCH_OP_LEER_EN` / `ESCRIBIR_DE` / `SALTAR` / `TAMANO` / `CERRAR` | 🟢 **ya esta** |
| stdin / stdout / stderr | `_io` | `TASK_OP_CONSOLE_WRITE` / `CONSOLE_READ` | 🟢 ya esta |
| `stat` (tamano, tipo, fecha) | `os.stat`, el importador | `ES_NODO_HIJO_BYTES` / `_TIPO` -- **solo sobre ESTRATOS** | 🟡 no hay `stat` de FAT32 desde Ring 3 |
| listar un directorio | el importador, `os.listdir` | `TASK_OP_DIR_ABRIR`, `ES_NODO_*` | 🟡 parcial y por dos caminos distintos |
| borrar / renombrar | `os.remove`, `__pycache__`, `tempfile` | `remove()` y `rename()` devuelven `-1` **con motivo escrito** | 🟡 hueco honesto, no mentira |
| variables de entorno | `PYTHONPATH`, `os.environ` | `getenv()` devuelve `0` con motivo escrito | 🟡 idem |
| **un monton que CRECE** | `obmalloc`, todo | `KIND_MEMORIA`, **tope de 4 bloques por proceso** | ⛔ **primer bloqueante real** |
| `mmap` / `mprotect` | arenas, y el JIT de 3.13 | no existe | 🟢 **se esquiva**: PEP 445 + sin JIT |
| entropia | `_Py_HashSecret`, `os.urandom` | **nada** | 🟡 se esquiva con `PYTHONHASHSEED=0`; se arregla con **UNA fila** de `intrinsics.toml` (`RDRAND`) |
| reloj monotono | `time.monotonic` | TSC via `OP_INFO` | 🟢 ya esta |
| **fecha y hora reales** | `time.time`, `datetime`, `os.stat` | ✅ **`INFO_FECHA`** -- `dev/clock.rs` lee el CMOS al arrancar y extrapola con el TSC, con calendario de verdad (meses de distinto largo y bisiestos) | 🟢 **ya esta.** Este documento decia que faltaba: era falso, comprobado el 16-08 |
| hilos | `PyThread_*` (obligatorio desde 3.7) | no hay hilos de Ring 3 | 🟡 stub de un hilo; hay que escribirlo |
| senales | `signalmodule`, `KeyboardInterrupt` | un fallo mata la tarea | 🟡 stub; el Ctrl+C real vendria de `INPUT_OP_*` |
| **libm completa** | `math`, `float`, `dtoa` | `math.h` tiene **`fabs` y `fabsf`** | ⛔ **segundo bloqueante real** |
| `dlopen` | extensiones `.so` | no hay, y **no hace falta**: build estatico (`Modules/Setup`) | 🟢 por diseno |
| `setjmp`/`longjmp` | poco, y evitable | descartado con motivo en `BRECHA.md` | 🟢 evitable |
| **compilacion separada** | 600 ficheros `.c` | **UNA unidad de traduccion** | ⛔ **tercer bloqueante, y el estructural** |

### Los tres bloqueantes, dichos sin adorno

1. **El monton con tope de cuatro bloques.** Es la regla escrita en
   `LA_RAM.md` -- *"BMO-X rechaza overcommit y OOM killer a proposito"* -- y es
   una buena regla. Pero CPython pide memoria **constantemente y en trozos
   pequenos**, y un interprete no sabe de antemano cuanta va a querer. Salidas:
   (a) una arena grande al arrancar y `PyMem_SetAllocator` apuntando al monton de
   BMO -- **funciona hoy, sin tocar el kernel**; (b) subir el tope. La (a) es la
   correcta y ademas es la que respeta el modelo.
2. **libm.** No es opcional: `math`, la conversion de flotantes y `dtoa` la
   necesitan. **No hay que escribirla**: la libm de **musl** (MIT) y **openlibm**
   (MIT) son autocontenidas y estan escritas justo para esto. Es trabajo acotado
   y **verificable entero en el anfitrion**, sin encender el Ryzen.
3. **La compilacion separada.** Ya era la palanca numero 2 de
   `QUE_DESBLOQUEA.md` (*"el techo duro hoy; por encima de ~100k lineas el unity
   build deja de ser viable"*). CPython son ~400k lineas solo de nucleo. **No se
   esquiva.**

### ⚠ Correccion a lo que se venia diciendo de BMO C

Dos cosas que las notas viejas dan por rotas y **ya no lo estan** (comprobado en
`codegen/mod.rs` y `codegen/floats.rs` el 2026-08-16):

- **Un `double` como PARAMETRO funciona.** El comentario que lo rechazaba sigue
  en el fichero como historia; debajo esta el arreglo. Los argumentos van por la
  pila y un `double` cabe en una ranura; lo que fallaba era el sitio de llamada.
- **Un `double` GLOBAL se lee donde vive** (`lea rip` + `movsd`).

Las dos hacian falta para cualquier cosa con flotantes, o sea para `math`.

---

## 3. Las tres rutas, con su numero incomodo

### Ruta A -- portar CPython de verdad

**Lo que cuesta:** compilacion separada, un preprocesador que aguante 400k
lineas escritas contra GCC, libm, `bmomodule.c`, el stub de hilos.
**El numero incomodo**: DOOM son ~35k lineas, **y todavia no es jugable**.
El nucleo de CPython es del orden de **diez a doce veces DOOM**, y mas denso en
caracteristicas de C.
**Lo que se gana:** Python de verdad, con su semantica y su libreria.
**Veredicto:** no es una fase, es un proyecto del tamano del kernel. Se deja
escrito y no se empieza.

### Ruta B -- MicroPython  ★ la que recomiendo

**Lo que es:** ~100k lineas de C, licencia MIT, **escrito para correr en metal
sin sistema operativo debajo**. Su capa de puerto es un directorio
(`ports/minimal` existe y es diminuto).

★★ **El hallazgo que lo pone primero: sus requisitos son casi exactamente los de
DOOM, y BMO ya sirve los de DOOM.**

- Su recolector trabaja sobre **UN bloque contiguo** que le das al arrancar.
  Eso es *literalmente* `Z_Zone` de DOOM, y `KIND_MEMORIA` ya lo entrega --
  **verificado en metal el 12-08**: `bloque entregado a Ring 3 =12582912`.
- **Trae su propia libm dentro** (`lib/libm/`). El bloqueante 2 desaparece.
- No pide hilos, ni entropia, ni `mmap`, ni `dlopen`.
- Cabe en una unidad de traduccion mucho antes que CPython.

**Lo que NO se gana:** la libreria estandar de CPython ni PyPI. MicroPython es
Python 3 de verdad -- corre en el ESP32 y en la Pico -- pero es un subconjunto.

**Y lo que lo hace valioso aparte del resultado:** es **codigo ajeno escrito por
gente que no sabia que BMO existe**. Es el mismo argumento que hace de DOOM la
prueba que vale, y el que encontro el agujero de `<strings.h>`: *esa clase de
hueco no sale de auditar lo que hay, sale de intentar compilar algo escrito
fuera*.

### Ruta C -- BMO Python propio (`toolchain/lang/python/`)

Encaja con todo lo que ya esta construido... **salvo en una cosa, y hay que
decirla antes de empezar y no despues**:

- Los cuatro frontends de hoy comparten backend porque los cuatro son
  **estaticos**. Python no. Un frontend de Python es **parser + RUNTIME**, y el
  runtime es el trabajo: cabecera de objeto, conteo de referencias *y* ciclos,
  `dict` (que es donde vive Python de verdad), `list`, `str` con su
  codificacion, `int` de precision arbitraria, marcos, generadores -- y
  **excepciones**.
- ★ **Y las excepciones son la pieza que decide.** BMO C++ es defendible
  precisamente porque las descarto (`PROPOSITO.md`: *"la mas cara... piden
  tablas de desenrollado"*). **Python no las puede descartar: `try/except` es el
  lenguaje.** Pero la salida existe y es la que usa CPython: **un interprete no
  necesita tablas de desenrollado** -- propaga una bandera de error y cada
  bytecode la mira.
- Conclusion honesta: **escribir un Python propio es escribir un INTERPRETE**,
  no un compilador AOT a BEF nativo. Es el primer sitio donde la identidad
  "compilo a nativo y no hay runtime debajo" se topa con un limite real. No
  tiene nada de malo -- pero hay que elegirlo con los ojos abiertos.
- **Numero incomodo:** un interprete de un subconjunto honesto de Python en
  Rust `no_std` sobre la superficie de BMO es del orden de **25-40k lineas**, o
  sea **mas que los cuatro frontends actuales juntos**, y eso antes de una sola
  linea de libreria estandar.

---

## 4. El plan total, por fases

**Fase 0 -- LA SUPERFICIE, y no Python.** *(esta es la unica fase que recomiendo
empezar ya)*

El entregable es la tabla de la seccion 2 convertida en **sondas**, con el metodo
de `c-gen`: programas minimos que compilan o no, y que se ejecutan.

- [x] ★ **Cuanto vale una puerta, en ciclos** -- `c/coste.bex`
      (`toolchain/lang/c/examples/coste_C.c`, escrito el 16-08). Compara bucle
      vacio / llamada normal / `INVOKE` pelado / `INVOKE` con handle, y se queda
      con el **minimo** porque el temporizador expropia. ✅ **CORRIDO EN EL
      RYZEN el 16-08: la puerta vale ~2.570 ciclos, la llamada ~20** -- ver la
      seccion 4b. ⏳ Falta la fila 4 (el handle), corregida el mismo dia.
- [ ] `wc -l` real sobre el tarball de CPython y sobre MicroPython. Sustituir
      los ordenes de magnitud de este documento por numeros.
- [ ] **Entropia: una fila de `intrinsics.toml`** (`RDRAND` / `RDSEED`). Es la
      pieza mas barata de la tabla y la usan tambien firma, `net` y ESTRATOS.
- [x] ~~**La fecha real.**~~ ✅ **YA ESTABA** (comprobado el 16-08):
      `INFO_FECHA` + `ring0/dev/clock.rs`, que lee el CMOS al arrancar y suma
      los segundos por TSC con un calendario de verdad. Se dio por ausente sin
      mirar, que es el error que este documento existe para no cometer.
- [ ] **`stat` y listar directorio desde Ring 3 sobre FAT32**, por UN camino y no
      por dos.
- [ ] Medir si el monton de `<bmo/monton.h>` aguanta un patron de asignacion de
      interprete (muchos bloques pequenos, vida corta, sin orden). **Con una
      sonda, no razonando.**

★ **Ninguna de estas cinco es "trabajo de Python".** Las cinco las piden tambien
DOOM, el audio, la red y Ada. Por eso esta fase va primero pase lo que pase con
las otras.

**Fase 1 -- libm, en el anfitrion.** Traer musl libm u openlibm y hacerla pasar
por BMO C. **Se verifica entera sin encender el Ryzen**, comparando contra la
libm de Windows valor a valor -- el mismo metodo que valido `ROUNDED` (dos
implementaciones de la misma regla comparadas entre si).

**Fase 2 -- la decision: A, B o C.** Es de Eddi. Este documento existe para que
la tome con los tres numeros delante.

**Fase 3 (si es B) -- MicroPython, con la forma de DOOM.**
1. Compilarlo en el anfitrion tal cual, para tener el oraculo.
2. `ports/bmo/` -- el puerto: monton sobre `KIND_MEMORIA`, consola sobre
   `TASK_OP_CONSOLE_*`, ficheros sobre `ARCH_OP_*`.
3. Un `.bex` que corra `print(2+2)`. **Esa es la foto.**
4. El REPL sobre la consola del escritorio.

**Fase 4 -- el paquete** (ver la seccion 5). Un `.bex` con los `.py` dentro.

**Fase 5 (si es C) -- el interprete propio.** Y el orden estaria decidido por lo
aprendido en la 3, que es la razon de ponerla detras.

---

## 4b. ★★★ LA DECISION DE ARQUITECTURA (2026-08-16)

Salida de la conversacion del 16-08. Eddi propuso **que el runtime entero fuera
un contrato que entra por `INVOKE`**. La idea es correcta en su intencion y hubo
que reencuadrarla; esto es el resultado.

### Por que el runtime NO entra por `INVOKE`

Un `INVOKE` es transicion de anillo + resolucion de capability + `xsave64` de
1024 B. Una llamada a funcion son ~2-5 ciclos. Un bucle de Python hace ~5
operaciones de runtime por vuelta; un millon de vueltas son 5 millones de
operaciones. **`a + b` es la operacion mas interna del lenguaje, y una
transicion de anillo ahi es la peor colocacion posible.**

★ **Y el arbol ya contesto esta pregunta dos veces:**

1. `ARCH_OP_LEER` devuelve **siete bytes por llamada**, y por eso tuvo que
   existir `ARCH_OP_LEER_EN`: *"para un WAD de 4 MB eso son seiscientas mil
   llamadas al sistema"*. Seiscientas mil ya se declaro inaceptable **para
   cargar un fichero UNA vez**.
2. `<bmo/monton.h>`: cuando llego *"cada `malloc` es un syscall?"*, la respuesta
   fue **no** -- el kernel entrega UN bloque con `KIND_MEMORIA` y toda la logica
   vive en Ring 3. **El modelo de objetos es esa misma pregunta un piso arriba.**

Regla que resume: **una capability existe para arbitrar AUTORIDAD. `2+2` no
tiene ninguna autoridad que arbitrar** -- no toca hardware, ni memoria de otro,
ni puede robar nada.

### ✅ MEDIDO EN EL RYZEN el 2026-08-16 -- `c/coste.bex`

```text
   TSC 3700000000 Hz, lote 4096, vueltas 16
   1. bucle vacio   min   43 ciclos/op, media   45
   2. llamada       min   65 ciclos/op, media   67
   3. puerta minima min 2615 ciclos/op, media 6287
   4. puerta con handle: NO SE MIDIO  (defecto de la sonda, corregido)
```

**Una puerta cuesta ~2.570 ciclos netos = ~707 ns. Una llamada, ~20. Factor
~120x.** La estimacion previa de este documento decia ~500 ciclos: **era baja
por CINCO veces**, y en la direccion que refuerza la conclusion.

La aritmetica de Python, ya con el numero del dueno -- `for i in
range(1000000): x += i`, unos 5 millones de operaciones de runtime:

| | por operacion | el bucle entero |
|---|---|---|
| como llamadas | ~20 ciclos | **~30 ms** |
| como `INVOKE` | ~2.570 ciclos | **~3,5 s** (con la media, ~8,5 s) |

★★★ **Y hay un argumento que no es de velocidad y que cierra el asunto del
todo**: `schedule_locked` solo cambia de tarea **en la frontera de un trap**, o
sea que **cada syscall ES una oportunidad de planificacion**. Una puerta no
cuesta 2.570 ciclos: cuesta 2.570 ciclos **y una probabilidad de perder el
turno**. Se ve en los propios datos -- las filas 1 y 2 quedan a un 5% de su
minimo y la fila 3 se va al **240%**. Para el interior de `a + b` eso no es
lento, es impredecible.

### ★★ Y el hallazgo que NO es de Python: 2.570 ciclos es CARO para un syscall

El par `syscall`/`sysret` son ~100-150 ciclos de silicio, y BMO **no lleva
mitigaciones de Spectre** (kernel propio, sin KPTI). O sea que **~95% del coste
es trabajo del propio BMO**, no la transicion de anillo. Leido en
`ring0/syscall/entry.rs`, esto es lo que paga TODA puerta:

- **8 escrituras** para poner a cero la cabecera del area XSAVE (lineas 52-59).
- **`xsave64` con `RFBM = -1`** (linea 64-65): guarda **todo** lo que XCR0
  habilite, sin mirar si cambio.
- **7 lecturas + OR** para validar esa cabecera al volver (lineas 80-87).
- **`xrstor64` con `RFBM = -1`** (linea 92-93).
- **`iretq`** (linea 103) y no `sysretq`: `iretq` **serializa**, y es de las
  instrucciones caras de x86-64. Se usa porque el epilogo se comparte con las
  interrupciones.

★★★ **La observacion que lo hace accionable: una puerta que vuelve a LA MISMA
tarea no necesita `xsave64`/`xrstor64` para nada.** El estado extendido ya es el
correcto -- nadie lo toco. Ese guardado existe para el **cambio de contexto**,
no para el syscall. Guardarlo solo cuando `dispatch` decide de verdad cambiar de
tarea es lo que hace Linux en su camino rapido, y aqui se ahorraria el XSAVE, el
XRSTOR y las 15 escrituras/lecturas de cabecera **en el 100% de las puertas que
no cambian de tarea**, que son casi todas.

⚠ **No esta medido cuanto es cada trozo**: esta confirmado el MECANISMO (leido
en el codigo), no el reparto. Lo que lo separa es medir otra vez con el XSAVE
fuera del camino que no conmuta.

**Y esto no mejora Python: mejora TODO lo que cruza la puerta** -- la entrada de
DOOM, el metronomo del audio, el camino asincrono del disco, el compositor.

#### ★ Por que es DEMOSTRABLEMENTE seguro, y aun asi NO es un cambio rapido

Comprobado el 16-08 preguntandoselo a rustc:

```text
   x86_64-unknown-none
   features: -mmx,-sse,-sse2,-sse3,-ssse3,-sse4.1,-sse4.2,-avx,-avx2,+soft-float
   rustc-abi: softfloat
```

★★ **El kernel se compila SIN SSE y con soft-float: no toca un solo registro
xmm.** Asi que el estado extendido de una tarea esta **garantizado intacto**
durante un syscall que no conmuta. No es "probablemente seguro": es seguro por
construccion del target.

⚠ **Y aun asi no se toca hoy, por tres razones que hay que decir juntas:**

1. **No se puede saber al ENTRAR si `dispatch` va a conmutar.** El arreglo
   correcto no es "saltarse el xsave": es **moverlo al camino del cambio de
   contexto**, o sea reestructurar donde vive el area y el **sello** de contexto
   (`{firma}`, `gs:[0x10]`, el back-pointer). Es diseno, no una linea.
2. **El camino de la INTERRUPCION se queda como esta.** Un timer puede caer a
   mitad de un calculo de usuario y ese si conmuta. Solo el syscall que vuelve a
   la misma tarea puede ahorrarselo.
3. ⛔ **Es el codigo que produjo el `#GP en xrstor`** que costo el metodo de las
   cinco sondas. Y ahora mismo hay repartos del kernel **sin verificar en
   metal**: meter una quinta cosa antes de esa foto convierte *"arranco?"* en
   *"cual de las cinco?"*. La misma parada deliberada que ya se decidio una vez.

**`iretq` -> `sysretq`** es un segundo cambio, independiente y tambien con
trampas propias (`sysret` no restaura RFLAGS arbitrarias y da `#GP` con un RIP
no canonico). No se mezclan.

★ **Y lo que hay que tener claro: NADA de esto desbloquea Python.** Aunque la
puerta bajara a 800 ciclos seguiria siendo **40x** una llamada, y la decision de
la seccion 4b no se moveria ni un milimetro. Son **dos pistas independientes**
que salieron de la misma sonda.

### ★ El premio retroactivo: `ARCH_OP_LEER_EN` tenia razon, y ahora con numero

`ARCH_OP_LEER` da **siete bytes por llamada**. El WAD de DOOM son 4.196.020
bytes = **599.431 puertas**, y a 707 ns cada una eso son **0,42 segundos solo en
coste de puerta** (con la media, ~1 s). La decision de crear `ARCH_OP_LEER_EN`
se tomo razonando; ahora tiene su cifra detras.

### ★★ Lo que SI es contrato: el FORMATO, no la operacion

Es como funciona BMO en todas partes -- el `.bex` es un formato con **dos
lectores independientes**, `BRES` son entradas de **64 bytes fijos**,
`intrinsics.toml` es una tabla, los mods son tablas y no plugins.

Un `.bex` de Python declara en **BEF**:

| Seccion | Que lleva | Por que ahi |
|---|---|---|
| **Tipos** | clases, ranuras, disposicion | tamano fijo, offsets y no punteros: legible sin `alloc`, como `BRES` |
| **Bytecode** | instrucciones de tamano fijo | **se ejecuta donde esta**, no entra en RAM |
| **Constantes** | el pozo inmutable: cadenas internadas, enteros, tuplas | ★★ **esto es lo que se presta** |

★ **La tabla de tipo con ranuras numeradas ES la idea de Eddi, bien colocada**:
`OP_SUMAR = 0x03` indexa una **vtabla en memoria de usuario**, o sea un
`call [rax+24]` y no un `syscall`. BMO C++ ya emite exactamente eso desde el
paso 5 (`d24b96c1`).

### ★★★ "El kernel presta" -- la version precisa

**El kernel no ejecuta Python: presta las partes del programa que NUNCA
cambian.** Codigo, tipos y constantes son de solo lectura e identicos en cada
instancia; solo el monton es por proceso.

Es el patron de la casa otra vez -- *el kernel da el aparato y los protocolos
son de usuario* (red), *el kernel da el bloque y `malloc` es de Ring 3*
(monton), *el kernel da la pantalla y se aparta* (`KIND_FRAMEBUFFER`).

Las cuatro piezas **ya existen**: `MEM_OP_OFRECER`, `PRESTADO_OP_*`, la seccion
de recursos y "se abre, no se carga".

⚠ **El precio, y se decide el dia uno o no se decide**: para prestar el pozo de
constantes, esos objetos **no pueden escribir su contador de referencias** -- si
lo escriben, ensucian la pagina compartida y el prestamo no vale nada. Eso son
objetos **inmortales**, y tiene que estar **en la cabecera de objeto desde la
primera linea**. CPython llego a lo mismo (PEP 683) y le costo una version
entera meterlo a posteriori. Es el argumento mas fuerte para escribir el
contrato antes que el codigo.

### Los DOS MODOS

Decidido por Eddi el 16-08: modo interprete y modo AOT.

```text
   .py -> lexer -> parser -> AST -> compilador
                                      +-> bytecode  -> interprete  (REPL, todo el lenguaje)
                                      +-> AST de C  -> BMO C -> BEF  (AOT, closed world)
```

- Comparten la mitad delantera. **Y los dos usan EL MISMO runtime.**
- ★ **El AOT no elimina el runtime**: un `x + y` compilado sigue siendo
  `call runtime_sumar(x, y)`, porque sigue sin saberse los tipos. El AOT quita
  **el bucle de despacho**, no el modelo de objetos. Ganancia realista **~2-4x**,
  no 50x. Para acercarse a C haria falta informacion de tipos, o sea
  anotaciones y un subconjunto restringido -- eso es Cython/mypyc, y es otro
  lenguaje.
- El AOT pierde `eval`/`exec`/REPL: es el modelo **closed-world** de
  NativeAOT/GraalVM, ya nombrado en [`bmo-ada-plan`] como *"el modelo a copiar"*.
- ⚠ **Y de ahi el ORDEN: el AOT no se puede hacer primero.** Necesita el
  runtime, el runtime es el 80% del trabajo, y quien lo estrena es el
  interprete. Hacer el AOT antes es poner el tejado.

### El alcance: contrato completo, implementacion semilla

Hay una tension real entre *"Python tendra su BASE, para darle semillas"* y
*"construir TODO lo que Python tenga"*. Se resuelve asi:

> **El CONTRATO puede estar completo. La IMPLEMENTACION es la semilla.**

Y hay precedente exacto: `SectionKind::Resources = 0x0B` estuvo **declarada y
vacia desde que se diseno BEF**, con su comentario, y nadie la escribia --
`Manifest = 0x09` y `Signature = 0x0F` siguen asi. Eso no fue desperdicio: fue
**que el formato estuvo listo antes que el codigo**, y por eso el paquete salio
sin tocar el cargador.

### Donde vive el runtime, entonces

**No en el kernel** (seria un *cerebro*, y la regla 2 lo prohibe) y **no en
`bmo_lower`** -- porque `bmo_lower` **emite bytes**, y eso vale para `memcpy`
pero emitir una tabla hash byte a byte desde Rust es una locura.

★ **Vive como C, en `forge/`.** Y la forma ya la invento BMO C++:
`lang/cpp/src/descenso.rs` no tiene backend propio, **desciende al AST de BMO
C**. El runtime de Python es un `.c`, el frontend emite llamadas contra el, y la
unidad de traduccion unica **deja de ser un problema** porque runtime y programa
son el mismo `.bex`. De regalo: se prueba en el anfitrion con gcc antes de que
BMO lo compile, y convierte a Python en **el primer cliente serio de BMO C como
DESTINO en vez de como herramienta**.

### La prueba de aceptacion

`PROPOSITO.md` exige la prueba de fuego de cada lenguaje. La de Python:

> **Puedo escribirlo sin declarar nada y probarlo AHORA MISMO?**

Python existe para *el programa que escribes para averiguar que deberia ser el
programa*. De ahi sale la base entera, y de ahi sale la foto que la demuestra:

★★ **Una foto del Ryzen con un `>>>` donde se escribe `2+2` y contesta `4`.**
Es el equivalente de `19.99 x 3 = 59.97` en COBOL. Y **el REPL fuerza un
interprete**: no se puede compilar AOT una linea que el usuario todavia no ha
escrito.

| La semilla | El crecimiento |
|---|---|
| **el REPL** -- no es una caracteristica, es el motivo | generadores / `yield` (piden un marco que sobrevive al `return`) |
| tipos dinamicos, todo es objeto | decoradores, comprensiones |
| `int` de precision arbitraria, `str`, `list`, `dict`, `tuple`, `bool`, `None` | metaclases, descriptores, `async` |
| `def`, `class`, `if/for/while`, `try/except`, bloques por indentacion | herencia multiple y el MRO |
| el protocolo de iteracion | la libreria estandar entera |

**Numero incomodo**: MicroPython entero son ~100k lineas. Una base honesta
--REPL + los siete tipos + `def`/`class`/`if`/`for`/`try`-- es del orden de
**12-18k lineas de C**. Mas que cualquier frontend de hoy, menos que el kernel.

---

## 5. ★★ BEF: la mitad que ya esta hecha y nadie se dio cuenta

Eddi dijo *"es BEF que gracias a ese encabezado enmascara"*. Tiene razon, y esta
es la version concreta:

**Una app de Python en BMO-X es UN fichero `.bex`**: las secciones de codigo del
interprete, **y todos los `.py` / `.pyc` dentro de la seccion
`Resources = 0x0B`**, leidos por desplazamiento con `TASK_OP_MI_PAQUETE`, **sin
cargarse a RAM** -- la decision *"se abre, no se carga"* del escalon 2 de
`LA_RAM.md`, que ya esta implementada y medida (`doom.bex` + WAD: de 6.313.632 B
se traen 813.552, **-87,1%**).

Comparado con lo que hace el mundo:

| | Que es |
|---|---|
| CPython `zipapp` / `.pyz` | un ZIP con un cabecero, leido por `zipimport` |
| CPython `Lib/` suelto | cientos de ficheros que hay que instalar y no perder |
| **BMO** | **el mismo binario**, con su indice `BRES` de entradas de 64 bytes |

★ **El `sys.path` de un Python de BMO es el indice de recursos del propio
`.bex`, y el gancho de importacion son veinte lineas.** El formato existe
(`bmo_abi::bef::recursos`), la herramienta existe (`bmo-pack -r`), el kernel ya
devuelve el handle (`TASK_OP_MI_PAQUETE = 0x25`), y esta **verificado en el
Ryzen** desde el 2026-08-09 (`c/caja.bex`, las cuatro pruebas).

Y de regalo: la seccion `Signature` viaja dentro, asi que **un paquete de Python
lleva su firma a cualquier sitio** -- cosa que un `.pyz` no puede.

**O sea: la parte de "como se entrega una app de Python" esta resuelta antes de
existir el interprete.** Lo que falta es el interprete, no el sobre.

---

## 6. Lo que NO entra, con motivo

Mismo trato que `PROPOSITO.md`: `DESCARTAR` no significa *nunca*, significa *no
en este alcance, y este es el motivo*.

| Fuera | Motivo |
|---|---|
| El **JIT** de CPython 3.13 (copy-and-patch) | pide `mprotect` y memoria ejecutable en tiempo de ejecucion. Contradice el modelo de carga de BEF, donde lo que ejecuta paso el gate. Y es experimental hasta en CPython |
| **free-threading** (PEP 703) | no hay hilos de Ring 3 todavia, y con SMP sin cablear seria estrenar concurrencia por el sitio mas caro |
| **`ctypes` / extensiones `.so`** | no hay enlazado dinamico y es una decision, no una carencia. Build estatico |
| **`socket` / `ssl`** | ⛔ bloqueado por la red, no por Python. `net rx` recibe tramas; entre eso y `import ssl` estan ARP, IP, TCP, DNS y TLS |
| **`subprocess`** | `system()` ya devuelve `-1` con motivo: aqui lanzar es `TASK_OP_EJECUTAR` con una ruta, no una cadena que otro interpreta |
| **`locale` / `wchar`** | misma razon que en `BRECHA.md`: una libc de verdad empieza aqui y no acaba nunca. Se fuerza modo UTF-8 (PEP 540) |
| **La suite de tests de CPython** | es mayor que el resto del arbol. El oraculo es CPython corriendo en el anfitrion, no su suite dentro de BMO |

---

## 7. Donde cae esto en la hoja de ruta

*(Eddi pidio expresamente que se le diga esto: "cada vez que tengo ideas me
restringe y eso es normal que me acuerdes para mantener firme".)*

- **El objetivo declarado sigue siendo BANCA + Ada.** Python no es ninguna de
  las dos.
- **Lo que hay en vuelo AHORA MISMO esperando una foto del Ryzen** es corto y
  barato: DOOM con bucle y el metro de `[perf]`, el reciclado de marcos
  (`PTE_NUESTRA`), el lanzamiento desde el escritorio, `net rx`, el audifono.
  Cada uno cuesta horas y **cada uno cierra una incertidumbre**.
- **Python, en CUALQUIERA de las tres rutas, son meses.** No es el siguiente
  paso.

**Pero la premisa de Eddi es correcta y merece cobrarse**: si el freno es la
superficie, **la fase 0 de este documento es trabajo real, barato y util haya o
no Python** -- entropia, fecha, `stat`, listar, y medir el monton. Cinco cosas
que hoy tambien le faltan a DOOM, al audio, a la red y a Ada.

> **Etiqueta honesta**: la **fase 0 es el siguiente paso**. La **fase 1 (libm)
> es un desvio barato** y se justifica sola. **Las fases 2 en adelante son un
> proyecto aparte**, del tamano del kernel, y no se empiezan sin decidir
> primero que se aparca a cambio.

---

## 8. ★★ POR DONDE SE EMPIEZA -- el primer paso, uno solo

Escrito el 16-08 porque *"no se por donde comenzamos"* es una pregunta legitima
cuando hay tres pistas abiertas a la vez. Van en este orden y por este motivo.

### ✅ Paso 1 -- LA CABECERA DE OBJETO. HECHO el 2026-08-16.

`bmo_abi::dynobj` -- `header.rs` (16 bytes) + `slots.rs` (las operaciones
numeradas). **14 tests en verde, cero kernel, cero metal.**

```text
   +0   refs        u64   bit 63 = INMORTAL
   +8   type_index  u32   INDICE, nunca un puntero
   +12  flags       u32   PRESTADO, RASTREADO
```

★ **No se llama `python` a proposito**: es una REPRESENTACION, no semantica de
lenguaje -- la misma llamada que puso el BCD en `bmo_lower::packed`. Y **no va
dentro de `runtime/`**, que ya existe con `TypeRegistry`/`VTableStore` pero es
un registro de interfaces entre lenguajes (otro trabajo, y sin usuarios fuera de
sus tests).

★★ **Y escribirlo destapo una restriccion que no estaba en ningun sitio**:
`loan::take` mapea en la direccion que decide la RANURA, asi que **la misma
pagina prestada cae en una direccion distinta en cada proceso**. Por tanto un
objeto compartido **no puede llevar un puntero a otro objeto compartido**: lleva
un indice. La regla de *"offsets y no punteros"* que se escribio para el
bytecode resulta ser obligatoria tambien en la cabecera. Y es justo lo que
descalifica a `runtime/vtable.rs`, cuyas entradas son `extern "C" fn()`.

Los tests que se ganan el sitio: **un contador inmortal no se mueve nunca**
(sin eso el pozo de constantes no se puede prestar y se cae la seccion 4b),
**soltar por debajo de cero no falsifica un inmortal** (`0 - 1` pondria el bit
63: un doble free se volveria una fuga que no se denuncia), y **dos ranuras no
comparten numero** -- que no es higiene, es el bug de `INFO_CPU_HZ_REAL` escrito
encima de `INFO_FUGAS`.

**Lo siguiente de este paso**: la entrada de la tabla de tipos (el formato de la
seccion BEF `Tipos`), que necesitaba que los numeros de ranura estuvieran
decididos.

#### Por que fue este y no otro (historico)

**Por que este y no otro:**

- Es lo unico de toda la lista que **no se puede meter despues**. CPython lo
  intento (PEP 683) y le costo una version entera. Sin el, el pozo de constantes
  no se puede prestar y la mejor idea de este documento se cae.
- Es **Rust puro en `bmo-abi`, probado en el anfitrion**: cero kernel, cero
  metal, cero riesgo de que algo deje de arrancar.
- Es literalmente lo que se acordo: *el CONTRATO antes que el codigo*.
- Cabe en ~150 lineas y sus tests.

**Como se sabe que esta hecho:** tests de anfitrion que fijan tamano, alineado,
que un objeto inmortal **nunca cambia su contador**, y que la disposicion es la
que veria C. Ni una linea del kernel tocada.

### Paso 2 -- La entropia. Una fila de TOML, quince minutos.

`RDRAND` en `intrinsics.toml`. **No es privilegiado**, asi que no toca el kernel
ni la superficie. Cierra un hueco que piden la firma, ESTRATOS, la red y Python.

### ~~Paso 3 -- La fecha real.~~ ✅ YA ESTABA

`INFO_FECHA` existe y `ring0/dev/clock.rs` lee el CMOS. Se dio por ausente sin
comprobarlo. **Queda `stat` de FAT32 desde Ring 3**, que es lo que de verdad
falta de esa fila.

### ⛔ Lo que NO va ahora, y es una decision, no un olvido

- **El XSAVE.** Ver la nota de la seccion 4b: seguro por construccion, **pero es
  reestructurar el contexto**, y hay repartos de kernel sin verificar en metal.
  Va DESPUES de esa foto.
- **El interprete.** Va detras del paso 1, porque el paso 1 es su cimiento.
- **El AOT.** Va detras del interprete, porque estrena el mismo runtime.

---

Ver `QUE_DESBLOQUEA.md` (por que la superficie manda sobre el lenguaje),
`LA_RAM.md` (el monton y el "se abre, no se carga"),
`toolchain/lang/PROPOSITO.md` (para que existe cada lenguaje) y
`toolchain/lang/c/BRECHA.md` (lo que BMO C compila, medido con sondas).
