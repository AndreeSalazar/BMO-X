# AVANCES -- estado de BMO-X (recuperacion de contexto)

> Documento vivo para retomar el proyecto desde cero (chat nuevo). Resumen de
> QUE funciona, QUE falta, DONDE esta cada cosa y COMO se trabaja. Para el
> detalle cronologico ver los commits y `BITACORA.md`.

**BMO-X** = sistema operativo bare-metal en Rust: microkernel de **capabilities**
con **superficie congelada de 3 syscalls** (`INVOKE`/`CHANNEL_KICK`/`WAIT`) +
subsyscalls; arranca en **hardware real** (MSI A320M PRO MAX + Ryzen 5 5600X),
sin QEMU. Toolchain propio (C / COBOL / **Ada** / C++ -> BEF -> BEX nativo), y los
tres primeros **ya han ejecutado en el Ryzen**.

> **Al 2026-08-08**: **827 tests en verde y CERO rojos** (el conteo excluye
> `bmo-kernel` y `bmo-rt`, que por diseno no compilan en el anfitrion).
>
> Y desde el 08-08 el build valida **la codificacion de las fuentes** en el
> mismo paso que valida el contrato de syscalls: las fuentes son ASCII y las
> cadenas que pinta el kernel tambien. No es estetica -- un acento en un
> literal de C llego a multiplicar un `.bex` por 65.536, y la consola es
> Latin-1 mientras las cadenas de Rust son UTF-8. Ver `BITACORA.md`, Ep. 30.
> BMO-X ocupa ~5.4 MiB de 14.8 GiB, y el objetivo declarado sigue siendo
> **BANCA + Ada**.
>
> ⚠ **Dos descartes de julio ya no describen el proyecto**: las **ventanas
> estan HECHAS** --marco unico con minimizar/maximizar/cerrar, fichas en la barra
> y el grafo de ESTRATOS navegable, verificado en el Ryzen el 06-08-- y **Vulkan
> esta APARCADO con plan escrito** (`PLAN_VULKAN.md`), que no es lo mismo que
> descartado. Siguen fuera Wine y la libc completa.

## ★★★ 2026-08-07 -- SMP ARRANCA (12/12 EN METAL), Y LA CADENA DE FICHEROS EN C

### 1 - ✅ DOCE HILOS EN PIE, en el Ryzen

`nucleos en pie: 12 de 12`. El trampolin de SMP --escrito de cero: modo real de
16 bits, saltos lejanos emitidos byte a byte, y el `CR3` del kernel en vez de
tablas propias-- levanto los once APs **a la primera**.

El de `s1_cpu` que llevaba ahi sin llamarse **no podia funcionar**: estaba
ensamblado como codigo de 64 bits para un nucleo que arranca en 16 (un `0x48` es
`dec ax` en modo real), sus tablas de paginas se solapaban, el contador de vivos
vivia dentro de la PML4 que el paso anterior ponia a cero, y estaba colocado
*antes* de `ExitBootServices`, donde los nucleos todavia son del firmware.

**El mando, que es lo que se pidio y no un boton:**

```
smp          censa y NO TOCA NADA        smp parar    los obreros a hlt
smp all      despierta a todos           smp prueba   mide el reparto
smp 3        despierta exactamente tres
```

El caso por defecto es el inofensivo a proposito: INIT+SIPI no se deshace sin
reiniciar. Y a quien llamar **lo dice la MADT**, no una suposicion -- `plat/madt.rs`
enumera los APIC IDs de verdad y contrasta contra CPUID.

★ Y el reparto de trabajo (`plat/smp/obra.rs`): el BSP publica una funcion y n
partes, cada obrero hace la suya, barrera al final. **No es un planificador**, y
ser tan poca cosa es lo que lo hace seguro con los 209 `static mut` del kernel.
⚠ Un obrero en espera **gira**, no duerme: con los doce en pie hay once nucleos
al 100 %, y por eso existe `smp parar`.

### 2 - La cadena de ficheros en C: `fopen` - `fread` - `fseek` - `fclose`

`ARCH_OP_LEER` daba **siete bytes por llamada** --un WAD de DOOM serian 600 000
syscalls-- y no habia `seek`. El motivo estaba escrito: *"pasar un puntero de
Ring 3 obligaria al kernel a validar el rango contra el espacio del llamante, y
esa infraestructura no existe"*.

★ **La salida no fue construir esa infraestructura: fue no necesitarla.**
`ARCH_OP_LEER_EN` escribe dentro de **un bloque que concedio el kernel**, asi que
comprobar es una resta contra lo que entrego. Contrato en vez de comprobacion --
la misma razon por la que reclamar la pantalla ya era seguro.

Y `fopen` **no es un builtin del compilador, es una cabecera** (`<bmo/archivo.h>`),
porque abrir un fichero son varias llamadas y eso en opcodes a mano serian
doscientas lineas ilegibles. Lo que si hizo falta tocar en el compilador:
`malloc` **publica ahora el handle de su bloque**, que antes tiraba.

### 3 - Dos programas nuevos en C

- **`c/ray.bex`** -- un raycaster 2.5D en punto fijo, sin un solo `float` y sin
  tablas de senos, sobre la pantalla real. El ensayo general de DOOM.
- **`c/leer.bex`** -- abre `datos/salida.txt`, lee un bloque, hace `fseek` y
  **relee para comparar**. Que las dos lecturas coincidan es lo que separa
  *"leyo el fichero"* de *"escribio algo en mi buffer"*.

⚠ Anomalia abierta: `leer.bex` compila a **1,1 MB** cuando deberia rondar los
20 KB, y **cortando el fichero por la mitad sale mas grande**. Ninguna pieza por
separado lo reproduce. Es relleno o alineacion de seccion, no codigo emitido; la
biseccion esta hecha y anotada en el commit.

## ★★★ 2026-08-06 -- EL TECLADO QUE SE "DESCONECTABA", LAS AGUJAS, Y DOS ALMACENES DE PRUEBAS

Sesion de endurecimiento, no de features. Cuatro commits y ninguno anade una
capacidad nueva: los cuatro hacen que lo que ya existe **falle donde se vea**.

### 1 - Resucitar un endpoint USB parado (`f30e40b0`)

`bmo-xhci` sabia **ver** un endpoint Halted --`ep_state` lo documenta desde hace
tiempo-- y no tenia con que levantarlo: **Reset Endpoint (14) y Set TR Dequeue
(16) no estaban escritos**. Un error de transaccion del bus dejaba el endpoint
parado y a partir de ahi `rearmar()` encolaba y tocaba el timbre para nada,
porque **el xHC ignora el doorbell de un endpoint Halted**. Desde la silla se
veia identico a un teclado desenchufado -- que es exactamente como lo conto el
dueno.

El paso que se olvida es el 2: resetear sin recolocar el puntero deja el
endpoint listo para leer TRBs viejos con el ciclo cambiado. Y va en **un solo
sitio**, el reparto de `uhid::Hid::poll`, porque es el unico que ya sabe de quien
es el evento.

⚠ De propina: el raton contaba sus errores desde el primer dia y **el teclado no
tenia rama de error en absoluto**. El aparato del que se dijo "se desconecta sin
sentido" era justo el unico que no dejaba una linea al fallar.

### 2 - Las AGUJAS: 57 sitios, 7 mentian (`1017285f`)

Barrido de todo `let _ =`, `.ok()`, `unwrap_or(0)` y `unwrap_or(false)` en
kernel, userspace y platform. **50 no eran deuda** (callar un parametro, drenar
un puerto, una funcion que ya grita por dentro). Los 7 restantes compartian un
patron que no revienta: **convertir un fallo en un valor con pinta de buen
dato**.

- **El cargador aplicaba relocaciones y tiraba el error.** Una relocacion sin
  aplicar deja la direccion sin corregir: decia "cargado" y el programa saltaba a
  la basura. Un binario mal relocado no es un binario degradado, **es otro**.
- **Y devolvia `Ok` con entrada en la direccion cero** si el BEF no traia
  seccion de codigo.
- **`free_chain` de FAT32 hacia lo contrario de lo que existe para hacer**: si
  la FAT no se podia leer, el 0 se tomaba por fin de cadena y se salia dejando
  perdidos justo los clusters que venia a devolver.
- Una escritura a disco en un `let _ =` -> ahora `FatVolume::fallos_mudos()`,
  **que tiene que ser cero**.
- `seed_init` tiraba el resultado de `grant`: init podia arrancar con menos
  canales de los que creia.
- TLS: *"no uso TLS"* y *"no pude preparar TLS"* eran el mismo 0.
- Sin paginas DMA en xHCI se contestaba igual que "el aparato no mando nada".

La regla queda escrita: **un fallo o se maneja o se GRITA con su numero, nunca
se descarta callando.**

### 3 - Dos almacenes de pruebas, repartidos (`d0d201b5`, `47425909`)

`cobol/src/lib.rs` media 3687 lineas: **193 de compilador y 3494 de pruebas**.
Era un almacen de tests con una API pegada arriba. Y `c/src/tests/mod.rs` tenia
el trabajo a medias -- nueve ficheros por tema al lado y **112 tests sueltos** en
1784 lineas.

Ahora **el fichero es la categoria** en los dos: `rounded::rounded_respeta_el_signo`
dice que se rompio antes de abrir nada, y `cargo test printf::` corre esa parte
sola. 167 + 112 tests movidos **sin reescribir ninguno** -- bloques enteros, con
script, y la cuenta identica antes y despues (217->217, 238->238).

### 4 - Y una medicion que estaba mal

El mapa de monolitos anunciaba `try_parse_function` con 995 lineas. **Mide 138.**
El medidor contaba llaves dentro de literales de texto. Con el contador
arreglado, **el parser de C no es un monolito** (1754 lineas, 63 funciones, la
mayor de 227) y **queda UN monolito real en todo el repo**: el `_start` del
compositor, 1960 lineas en una funcion, en un fichero que tiene cuatro.

## ★★★ 2026-08-03 -- COMP-3, y el plan largo de banca ESCRITO

**`COMP-3` funciona de verdad** (`9c812537`). No es una palabra aceptada: el dato
vive en **nibbles**, dos digitos por byte y el signo en el ultimo, y el campo
ocupa **exactamente** lo que dice su PICTURE.

★ **La conversion BCD vive en `bmo-lower::packed`, NO en COBOL.** Empaquetar es
una REPRESENTACION, no la semantica de un lenguaje: los mismos nibbles en el
mismo orden los piden el `Decimal` del Annex F de Ada y el `FIXED DECIMAL` de
PL/I. Es el mismo argumento que la cabecera de `fmt.rs` usa para si misma --
**contratos y librerias, nunca cerebros**. De COBOL solo queda quien es COMP-3,
cuantos digitos y si lleva `S`; en `codegen.rs` lo miran **solo `load_var` y
`store_var`**, asi que la aritmetica sigue viendo el entero escalado y el decimal
exacto no se entera de la representacion.

★ **La prueba que no se puede fingir**: el mismo `12345` da `345` en un
`PIC 9(3) COMP-3` y `12345` en un `PIC 9(3)` (que hoy sigue siendo un i64).
Comprobado **mutando la caracteristica a no-operacion**: caen 3 tests. Los otros
seis pasan igual porque miden equivalencia y no representacion -- valen de
regresion, pero no prueban que sea real. *Ese metodo de verificacion es el que
hay que repetir cada vez que algo cambie como se GUARDA un dato.*

Se rechaza con motivo: `COMP`/`BINARY`/`COMP-5` (guardarian lo mismo que un
DISPLAY), `COMP-1`/`COMP-2` (flotante, y no representa 19.99), COMP-3 sin PIC,
sobre PIC X y sobre PIC editada. Ejemplo nuevo: `examples/7-empaquetado/`.

**★★ Y el plan largo, escrito: [`toolchain/lang/cobol/PLAN_BANCA.md`](toolchain/lang/cobol/PLAN_BANCA.md).**
Nueve fases con casillas, de "el suelo del compilador" a "un banco pequeno de
punta a punta en el Ryzen". Cada tarea dice **que la bloquea** y **como se sabe
que esta hecha**. `BANCA_REAL.md` dice que falta y por que; el plan dice en que
orden y quien depende de quien.

### La FASE 0 del plan, hecha entera menos el parser (misma sesion)

- **0.1 `VALUE` inicializa.** Se parseaba desde siempre y **no se emitia nunca**
  salvo en los 88: un campo declarado con `VALUE` arrancaba con basura y nadie
  avisaba. Pasa por `store_var`, asi que un `COMP-3` se inicializa empaquetado.
- **0.3 `OR`.** La condicion dejo de ser una `Vec` conjugada con AND y es un
  **arbol** con precedencia (`AND` liga mas fuerte) y **cortocircuito** -- que no
  es una optimizacion: un operando puede ser un elemento de tabla y ahi la
  evaluacion lleva guarda de rango. Cayeron con el **los `88` con `THRU` y con
  varios valores**, que estaban rechazados exactamente por eso.
- **★ 0.4 PARRAFOS.** La estructura de todo COBOL real, y no existia. Las cuatro
  formas del `PERFORM` fuera de linea. El retorno se decide **en ejecucion**
  (una ranura con "en que parrafo hay que volver") porque el mismo parrafo puede
  ser el final de un rango en una linea y estar en medio de otro en la de abajo.
  ★ Y de paso salio que **`STOP RUN` no emitia nada**: colaba por ser siempre la
  ultima linea, y un `STOP RUN` dentro de un `IF` se ignoraba en silencio.
- Al emulador le faltaban `push`/`pop` sobre memoria (`FF /6`, `8F /0`). Nadie
  los habia emitido nunca.

### ★ `EVALUATE`, y la revision 2 del plan (misma sesion, por la tarde)

**Se verifico el plan entero contra el codigo, y tres bloqueos eran falsos.**

1. **La fase 2 no dependia del parser de tokens.** `parser.rs` ya consume varias
   lineas (`parse_if`, `parse_perform`) y `EVALUATE ... WHEN ... END-EVALUATE` tiene
   la misma forma. `0.2` baja de bloqueo a **deuda**.
2. **El registro binario no necesita seek.** `ARCH_OP_LEER` ya saca 7 bytes
   crudos sin cortar por el salto, y esta en el kernel *y* en el emulador. El
   seek hace falta para el acceso **directo** (fase 4), no para leer un fichero
   entero.
3. **`SORT` no necesita `EXTEND`**: cada pasada puede escribir a un fichero
   nuevo con la operacion de crear que ya existe.

Y aparecio una tarea que no estaba: **`0.7`, el texto**. `FILE STATUS` es un
`PIC XX` y hoy un `PIC X` no guarda caracteres; de ahi cuelgan tambien `STRING`,
`INSPECT` y EBCDIC.

**★ Y con la fase 2 desbloqueada entro `EVALUATE`**, el verbo que mas falta
hacia. Las dos formas: con sujeto (`WHEN 1`, `WHEN 2 THRU 5`, `WHEN 6, 7`,
`WHEN OTHER`) y **`EVALUATE TRUE`**, que es la *tabla de decision* con la que un
banco escribe un escalado de comisiones.
★ El `THRU` y la coma **no costaron una linea de gramatica nueva**: la pregunta
"esta este campo en este conjunto?" se saco a `Condicion::de_valores` y ahora la
comparten el nivel 88 y el `WHEN`. Y como las dos sintaxis llegan al codegen como
el **mismo arbol**, el emisor son cinco lineas y heredan cortocircuito y
precedencia gratis. *Cuando el codegen de una caracteristica sale asi de corto,
es senal de que el parser hizo bien su trabajo.*

### ★★ `ROUNDED` con los seis modos, y la decision `1.0` TOMADA

**El redondeo es una decision LEGAL**, no una clausula de sintaxis. Medio
centimo repetido cuatro millones de veces es dinero, y hay jurisdicciones que
obligan al **redondeo del banquero** (`NEAREST-EVEN`) precisamente porque el
clasico tiene **sesgo**: en una muestra grande los empates siempre suben. Por eso
van **los seis modos del estandar** con su nombre, en las cinco aritmeticas, y
no "el redondeo" a secas. Hay un test que ensena el sesgo con cuatro empates
seguidos: el clasico inventa dos centimos y el del banquero cuadra con la suma
exacta.

★ **Se redondea el RESULTADO, no los operandos.** La operacion se hace en la
escala mas alta que aparezca y se baja **una sola vez**. Con los modos
asimetricos no es lo mismo: el techo de `-9.995` es `-9.99`, pero redondeando el
`9.995` primero sale `-10.00`. La primera version lo hacia mal y lo cazo el test
del signo.

★ **Y hay DOS implementaciones de la misma regla** --la emitida y una en Rust
para los literales, que se resuelven al compilar-- con un test que las compara
valor a valor en todo el rango alrededor de cada frontera. Dos que tienen que
coincidir prueban mas que una comparada contra una tabla escrita a mano, porque
la tabla la escribe el mismo que se pudo equivocar en las dos.

★ **Y destapo un bug de precision que no era de `ROUNDED`**: `COMPUTE` evaluaba
todo en la escala del destino, asi que `COMPUTE R = BASE * 0.075` con `R PIC V99`
**multiplicaba por `0.07`**. El resultado salia mal en el tercer decimal y ningun
redondeo podia arreglarlo, porque para cuando llegaba el digito ya no estaba.
Ahora se calcula en la escala mas alta que aparezca. `ON SIZE ERROR` se separo a
la tarea `2.6b`: necesita a donde saltar, y eso es un cuerpo de sentencias.

**★ Y la decision `1.0` esta TOMADA: camino B** -- el `FD` tiene un **area de
registro** (un buffer del largo del registro) y cada campo conserva su ranura de
trabajo *y* apunta a su posicion dentro del buffer; `READ` desempaqueta y `WRITE`
empaqueta. Los motivos escritos en el plan: es lo que dice el estandar (el area
solo vale entre un `READ` y el siguiente), `bmo_lower::packed` ya tiene media
pieza, no toca nada de lo que corre en el Ryzen, y el truncamiento de los
`DISPLAY` sigue siendo una decision aparte en vez de un efecto secundario. Lo que
se paga, dicho: `REDEFINES` no aliasara de verdad.

**Con `1.0` decidida, `0.5` (records con posiciones) se queda sin candados y es
lo siguiente** -- el primer eslabon de *leer lo que ya existe*.

### ★ LA ESTRATEGIA, decidida el 2026-08-03: primero lo que no depende del sistema

Esta escrita arriba del todo en `PLAN_BANCA.md`, con las dos listas separadas.

**Va primero TODO lo que es solo compilador** --`0.5`, `1.1`, `1.2`, el texto,
`FILE STATUS`, `STRING`, `INSPECT`, `SEARCH`, `COPY`, `SORT`, las intrinsecas--
por tres razones:

1. **Ahi esta el salto mas grande que queda y no tiene candado.** Leer
   **registros binarios de verdad** es lo que separa *"COBOL nuevo"* de *"COBOL
   que abre los datos que ya tienes"*, y se comprobo que **no necesita seek**.
2. El trabajo de kernel **no se pone mas dificil por esperar**.
3. Cada sesion de compilador **entrega algo que corre**; una de kernel no toca
   un `.cob` hasta que estan la superficie, el kernel y el emulador.

**⚠ Y el techo, dicho:** con solo eso se llega al **batch** --leer, calcular,
escribir-- que es el 80 % del COBOL que existe, pero **no** a buscar una cuenta
sin recorrer el fichero, ni a `REWRITE`, ni a varios usuarios. Eso son **tres
operaciones de `KIND_ARCHIVO`** (`EXTEND`, `I-O`, posicionar) y `3.4`. No son una
montana, pero no se pueden saltar -- y por eso estan en la lista, al final, no
descartadas.

**Tres cosas comprobadas que los documentos decian al reves:**
- ✅ **`bmo-verify` SI esta cableado** -- los cuatro frontends lo llaman **antes**
  de escribir el `.bex`. Lo que sigue sin usarlo es el **kernel**.
- ⛔ **`OPEN I-O` y el indice por clave estan bloqueados por el SISTEMA, no por
  el compilador**: `KIND_ARCHIVO` fija el modo al abrir y ESTRATOS todavia no
  crea objetos. O sea que el camino a VSAM **no empieza por el B-tree**.
- ⚠ **`VALUE` en un dato que no sea nivel 88 se ignora en silencio.** Se parsea
  y nunca se emite; los ejemplos no lo notan porque todos inicializan con
  `MOVE`. Es la tarea 0.1 del plan.

## ★★★★ 2026-08-02, QUINTA tanda -- LA LISTA DE PENDIENTES SE VACIO

No queda **nada** escrito-sin-estrenar de las seis cosas que abrieron el dia.
Todo con foto:

- **`ls` ensena**, con `-- historial --` al subir con RePag. El arreglo del
  escritor/lector (Ep. 26) confirmado.
- **★ EL FOCO ENTERO**: el conmutador de **Alt+Tab** sale con su ventanita
  (`> Ejecutar` / `Datos (ESTRATOS)`) y el modo escrito debajo
  (`modo: normal (Alt+M)`). 17 tests que llevaban desde el 2026-08-02 sin que
  nadie pulsara esas teclas -- pulsadas y correctas.
- **★★ `KIND_MEMORIA`, VERIFICADA POR LOS DOS LADOS.** `info` dice
  **`a Ring 3   8.4 MiB   pedida con KIND_MEMORIA`**. Eso no lo dice el
  programa: lo dice el KERNEL, con el contador que no leia nadie
  (`INFO_MEM_ENTREGADA`). 8.4 MiB = el doble bufer del compositor.
- **★ Ada corriendo desde el escritorio**: `run ada/cierre.bex` ->
  `CIERRE EN ADA - BANCO BMO`, `59.97`, `39.98`. Tercer lenguaje, lanzado desde
  Ring 3 y con su salida en la rejilla.
- **`info` entero**: 6 fisicos / 12 hilos, TSC 3.70 GHz medido, 14.8 GiB,
  kernel 2.1 MiB, 2 ranuras de 64, disco listo, datos montado para escritura.
- **Las tres ventanas conviven**: Ejecutar + Datos + kernel, con Z-order y foco.

**Estado real del sistema**: el escritorio arranca, lanza los tres lenguajes,
lee el disco, ensena el almacen, deja leer el log de Ring 0, y responde al
teclado y al raton. **Eso es un sistema operativo usable, no una demo.**

---

## ★★★ VERIFICADO EN EL RYZEN (2026-08-02, cuarta tanda) -- la tanda grande

Cinco de las seis cosas que estaban escritas y sin estrenar quedan **cerradas
con foto**, y la sexta la destapo el instrumento nuevo.

- **★ F11 FUNCIONA.** La ventana `RING 0 // lo que dice el kernel` sale con
  `guardadas 61 de 61` y el arranque entero legible **desde el escritorio**. Es
  la primera vez que Ring 3 puede leer lo que dijo Ring 0.
- **★★ EL DOBLE BUFER FUNCIONA, y lo dijo el mismo**: en esa ventana se lee
  `gui.bex> doble bufer: pintando fuera de la pantalla`. Como el bufer son
  **~8 MB contiguos pedidos con `KIND_MEMORIA`**, esa linea es tambien la
  **verificacion en metal de la capability de memoria**: el kernel entrego el
  bloque, el compositor pinta dentro y sigue en pie.
- **★ F12 / ESTRATOS FUNCIONA**: generacion 1, `96.00 KiB de 414.54 GiB`,
  estado holgado, **`identidad: nacio en ESTE disco`**, y `escritura: CERRADA`
  diciendo por que. El gate de identidad del section 5, en pantalla.
- **Las letras se dibujan.** El campo pinta lo que se teclea (`ls` en la foto).
- **El raton**, confirmado otra vez, y la barra de pulso se llena al moverlo --
  que es lo que esa barra existe para decir.

### ⚠ Y lo que el instrumento nuevo destapo: `ls` corria y no ensenaba nada

`ls` ejecutaba (la linea de estado decia `listo`) y la rejilla se quedaba en
blanco. **El escritor y el lector del buffer de salida miraban extremos
opuestos**: `Salida::nueva` empezaba a escribir en `fila = 0` y `pintar_salida`
ensena **las ultimas 16 filas de 200** (`celdas[184..200]`).

O sea que **las 184 primeras lineas que escribiera cualquier programa eran
invisibles**. `ls` escupe una docena: no llegaba ni de lejos. Llego con el
historial con scroll (`8ee091e2`), que movio la ventana del lector y dejo al
escritor donde estaba -- correcto cuando la rejilla eran 16 filas y punto.
Arreglado escribiendo siempre en la ultima fila. Ep. 26 de `BITACORA.md`.

---

## ★★ VERIFICADO EN EL RYZEN (2026-08-02, tercera tanda) -- con fotos

**El raton FUNCIONA.** Es el hito de la sesion: el puntero se mueve donde se
mueve la mano, y los ejes ya no van cruzados. **El arreglo de leer el Report
Descriptor (`7ffc4955`) queda CONFIRMADO** -- la pregunta de "8 o 16 bits" la
contesto el aparato, y acerto. Tres arranques y dos meses de episodios de USB se
cierran aqui (Ep. 17, 18, 19, 22, 24 de `BITACORA.md`).

Tambien confirmado en las mismas fotos:

- **Arranca directo al escritorio** con el doble bufer desplegado, o sea que
  **el compositor no murio** pidiendo 8 MB. La caja `Ejecutar` se dibuja entera:
  marco, barra de titulo, linea de pista y campo.
- **El testigo de botones responde**: pulsar cualquier boton del raton enciende
  el cuadro de 16x16 al final de la barra de pulso. Es lo que tiene que hacer.
- El log de Ring 0 llega hasta `[usb] mouse USB listo`, AHCI con
  `!p0x2 sig=0x101` (el Kingston SATA) y xHCI con `max_slots=0x40`.

### ★ Y por eso existe ahora **F11: la consola del KERNEL**

Lo que bloqueo el diagnostico no fue la falta de una teoria: fue que **la linea
que decidia no se podia leer**. Desde que el escritorio es el arranque, el panel
del kernel deja de pintarse en cuanto el compositor reclama la pantalla, y con
el desaparecia el relato entero de como arranco la maquina.

- **`ring0/core/klog.rs`**: el log del kernel se GUARDA en un anillo de 64
  lineas. Se guarda **antes** de los `return` que exigen framebuffer -- que son
  razones para no pintar, no para no recordar.
- **`TASK_OP_KLOG_INFO` (0x16) y `TASK_OP_KLOG_TEXTO` (0x17)**, calcadas de
  `INFO`/`INFO_TEXTO`. **No dan privilegio, dan vista**: Ring 3 pide texto por
  su numero y recibe bytes. En un sistema de capabilities *ver* y *poder* son
  cosas separadas, y juntarlas es como se acaba con un "modo administrador".
- **La ventana (F11)**, con color por emisor y RePag/AvPag para llegar al
  principio del arranque. Y F11 en vez de un comando por una razon de hoy:
  **no hace falta teclear nada para abrirla** -- que es justo lo que falla.

### ⚠ ABIERTO, y es lo siguiente: **no se dibujan las letras que se teclean**

El campo de la caja se queda vacio mientras se escribe. El resto de la ventana
--marco, titulo, la linea de pista, el cursor-- **si se dibuja**, asi que no es
que el compositor este muerto ni que el texto no sepa pintarse.

**Lo que hay que averiguar primero, y es UNA linea del log de arranque**:
`doble bufer: pintando fuera de la pantalla` o `SIN doble bufer`. Decide entre
dos culpables muy distintos, y hasta saberlo cualquier teoria es teoria (ley 9:
un aviso correcto no implica una teoria correcta).

**Y ahora esa linea se puede leer sin serie ni camara: se pulsa F11.** Esa es la
prueba de fuego de la ventana nueva -- si al abrirla sale el arranque entero, el
instrumento funciona y el diagnostico deja de depender de una foto.

**El discriminador de 30 segundos**, si el F11 tampoco dijera nada:
`git checkout 7f6d1085 -- Ultra_userspace/` deja el compositor **justo antes**
del doble bufer, con el arreglo del ghosting puesto. Si teclear vuelve a
pintar, el culpable es el doble bufer y esta acotado a un commit.

---

## ★ Lo ultimo que paso (2026-08-02, segunda tanda) -- leer esto primero

Tres frentes, y los tres eran "escrito y sin estrenar". Nada de esto ha tocado
un CPU todavia: es lo que hay que llevar al Ryzen en el arranque siguiente.

- **`KIND_MEMORIA` tiene por fin quien la llame**: `c/memc.bex`
  (`toolchain/lang/c/examples/memoria_C.c`) pide, escribe y relee 1024 bytes,
  marca las dieciseis paginas de un bloque de 64 KiB, y **agota el tope de
  cuatro peticiones** para ver que la quinta devuelve 0.
- **Y al escribirlo salio que el tope no se cumplia.** El `malloc` del codegen
  emitia sus saltos con desplazamientos contados a mano y el de la rama de
  fallo se quedaba **seis bytes corto**: cuando el kernel rechazaba, el CPU
  seguia a media instruccion. La rama buena estaba bien, que es lo que lo hacia
  invisible. Ahora van por etiqueta. (Ep. 23 de `BITACORA.md`.)
- **El emulador modela la capability de memoria.** Antes `TASK_OP_MEMORIA_PEDIR`
  caia en el `_ => {}` del despacho y salia por el epilogo de EXITO con el
  valor a cero: contestaba "toma tu bloque" y entregaba el puntero nulo.
- **El contador de memoria entregada se lee.** `total_entregado()` existia y
  **no lo consultaba nadie** (patron 19). Ahora es `INFO_MEM_ENTREGADA` y sale
  en `info` como `a Ring 3`. Importa porque **la linea de CABINA `mem: bloque
  entregado a Ring 3` no se puede ver desde el escritorio**: mientras el
  compositor tiene la pantalla, el panel del kernel no se pinta.
- **El raton ya no adivina su formato: lo lee.** Se le pide el Report
  Descriptor (`GET_DESCRIPTOR` tipo 0x22) y `bmo_uhid::formato` saca botones,
  X, Y y rueda con su bit y su ancho. La pregunta de "8 o 16 bits" la contesta
  el aparato, no una foto. Con reserva al formato BOOT si no se entiende, **y
  dicho**.
- **★★ DOBLE BUFER**, y es el primer cliente de verdad de `KIND_MEMORIA`. El
  compositor pide `stride x alto x 4` (~8 MiB) y **dibuja en RAM normal**,
  volcando al panel una vez por fotograma y **solo la caja de lo sucio** -- que
  la regla de esta casa sigue siendo *repintar el dano, no la pantalla*. Mata
  el ghosting **por construccion** (nunca se lee memoria WC), mata el tearing,
  pintar pasa a ser en RAM cacheada, y es la pieza que hacia falta para las
  superficies. Si no hay bloque, se dibuja en el panel como siempre **y se
  dice**.
- **★ EL GHOSTING TENIA CAUSA** (Ep. 25). El *save-under* del cursor es el
  **unico** sitio que LEE el framebuffer en todo el compositor, y lo hacia
  justo antes del unico `sfence` del fotograma: con write-combining, leer sin
  barrera devuelve la pantalla de **hace un fotograma**. Guardaba pixeles
  caducados y la vuelta siguiente los devolvia encima de lo nuevo -- un
  rectangulo de 10x16 persiguiendo al puntero. Es el Ep. 20 por el lado del
  lector. Arreglado con un `sfence` dentro de `Bajo::poner`, mas el pintado de
  la calculadora que se colaba con el cursor puesto.

---

## ★ Lo anterior (2026-08-02, primera tanda)

El dia en que **el escritorio dejo de ser una demostracion y paso a ser el
arranque**. Verificado en el Ryzen con fotos:

- **Arranca limpio al escritorio**, sin panel del kernel encima. Los cinco
  programas de ejemplo ya **no se lanzan solos**: `init_hello` reclamaba la
  pantalla, moria, y el kernel repintaba su panel sobre el escritorio recien
  nacido. Eso costo tres arranques culpando al compositor de morirse -- y el
  compositor **nunca estuvo muriendose**. El kernel adelgazo 37 KB al irse.
- **Teclear pinta al momento.** Faltaba un `sfence`: con write-combining el CPU
  retiene los pixeles hasta que el bufer se llena, y mover el raton era lo que
  lo llenaba. *"Tengo que apuntar bien para que me pinte las escrituras"* era
  eso. **WC sin barrera no es rapido: es incorrecto.**
- **Write-combining** por PAT (`MSR_PAT` llevaba declarado y **nunca se
  escribia**), y -320 ms de esperas de VBUS en el arranque.
- **El raton lo confeso el mismo**: `protocolo=0x1 (INFORME: el aparato ignoro
  el BOOT)`. Su informe lleva Report ID, por eso iba corrido un byte y se movia
  al hacer clic. Falta decidir 8 vs 16 bits de desplazamiento -- el driver ya
  registra ocho bytes crudos.

Y en el toolchain: **C completo para lo que DOOM pide** (32/32 sondas),
`static`, prototipos, varargs, arrays en agregados, `int a,b;`, y la libc
esencial en L1. Mas `KIND_MEMORIA`, que **ningun programa ha llamado aun en
metal**.

---

## Como leer este documento

Hay **tres estados**, y confundirlos es lo que hace que uno se sienta perdido:

- ✅ **corre en metal** -- se ha visto funcionar en el Ryzen, con foto o con
  linea de CABINA. Es lo unico que cuenta como hecho.
- ✍ **escrito sin estrenar** -- compila, enlaza, `bex-link` verifica sus
  direcciones... y ningun CPU ha ejecutado una sola de sus instrucciones. **No es
  lo mismo que hecho.** Es exactamente la clase de cosa que en otros proyectos
  acaba existiendo solo en la documentacion.
- ⬜ **diseno** -- pensado o escrito en un documento, sin codigo vivo.

---

## Estado global

| Componente | Estado |
|---|---|
| Boot chain (UEFI shim + s1_cpu + s2_mem) | ✅ arranca en HW real |
| Ring 0 (kernel: scheduler preemptivo, mm, caps, IPC) | ✅ estable en HW |
| Ring 3 (userspace) | ✅ varios procesos, cada uno con su espacio y sus caps |
| Fault isolation (crash R3 mata la tarea, no el kernel) | ✅ implementado |
| Boot cinematico (logo->RING0->RING3, escenas) | ✅ |
| Teclado USB (xHCI+HID) | ✅ **ESCRIBE en HW** -- el Interval del endpoint era un EXPONENTE (2^n x125us) y se escribia el bInterval crudo: un teclado que pedia 24 ms quedaba programado a 35 minutos entre sondeos. Layouts es-latam/es-espana/us, teclas muertas, AltGr, Ctrl, repeticion al mantener, LEDs, historial |
| Mouse USB | ✅ **FUNCIONA EN METAL** (2026-08-02, con foto): el puntero va donde va la mano y los botones responden. El driver **lee su Report Descriptor** y saca bit y ancho de cada campo en vez de suponerlos |
| **CABINA** (telemetria omnisciente) | ✅ **viva**: cockpit + color semantico + bitacora de eventos (narrador) + deteccion de disco PCI |
| **`KIND_FRAMEBUFFER`** (la pantalla es una capability) | ✅ Ring 3 pinta con `mov`; el kernel contesta 4 preguntas y se aparta |
| **`KIND_INPUT`** (raton, teclado **y modificadores**) | ✅ en metal; `Ctrl+Alt` detectado sin romper `AltGr` |
| **Compositor** (Ultra_userspace/services/gui) | ✅ **se carga de `sys/gui.bex`**, fuera del kernel (123 KiB; el tope son 256) |
| **Terminal de Ring 3** (caja Win+R + comandos) | ✅ **corre**: historial, TAB que completa, editor de linea con cursor, portapapeles, `ls`, `Ctrl+Alt` para invocar |
| **`KIND_CONSOLE`** (la salida es una capability, en LOS DOS sentidos) | ✅ el hijo escribe y el terminal lee; el terminal escribe y el hijo lee (`ACCEPT`) |
| **`KIND_DIRECTORIO`** (preguntar que hay en el disco) | ✅ `ls` en el terminal, iteracion sin cursor en el driver |
| **Calculadora con botones** | ✅ cara en Rust, calculo en BMO COBOL |
| **`ring0/lanzar.rs`** (buscar+firma+admitir, un solo camino) | ✅ lo usa `run` en metal |
| **ESTRATOS** | ✅ montado, superbloque leido, **firma verificada antes de ejecutar** |
| Toolchain reorganizado (lang/forge/tools) | ✅ |
| sem-asm (encoder tabla->bytes + intrinsecos) | ✅ C lo usa; fusion sem-asm<->C hecha |
| BMO COBOL | ✅ **banca cerrada en su alcance**: PICTURE de edicion en ejecucion, File I/O secuencial, OCCURS con guarda de rango, nivel 88. `batch.bex` y `concep.bex` verificados en el Ryzen |
| **BMO C ("CONTROL ABSOLUTE")** | ✅ **32 de 32 sondas del lenguaje** -- completo para lo que DOOM pide. 216 tests que EJECUTAN. `static`, prototipos, varargs, arrays en agregados, `int a,b;`. libc 11/15 |
| **BMO Ada** | ✅ **verificado en el Ryzen el 2026-07-30**, el mismo dia que nacio el compilador. Perfil ZFP + Annex F: Annex F copio el `PICTURE` de COBOL, asi que el decimal ya estaba pagado |
| C++ frontend | ◐ ~900 lineas y **desborda la pila con una clase de dos metodos**. Alcance escrito en `lang/cpp/BRECHA.md` |
| **El FOCO del escritorio** (`bmo_input::foco`) | ✅ **EN METAL** (2026-08-02): Alt+Tab con su conmutador, pila MRU, `modo: normal (Alt+M)`, el foco arrastra el Z-order. 17 tests y la foto |
| **`KIND_MEMORIA`** (un proceso pide memoria) | ✅ **EN METAL, por los dos lados**: `info` dice `a Ring 3  8.4 MiB  pedida con KIND_MEMORIA` -- lo dice el KERNEL. Su primer cliente es el doble bufer del compositor. Mas `c/memc.bex` y 7 tests que EJECUTAN |
| **Write-combining del framebuffer** | ✅ PAT programado + `sfence` por fotograma. Sin la barrera, lo pintado se quedaba en el bufer |
| **c-gen** (la fabrica que mide el compilador) | ✅ sondas que COMPILAN, censo de 91 elementos de C (25 fuera) y 49 de C++ (17 fuera) |
| **Driver de disco (AHCI/SATA)** | ✅ **LEE Y MONTA**: GPT + FAT32 + volumen de datos con escritor. El NVMe de esta maquina es el disco de **Windows** -- nunca se toca |
| **XSAVE per-task** | ✅ **resuelto y confirmado en metal** (ver abajo: la causa raiz) |

---

## Lo que corre en metal, verificado (arranque del 2026-07-27)

Esto no es una lista de intenciones -- cada linea salio en pantalla o en CABINA:

- Arranque completo **sin pantalla azul**, shell vivo, 54 eventos en CABINA.
- `fs: volumen de datos montado para ESCRITURA` - `estratos: volumen montado y
  es de este disco` - superbloque generacion 1.
- `sched: primer switch a CPL3` - `ring3: primer CONSOLE_WRITE` - cuatro
  procesos Ring 3 terminando **por su cuenta** (`EXIT`).
- `usb: primera tecla recibida: el teclado ESCRIBE`.
- **`run apps/COBOL.bex` desde ESTRATOS con la firma verificada** -> `tid 7`.
  Y el programa imprimio `3 x 19.99 = 59.97 exacto`: **decimal exacto de COBOL,
  compilado por el toolchain propio, corriendo sobre el kernel propio, en un
  Ryzen de verdad.**
- La tabla `bex` con `asm`, `C`, `COBOL`, `srv`, `cli` y `COBOL.b` -- y
  `leeme.t` marcado **RECHAZADO**: la admision BEX rechaza lo que no es un
  programa en vez de saltar al vacio.

## Verificado en el Ryzen despues (2026-07-30, con fotos)

La sesion que cerro dos dias de trabajo:

- **`batch.bex`** -- `BATCH DE CIERRE - BANCO BMO`, `total del dia: $1,135.00`,
  `cierre escrito en apps/cierre.txt`. **File I/O de COBOL en silicio**: leer un
  fichero, totalizar en decimal exacto, escribir el cierre y cerrarlo.
- **`concep.bex`** -- `$105.00 / $25.50 / $60.00 / $0.00`: **OCCURS funciona**.
- **`extracto.bex`** -- `$12,345.67`, `*****0.45` y `  120.00CR` alineados:
  **PICTURE de edicion en ejecucion**, la linea de un banco de punta a punta.
- **`cierre.bex` en ADA** -- `CIERRE EN ADA - BANCO BMO`, `59.97`, `39.98`.
  **Tercer lenguaje en silicio real.**
- **El contador de programas**: `info` dijo *17 lanzados* con *ranuras 4 en uso
  de 64*. Antes moria al tercero -- `has_room()` miraba una bitacora historica
  de 8 entradas en vez de preguntarle al planificador.
- **`info` entero**: Zen 3 (Vermeer) 19h/21h, 6 fisicos / 12 hilos, TSC medido
  3.70 GHz, **14.8 GiB totales y 5.4 MiB usados**, kernel 2.1 MiB.

## Lo que esta escrito y NUNCA ha corrido

Honestidad primero: esto es lo que hay que estrenar antes de construir encima.
La lista completa, con **como se comprueba cada cosa**, vive en la memoria de
pendientes de hardware; aqui va el resumen.

- **El raton, otra vez.** Enumera y da puntero y botones, pero el arreglo del
  **anillo de eventos compartido** (`BITACORA.md` Ep. 18) espera foto. Lo que
  hay que mirar: `apk=total:perdidos:ahora` con **perdidos en 0**, `kev=`
  subiendo al teclear y `raton ev=` subiendo al mover.
  Y ahora tambien **su formato**: en el arranque tiene que salir
  `[uhid] formato del raton: id=N x=bitN/Nb y=bitN/Nb informe=N bits`. Si sale
  `no entiendo su Report Descriptor`, el parser tiene un caso sin cubrir y los
  ocho bytes crudos del log dicen cual.
- **`KIND_MEMORIA` en metal.** Y ahora hay **dos** pruebas, porque el doble
  bufer la ejerce en el arranque:
  1. **En el log de arranque**: `doble bufer: pintando fuera de la pantalla`.
     Si sale `SIN doble bufer: no hubo bloque, pinto directo al panel`, la
     capability fallo al primer cliente de verdad y el motivo esta en CABINA.
  2. `run c/memc.bex` desde la caja: nueve lineas, la primera direccion
     `0xe0000000`, y acaba en `MEMORIA: las cuatro pruebas pasan`.
  3. `info`, fila **`a Ring 3`**: nada mas arrancar tiene que marcar **~=8 MiB**
     (el bufer del compositor, `stride x alto x 4`), y **~=76 KiB mas** despues
     de `memc.bex`. Ese numero lo da el KERNEL, no el programa -- es la
     confirmacion desde el otro lado.
- **El escritorio con foco** (`d29ad7c6`, `9d3f4943`, `345acfc5`): F12 abre la
  consola de datos de ESTRATOS, **Alt+Tab** recorre la MRU con su ventanita,
  **Alt+M** rota el modo, el clic da el teclado y **el foco arrastra el
  Z-order**. Y el cursor del raton ya no agujerea las ventanas (*save-under*).
- **La escritura de ESTRATOS**: la transaccion esta escrita y probada (12
  tests) y **nadie la ha cableado al dispositivo**. La ventana de datos lo
  dice en rojo -- si algun dia aparece en verde sin cablearla, eso es el bug.
- **La calculadora con botones**: el motor `cobol/calcgui.bex` compila y el
  panel dibuja, pero nadie ha pulsado `=` en metal.

Lo que SI se estreno: el terminal dibujando, la fuente en Ring 3, `tecla()`,
`OP_EJECUTAR`, el compositor desde disco, `KIND_CONSOLE`, `ACCEPT` de COBOL con
un importe tecleado, y los tres lenguajes.

---

## Kernel (Ultra_kernel_x86-64/)

Funciona en HW real: boot chain unificado (BOOTX64.EFI embebe s1/s2/kernel),
GDT/IDT propias, paginacion (physmap 16 GiB, kernel-half pre-poblado),
scheduler preemptivo por LAPIC timer, Capability Engine, BMO Channel (IPC),
3 syscalls, fault isolation. **Bugs raiz historicos resueltos** (ver BITACORA):
CS fantasma UEFI, split-brain de gs, framebuffer bajo CR3 usuario, stacks no
contiguos.

**Teclado USB -- RESUELTO.** El `Interval` del Endpoint Context de xHCI es un
**exponente** (2^n x 125 us) y se escribia el `bInterval` crudo del descriptor,
que en Low/Full Speed viene en **milisegundos**: un teclado que pedia 24 ms
quedaba programado a **35 minutos** entre sondeos. Hoy `usb: primera tecla
recibida` sale en CABINA en cada arranque. El debug vive en la fila `usb`
(`kev/tev/hev/dci/lev`).

**XSAVE -- la causa raiz (2026-07-27, cinco sondas y cuatro pantallas azules).**
`XSAVE` **no inicializa la cabecera XSAVE: hace MERGE.**

```text
XSTATE_BV <- (XSTATE_BV_viejo AND NOT RFBM) OR (XINUSE AND RFBM)
```

con `RFBM = EDX:EAX AND XCR0`, y **no toca** los 48 bytes reservados. Los stubs
tallan su area sobre la pila (`sub`+`and`), o sea sobre basura, y esa basura
sobrevivia al guardado en los bits altos -> `XRSTOR` la rechaza con `#GP(0)`.
`trap::fabricate` nunca lo sufrio porque pone a cero los 1024 bytes antes de
nada; los stubs no. **Esa era la asimetria.** Arreglo: los prologos ponen a cero
la cabecera **entera** (512..575) antes del `xsave64`.

*La firma que lo delato*: los volcados daban `0x5F0FCB` y `0x37B`, y los dos son
**el valor viejo con los tres bits bajos puestos a 3** -- que es exactamente
`XINUSE & 7`. Un campo corrupto con unos pocos bits bajos coherentes no es
corrupcion: es una instruccion haciendo merge donde creiamos store.

*Defensas que quedan puestas*: guardia de cabecera en los cinco epilogos
(motivo `PODRIDO_CABECERA`), anillo de las ultimas areas publicadas
(`pub0..pub3`) con su tid, y las sondas `bv0`/`bvX`/`baseX`. El informe de fallo
es el unico depurador que hay en esta maquina -- por eso se quedan.

**CABINA (ring0/cabina.rs)** -- telemetria omnisciente, always-on desde el shell
loop (NO desde el timer IRQ: causaba cuelgue->reset). Da vida a `cabina-core`:
`snapshot()` desde contadores vivos + `render_hud()` pinta bitacora de 9 lineas
(eventos con severidad/capa/color) + 3 de telemetria compacta. `record()`/
`info/warn/fault` = el narrador; ring de 48 eventos. `find_storage()` en dev/pci
detecta el controlador de disco (NVMe/AHCI). Color: verde=bien, ambar=atencion,
rojo=problema. Anti-ghosting por change-detection + SCREEN_GEN.

**Pendiente kernel**: capability de **memoria** -- un proceso recibe su imagen
y 64 KiB de pila y no puede pedir mas. Bloquea DOS cosas a la vez: cualquier
lenguaje con GC, y las **superficies compartidas** que hacen falta para
ventanas de verdad (hoy `KIND_FRAMEBUFFER` es exclusivo, un solo proceso es
dueno de la pantalla). Despues: CABINA caja negra en disco, demand paging,
endpoint RPC (servidores Ring 3), EXIT-reclaim, SMP.

**Hecho desde entonces**: `KIND_DIRECTORIO` (hay `ls`), modificadores en
`INPUT_OP_MODIFICADORES` (hay `Ctrl+Alt`), `KIND_CONSOLE` en los dos sentidos
(hay `ACCEPT`).

**Deuda visible**: `services/input` es una carpeta que promete un multiplexor de
entrada y esta vacia -- la entrada la reclama el compositor directamente. O se
cablea o se borra, como se borro `apps/terminal`. Y el **manifest BEF**
(`provides`/`requires`, en `platform/abi/bmo-abi/src/bef/manifest.rs`) tiene
struct y parser TOML completos, y **el kernel no compila `bmo-abi`**: `build.ps1`
lo lee como TEXTO para el drift guard y nada mas. Es el prerequisito si algun dia
se quiere clasificar programas por lo que le PIDEN al kernel (AOT / GC / GIL).

---

## Toolchain (toolchain/)

```
toolchain/
  lang/    frontends (esencia): c, cobol, cpp, base(stdlib)
  forge/   pipeline compartido: sem-asm(encoder ✅), bmo-verify(gate ✅)
  tools/   generadores: bef-bootstrap, hello-bex, fontgen, bmo-linker, cobol-gen(Python)
```

### ★ El emulador, y hasta donde llega (auditado 2026-08-02)

`bmo-lower::emu` es lo que hace que los tests del toolchain **ejecuten** en vez
de mirar bytes, y es la razon de que 574 pruebas signifiquen algo. Pero su
cobertura **no esta repartida -- esta concentrada**, y confundir eso es como se
acumulan cosas verdes que nunca han corrido. El detalle entero vive en la
cabecera de `toolchain/forge/bmo-lower/src/emu.rs`, seccion **FIDELIDAD**; el
resumen:

| Eje | Cobertura | Por que |
|---|---|---|
| los bytes calculan lo que dice la fuente? | **alto** | es para lo que se construyo; cazo el salto corto de `malloc` |
| el kernel hace lo que el modelo dice? | **cero** | **no ejecuta el kernel: lo imita**. Si los dos se separan, los dos parecen sanos |
| lo fisico (paginacion, anillos, XSAVE, IRQs, DMA, WC, USB, tiempos) | **cero** | por construccion. Los 24 episodios de `BITACORA.md` son de aqui |

**Los agujeros con nombre**: no hay SSE (y por eso los 9 tests de float no
ejecutan ninguno), la memoria es un mapa disperso (toda direccion funciona: sin
fallos de pagina ni aliasing), no hay tope de pila (el proceso real tiene 64
KiB), y **no hay cargador** -- el banco rearma las secciones a mano, asi que el
cargador del kernel y la admision de `bmo-verify` no se ejercen.

**La regla de reparto**: lo que se puede equivocar en la aritmetica o en el
flujo, en el emulador; lo que depende del silicio o del kernel, en el Ryzen, y
**con su numero escrito antes de arrancar**. El valor del emulador no es un
porcentaje: es el coste por bug -- segundos aqui, contra flashear + reiniciar +
fotografiar + una teoria que puede estar mal.

- **sem-asm** ✅: motor que lee `forge/sem-asm/tables/*.toml` y encodea
  instrucciones->bytes. C y COBOL migrados a usarlo (fuera bytes hardcodeados).
- **bmo-verify**: gate que valida el BEF (delega en `bmo-abi::bef::validator`,
  el validador real de 15 tests). `bmo-lower` (descenso ABI) y `bmo-opt`
  (optimizacion) se recrearan con codigo real al empezar su fase -- no stubs.

---

## BMO C -- "CONTROL ABSOLUTE" (toolchain/lang/c/) -- MUY completo

C esencial de Ritchie (~C11). **85 tests verdes.** Modulos: `standard.rs`
(versiones C89..C23, tablas en forge/sem-asm), `lexer.rs`, `parser/mod.rs`,
`ast/`, `codegen.rs` (el "diccionario" -> bytes exactos, sin cerebro intermedio
tipo LLVM), `module.rs`.

**Fases HECHAS (2026-07-23/24):**
- **F0 -- cimientos honestos**: exterminados ~10 "silencios traicioneros" (bytes
  MAL sin avisar): offsets `a->b->c` anidados, `int **pp`, sufijos `10UL`,
  `arr[i]=x` que se descartaba, `TypeSpec::Array(elem,n)` con tamano real,
  decls anidadas sin slot (for infinito), subscript array-vs-puntero, stores de
  campo con tamano exacto (`pt.x` ya no pisa `pt.y`), casts reales (movsx/movzx),
  errores con LINEA real. Criterio: "un diccionario no adivina".
- **F1 -- LA FUSION sem-asm<->C**: `tables/arch/x86_64/intrinsics.toml` +
  `__hlt/__pause/__rdtsc/__outb/__inb/__wrmsr/__cpuid`. El compilador emite los
  BYTES EXACTOS de la tabla (no caja negra tipo `asm()`); agregar instruccion =
  1 entrada TOML, cero Rust.
- **F2 -- completo**: punteros a funcion (`int (*op)(int)`, decadencia, call rax
  indirecto = base de vtables C++), subscript compuesto (`p->arr[i]` = IndexPtr),
  `(*fp)(args)` (CallPtr), **floats SSE** (ruta xmm paralela: literales, +-x/,
  comparaciones comisd, cvtsi2sd/cvttsd2si, retorno en xmm0; float globales y
  args-de-funcion = deferido honesto).
  ✅ **Y desde el 2026-08-02 la ruta SSE EJECUTA**: el emulador modela las
  quince instrucciones escalares que emite BMO C, y hay 7 tests que corren de
  verdad. Antes: de los **9 tests de coma flotante, 0 EJECUTABAN** -- los nueve comparan ventanas de bytes
  (`bef.windows(3).any(...)`), que es el metodo que el propio emulador declara
  insuficiente en su cabecera. El emulador **no tiene SSE**, asi que esa ruta
  entera compila, da verde y **ningun CPU la ha ejecutado**. Es la misma forma
  que tenia el bug de `malloc` (Ep. 23). Lo que lo arregla es meter `xmm` al
  emulador, no escribir mas tests de bytes.

**FALTA C** (por orden de lo que mas duele):

1. **ENTRADA. No puede leer NADA** -- ni `scanf` ni `getchar`. Tiene `printf` y
   106 tests verdes, o sea que habla y no escucha. Es exactamente el hueco que
   COBOL tenia hasta el 2026-07-28, y ahora es barato: `console::read_line` y
   `fmt::parse_decimal_scaled` ya existen en `bmo-lower` y **no son de ningun
   lenguaje** -- se comparten igual que el conversor de enteros.
2. `printf %f` y float args por ABI xmm; float globales.
3. Preprocesador completo.
4. **stdlib (`impl.c`)** -- y esta es la de verdad: *la universalidad de C no
   viene del lenguaje, viene de libc*. Sin biblioteca estandar, C es un
   ensamblador portable con llaves. Es lo que `bmo-rt` tiene que llegar a ser.

Base solida para C++ (hereda lexer/tablas/intrinsecos/codegen; solo pone RAII
+ vtables encima).

---

## BMO COBOL (toolchain/lang/cobol/)

Ver `ARCHITECTURE.md` y `cobol.md` en esa carpeta.

> **Aqui no se pone un porcentaje, y es a proposito.** "COBOL al 15%" da a
> entender que existe un 100% -- un denominador. No existe: el estandar sigue
> creciendo y ningun compilador del mundo lo implementa entero. Medirse contra
> un infinito no informa de nada y solo sirve para sentirse pequeno. Lo que si
> se puede afirmar y comprobar es **que corre**, y cada linea de abajo tiene su
> fila en la matriz de conformidad, que EJECUTA lo que dice soportar.

**CORRE** (verificado ejecutando, no leyendo bytes):
- **Lexer** (`lexer.rs`): Source->Tokens; `.` decimal vs terminador; usa tablas.
- **Parser de tokens** (`tparser.rs`): sentencias + DATA DIVISION + programa
  completo -> AST. Camino paralelo al `parser.rs` por-lineas (aun el principal).
- **PIC propio** (`pic.rs`): 100% BMO, sin gnucobol-rs (GPL). Da la escala.
- **Decimal EXACTO** (`codegen.rs`): ADD/SUB/MUL/DIV escalan por el PIC ->
  centavos sin float. **El alma bancaria de Grace Hopper.** Confirmado en el
  Ryzen: `3 x 19.99 = 59.97`.
- **Flujo de control real**: IF/ELSE anidado y con AND, PERFORM TIMES,
  PERFORM UNTIL, COMPUTE con precedencia y parentesis.
- **DISPLAY** de literal y de variable, **ACCEPT** por el anillo de entrada
  de la consola.
- **PICTURE de edicion EN EJECUCION** (`edicion.rs`): `$$$,$$9.99`,
  `**,**9.99`, `Z,ZZ9.99CR`, `DB`, signos fijos y flotantes, `99/99/99`.
  El recorrido de la plantilla se emite como INSTRUCCIONES: en el `.bex` no
  queda ni la mascara ni un interprete que la lea. Atado a `formatear` por
  238 casos ejecutados en el emulador. Ver `examples/extracto.cob`.
- **Fabrica Python** (`tools/cobol-gen/`): genera `generated/words.rs` (556
  reservadas separadas ESENCIA vs VENDOR, 55 intrinsecas). Organizada en
  `defs/{words,verbs,intrinsics,grammar}.py`.
- Pipeline end-to-end probado: Source->lexer->tparser->AST->codegen->BEF (magic BEF1).
- **71 tests verdes.**

**NO CORRE** (y se dice, en vez de fingirlo):
- **File I/O** (`SELECT`/`FD`/`OPEN`/`READ`/`WRITE`/`CLOSE`) -- se RECHAZA con
  su motivo en vez de compilar un READ que no lee. **El siguiente grande**: sin
  ficheros no hay batch, y debajo ya estan el disco, FAT32 y el gate.
- DATA: records anidados (grupos 01/05/10), OCCURS, REDEFINES, nivel 88/66,
  COMP-3 real.
- Verbos: EVALUATE, PERFORM VARYING, STRING/UNSTRING, INSPECT, SEARCH, CALL,
  SORT.
- Subindices, 55 intrinsecas (0 implementadas), runtime (bmo-rt), COPY,
  formato fijo/libre.
- Cablear `tparser::parse_program` como principal (jubilar `parser.rs`).

**Regla de la esencia**: "el encoder puede ser compartido; la aritmetica de
COBOL jamas. El decimal es sagrado, vive solo en lang/cobol." GnuCOBOL infla a
1130+ palabras porque **traduce a C**; BMO compila **nativo** y separa esencia
de vendor. **COBOL devorado -> BMO COBOL.**

---

## Filosofia / arquitectura (los principios)

1. **3 syscalls congelados + subsyscalls**: `INVOKE`/`KICK`/`WAIT` nunca
   cambian; todo lo demas son operaciones sobre capabilities (modelo seL4/Zircon,
   no Windows). Ver README raiz "Subsyscalls".
2. **Contratos y librerias, NUNCA cerebros**: se comparten formatos (BEF, ABI)
   y librerias opcionales; jamas un IR/embudo central (seria monolito).
3. **Library OS + Devour_System**: superficies ajenas (Win32, POSIX) se
   traducen a subsyscalls -> nativo. El kernel no sabe que existieron.
4. **Borrar costos, no optimizarlos**: library OS borra la frontera de syscall;
   lenguajes nativos borran el impuesto del C ABI; perfil per-CPU borra el
   impuesto generico.
5. **Python = fabrica de tablas** (dev-time), nunca entra a BMO. Genera lo
   TABULAR (~40%); la semantica/codegen es Rust (~60%).

---

## Flujo de trabajo

**Compilar + desplegar a hardware (Ring 0 Y los programas, de una vez):**
```bash
cd C:\Users\Salazar\Documents\BMO\Ultra_kernel_x86-64
.\build.ps1 -Flash -Drive A -Data A -Yes
bcdedit /set "{fwbootmgr}" bootsequence "{57cb1744-7f84-11f1-930d-c3a2d7ca848a}"
shutdown /r /t 5
```
En esta maquina el volumen de arranque y el de programas son **el mismo** (A:,
la particion 2 del Kingston SATA), asi que las dos banderas llevan la misma
letra -- pero siguen siendo dos banderas, porque son dos riesgos.
(El one-shot arranca BMO-X una vez y vuelve a Windows. Si el video del firmware
falla: **apagado completo** re-inicializa el VBIOS. F11 tapado por Windows
Boot Manager primero en BootOrder.)

**Regenerar las tablas COBOL (Python):**
```bash
py toolchain/tools/cobol-gen/generate.py
```
(Python 3.13 instalado en `%LOCALAPPDATA%\Programs\Python\Python313\`.)

**Tests:**
```bash
cargo test -p bmo-c-front       # 223 verdes: EJECUTAN el programa, no lo miran
cargo test -p bmo-cobol-front   # COBOL, con el banco de matriz
cargo test -p bmo-input         # 17 del FOCO (Alt+Tab, modos, Z-order)
cargo test -p bmo-uhid          # 21: el Report Descriptor y el descifrado del raton
cargo test --workspace --exclude bmo-kernel --exclude boot-context --exclude bmo-rt
```
Lo ultimo son **620 verdes y CERO rojos**.

★ **`boot-context` con GUION.** Estaba escrito `boot_context` con guion bajo, que
no es el nombre de ningun paquete -- cargo se lo tragaba en silencio y ese crate
llevaba entrando en la suite todo el tiempo. Una exclusion que no excluye nada
es peor que ninguna: hace creer que algo esta apartado cuando no lo esta. Las cuatro exclusiones no son cosmetica: el
kernel y `boot_context` son `no_std` y `cargo test` les mete `std` encima
(`duplicate lang item panic_impl`); `byte-defender` y `bmo-rt` estan rotos
desde hace tiempo y son parte de la deuda tecnica anotada.

**Copiar los programas de Ring 3 al volumen de datos:**
```bash
cd Ultra_kernel_x86-64; .\build.ps1 -Data A
```
El `.bex` del compositor sale a `staging\BMO-DATA\sys\gui.bex` en cada build y
de ahi se copia. `RUTA_COMPOSITOR` en `phase.rs` es `sys/gui.bex` (8.3: el
driver FAT32 no lee nombres largos y no recorta) -- la ruta de dentro del
volumen es el contrato entre el build y el arranque, y el resto va por
categorias: `cobol/ c/ ada/ datos/`. El mapa completo, en
`Ultra_kernel_x86-64/VOLUMEN.md`.
Tres cierres antes de escribir un byte: **nunca el disco del sistema**, tiene que
ser FAT/FAT32, y hay que teclear `DATA <letra> BMO`. Es el UNICO sitio del build
que escribe fuera del arbol del proyecto. `-Flash` es aparte y es para Ring 0:
las dos banderas tocan discos distintos a proposito.

**Compilar solo el kernel (sin flashear) para verificar cambios:**
```bash
cd Ultra_kernel_x86-64; .\build.ps1 -BuildOnly
```
(El kernel es bare-metal; `cargo build --workspace` falla al linkear con
link.exe del host -- usar build.ps1. Nota commits: mensajes con `->`/comillas/
parentesis rompen el heredoc de PowerShell -- usar `git commit -F archivo`.)

---

## Docs de referencia

- `BITACORA.md` -- bitacora de guerra del debugging en HW (11 episodios).
- `README.md` (raiz) -- arquitectura, Subsyscalls, boot path.
- `toolchain/lang/cobol/ARCHITECTURE.md` -- pipeline COBOL completo + roadmap.
- `toolchain/lang/cobol/cobol.md` -- esencia/teoria de COBOL en BMO.
- `toolchain/forge/README.md` + `toolchain/README.md` -- pipeline y estructura.
- `toolchain/tools/cobol-gen/README.md` -- la fabrica Python.
- `platform/abi/bmo-abi/src/ENDPOINT_RPC.md` -- diseno RPC a Ring 3.

---

## Proximos frentes (prioridad)

**HECHO desde el 2026-07-25** (estaban aqui y ya no): FAT32 + volumen de datos
montado, gate de identidad antes de escribir, XSAVE per-task (y su causa raiz),
`.bex` fuera del kernel (el compositor se carga de disco), ESTRATOS montado con
gate de firma.

**HECHO desde entonces** (2026-07-28): la caja estrenada, el terminal con
comandos e historial, modificadores (`Ctrl+Alt`), `KIND_DIRECTORIO` (`ls`),
`KIND_CONSOLE` en los dos sentidos, `DISPLAY <var>` y `ACCEPT` en COBOL, y la
calculadora.

**HECHO desde entonces** (2026-07-29/31), y con eso **COBOL para banca queda
cerrado en su alcance declarado**: PICTURE de edicion en ejecucion, File I/O
secuencial, OCCURS con guarda de rango, nivel 88, entrada en BMO C
(`getchar`/`scanf`), **Ada verificada en el Ryzen**, el volumen de datos por
categorias, `info`/`cpu`/`mem` desde Ring 3, el historial con scroll, y el
escritorio con foco (F12, Alt+Tab, Alt+M). Lo que le queda a COBOL --`EVALUATE`,
`STRING`, `SEARCH`, `CALL`, `SORT`-- es **cola larga del estandar, no banca**.
**COMP-3 ya no esta en esa lista: entro el 2026-08-03** y guarda nibbles de
verdad. Lo que si sigue siendo banca y falta son los **registros binarios** y el
**indice por clave**; ver `toolchain/lang/cobol/BANCA_REAL.md`.

**Kernel/HW (orden vigente 2026-08-02):**

**Antes que nada: EL ARRANQUE PENDIENTE.** Hay tres cosas escritas y sin
estrenar, y cada una tiene su prueba exacta arriba. Cuanto mas crezca la pila
sin verificar, mas dificil es saber cual de las tres rompio algo si falla:
el foco entero (F12, Alt+Tab, Alt+M, clic), `run c/memc.bex` + `info`, y el
formato del raton en el log de arranque.

1. ~~**Capability de MEMORIA**~~ -- **HECHA** (`a9ccd4f8`), con su programa y
   su contador en `info`. Falta la foto.
2. **Cablear la escritura de ESTRATOS al dispositivo.** La transaccion existe y
   esta probada (12 tests); faltan el `write` y el `FLUSH CACHE` de verdad. Es
   lo unico que separa "un almacen que se lee" de un almacen. **Es el frente
   grande que queda.**
3. ~~**Write-combining del framebuffer**~~ -- **HECHO y verificado** (`952681c7`
   + el `sfence` de `3409ea8e`).
4. **Ada hacia ACATS** -- el estandar trae su propio banco de conformidad, que
   es la forma honesta de medir cuanto Ada hay de verdad.
5. **Superficies y ventanas** -- hoy `KIND_FRAMEBUFFER` es exclusivo. Wayland
   en pequeno, y ahora **ya tiene debajo lo que le faltaba**: la memoria
   compartida entre procesos se pide con `KIND_MEMORIA`. Es lo que saca la
   calculadora del
   compositor a su propia ventana **sin tocar el COBOL**. La politica de foco
   ya esta escrita y probada, asi que ese dia no hay que inventarla.
6. **Endpoint RPC -> servicios Ring 3**: el momento library-OS.
7. **SMP al final**: el codigo de despertar los APs YA EXISTE en s1_cpu
   (trampolin, INIT+SIPI, GDT/IDT), pero `smp_startup()` no tiene ni una
   llamada y `ap_entry64` solo cuenta y hace hlt. Va el ultimo a proposito: el
   dia que corra un 2o nucleo, cada `static mut` del kernel es una carrera.

**Palancas de velocidad ARQUITECTONICAS (no micro-optimizacion):** sin cruce de
anillos (library OS), DMA directo al buffer del llamante (hoy hay pagina de
rebote), NCQ (el HBA declara 32 ranuras, se usa 1) e interrupciones MSI en vez
de sondeo.

**Sistemas de ficheros ajenos:** leer NTFS es viable HOY -- el crate `ntfs` de
ColinFinck es no_std, MIT/Apache y esta pensado para firmware y drivers de
kernel. Escribirlo no: no hay nada seguro que enlazar. La decision es del dueno,
no una imposibilidad tecnica.

**Filosofia politica grabada (2026-07-24)**: BMO-X = "dictadura absoluta pero
benevolente" -- cero-confianza en el CODIGO (capabilities + bmo-verify), soberania
del DUENO, transparencia total (CABINA lo confiesa todo). Trade-off honesto:
software que exige opacidad (DRM/anti-cheat de kernel) se auto-excluye. No es
pirateria; es "esta maquina me obedece solo a mi". Consola-con-esteroides + PC.

**Lenguajes:**
0. **SSE en el emulador** -- y va delante de C++ a proposito, porque es barato y
   tapa un agujero que YA existe en vez de abrir uno nuevo: hoy la ruta de coma
   flotante de BMO C tiene 9 tests y **ninguno la ejecuta**. Ademas C++ hereda
   esa ruta entera, asi que construir encima sin ejecutarla es apilar sobre algo
   que nadie ha visto funcionar.
5. **BMO C++ (esencial, ACOTADO)** -- SIGUIENTE lenguaje; barato encima de C
   (hereda todo). NO es "todo C++". Alcance deliberado =
   desde Bjarne (origen) hasta lo ESENCIAL de C++17, sin la bola moderna.
   - DENTRO: clases/structs, ctor/dtor (RAII), referencias, sobrecarga,
     herencia + virtuales (vtables, ya presente), namespaces, templates
     basicos, new/delete, auto, range-for, nullptr, constexpr basico, lambdas.
   - FUERA (la "basura" que hunde el barco, cf. Stroustrup "Remember the
     Vasa!"): concepts, coroutines, modules, ranges, STL gigante,
     metaprogramacion pesada, C++20/23, el treadmill moderno.
   - Los 3 syscalls + runtime minimo (bmo-rt) lo hacen FINITO/terminable:
     no necesita std::thread/filesystem/etc. **C++ congelado en su esencia.**

**Desktop (F5)**: compositor sobre Endpoint RPC, estetica Win11+Mac cyberpunk.
