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
| **fecha y hora reales** | `time.time`, `datetime`, `os.stat` | **no se lee el RTC** | 🟡 `time.time()` mentiria, y mentir es peor que fallar |
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
      con el **minimo** porque el temporizador expropia. **Escrito y compilado;
      falta correrlo en el Ryzen.** Es el numero que decide la seccion 4b, y
      tambien lo piden el audio, el disco asincrono y la red.
- [ ] `wc -l` real sobre el tarball de CPython y sobre MicroPython. Sustituir
      los ordenes de magnitud de este documento por numeros.
- [ ] **Entropia: una fila de `intrinsics.toml`** (`RDRAND` / `RDSEED`). Es la
      pieza mas barata de la tabla y la usan tambien firma, `net` y ESTRATOS.
- [ ] **La fecha real.** Hoy no se lee el RTC. `time.time()` mintiendo es peor
      que `time.time()` fallando -- regla 1.
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

⚠ La cifra de arriba es una estimacion. **La cierra `c/coste.bex`**, y hasta
entonces se dice que es una estimacion.

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

Ver `QUE_DESBLOQUEA.md` (por que la superficie manda sobre el lenguaje),
`LA_RAM.md` (el monton y el "se abre, no se carga"),
`toolchain/lang/PROPOSITO.md` (para que existe cada lenguaje) y
`toolchain/lang/c/BRECHA.md` (lo que BMO C compila, medido con sondas).
