# LA PUERTA POR DENTRO -- todos los elementos, en CICLOS

> Capitulo de componente, como `docs/EL_TECLADO_EXIGE.md`. Alli la pregunta era
> *"que exige el teclado"*; aqui es **"por donde se van los ciclos de una
> llamada al sistema, y cual de ellos se puede tocar"**.
>
> Escrito el **2026-08-17** con la tanda del Ryzen delante (`CCOSTE.TXT`,
> `SYSPRECI.TXT`). Todo numero de este fichero o esta MEDIDO en esa tanda o dice
> que no lo esta.

---

## 0. La unidad, antes que nada: **ciclos, no ticks**

La maquina trajo sus dos relojes en la misma pantalla, y son distintos:

```
   [MEDIDO]  reloj base    3700 MHz    el TSC -- lo que cuenta `rdtsc`
   [MEDIDO]  reloj ahora   4529 MHz    MPERF/APERF -- a lo que va el nucleo
```

El TSC es **invariante**: cuenta a la frecuencia base pase lo que pase con el
boost. Asi que:

```
   1 tick de TSC   =  1,22 ciclos de nucleo  =  0,27 ns
   792 ticks       =  969 ciclos             =  214 ns     <- una puerta pelada
```

★ **Y el propio metro llevaba desde el primer dia imprimiendo `ciclos/op` donde
median TICKS**: un error del 22%, el patron 2 de la casa (el campo que viene en
otra unidad). Corregido el 17-08 en los dos testigos; la conversion vive en
`bmo-juicio::Reloj` con pruebas de anfitrion, y **los presupuestos siguen en
ticks a proposito** -- convertir antes de comparar contra el techo moveria el
trinquete cada vez que el CPU cambia de frecuencia, que es justo lo que un
trinquete no puede hacer.

---

## 1. El recorrido entero, elemento a elemento

Una puerta pelada --`INVOKE` sobre `CURRENT_TASK`, la mas barata que existe--
recorre esto, en este orden:

| # | elemento | donde vive | que hace |
|---|---|---|---|
| 0 | el envoltorio de Ring 3 | [`userland/src/sys.rs:38`](Ultra_userspace/userland/src/sys.rs) | 5 registros y `syscall`. `#[inline(always)]`: no hay llamada |
| 1 | **la transicion CPL3 -> CPL0** | el CPU | `syscall`: carga CS/SS de `IA32_STAR`, RIP de `LSTAR`, enmascara RFLAGS con `SFMASK` |
| 2 | `swapgs` + cambio de pila | [`entry.rs:46`](Ultra_kernel_x86-64/kernel/src/ring0/syscall/entry.rs) | 3 instrucciones: GS del kernel y la pila de syscall (32 KiB por CPU) |
| 3 | la cola del marco | `entry.rs:58` | 5 `push`: SS, RSP, RFLAGS, CS, RIP |
| 4 | los 15 registros | `entry.rs:64` | 15 `push`. 160 B en total con lo anterior |
| 5 | el area de contexto | `entry.rs:83` | `sub rsp, 1096` + alinear a 64 + back-pointer + sello |
| 6 | publicar el contexto | `entry.rs:118` | `gs:[0x10]`, `cld`, `rdi = marco` |
| 7 | **`call dispatch`** -- la mitad Rust | [`syscall/mod.rs:1084`](Ultra_kernel_x86-64/kernel/src/ring0/syscall/mod.rs) | ver el desglose de abajo |
| 8 | la bifurcacion | `entry.rs:145` | `cmp rax, rsp`: 1 comparacion de registros decide si hubo cambio de tarea |
| 9 | volver | `entry.rs:168` | borrar el sello + 15 `pop` |
| 10 | canonicidad | `entry.rs:233` | 3 instrucciones. Cierra el CVE clasico de `sysret` |
| 11 | **la transicion CPL0 -> CPL3** | `entry.rs:243` | `swapgs` + `sysretq` |

**Y dentro del 7, la mitad Rust:**

| # | elemento | donde vive | cuando se paga |
|---|---|---|---|
| 7a | `registrar_publicacion` | [`plat/trap.rs`](Ultra_kernel_x86-64/kernel/src/ring0/plat/trap.rs) | siempre. Dos escrituras de diagnostico |
| 7b | clasificar la puerta | `mod.rs:1112` | siempre. Tres hechos que ya estan en registros, **sin lista** |
| 7c | el `match` de la operacion | `mod.rs:745` | siempre |
| 7d | **resolver la capability** | [`obj/cap.rs:189`](Ultra_kernel_x86-64/kernel/src/ring0/obj/cap.rs) | solo si el handle **no** es `CURRENT_TASK` |
| 7e | el trabajo pedido | segun la operacion | de 36 ciclos (`OP_INFO`) a 2,7 M (consola) |

---

## 2. Lo que esta MEDIDO y lo que no

```
   MEDIDO (Ryzen, 17-08)                          ticks    ciclos     ns
   ------------------------------------------    ------   -------   ----
   puerta pelada, testigo de Rust                   779       953    210
   puerta pelada, testigo de C                      792       969    214
   una operacion mas gorda (7e, OP_INFO)             30        36      8
   resolver una capability (7d), sonda aislada      181       221     49
   un sello `rdtsc` en bucle apretado               107       130     28
   un sello `rdtsc` en bucle flojo                   69        84     18
```

```
   NO MEDIDO -- y es la lista de lo que hay que medir
   ------------------------------------------------------------------
   1 + 11   las DOS transiciones de privilegio      el sospechoso #1
   2..6     el prologo del stub (31 instrucciones)
   8..10    el epilogo de la via rapida (26)
   7a..7c   la cabecera de `dispatch`
```

★ **Los dos testigos coinciden en un 1,7%** (953 vs 969 ciclos) midiendo con
compiladores distintos y bucles distintos. Eso no valida la puerta: **valida el
instrumento**, que es lo que permite creerse el resto de esta pagina.

[!] Y la unica cifra en la que **no** coinciden es el handle: la sonda de Rust
dice 221 ciclos y la resta del programa de C dice 383. No se promedian. La de
Rust aisla la variable --misma operacion gorda, lo unico que cambia es que la
capability sea real-- y la de C compara dos filas que se diferencian en dos
cosas a la vez. **La buena es 221**; los 383 miden otra pregunta.

---

## 3. La aritmetica que TACHA: 58 instrucciones no son 969 ciclos

La via rapida del stub, contada instruccion a instruccion sobre `entry.rs`:

```
   prologo      31 instrucciones   (swapgs, pila, 20 push, area, sello, publicar)
   el `call`     1
   epilogo      26 instrucciones   (sello a cero, 15 pop, canonico, sysretq)
                --
                58 instrucciones de ensamblador por puerta
```

Un Zen 3 retira hasta 6 instrucciones por ciclo y ninguna de estas es
complicada. Aun contandolas a **una por ciclo** --lo mas pesimista posible-- eso
son 58 ciclos de los 969.

> ★★ **O sea que el 94% de una puerta NO esta en el numero de instrucciones del
> stub.** Esa resta es la que ordena todo el trabajo que queda, y deja tres
> sospechosos y solo tres:
>
> 1. **las dos transiciones de privilegio** (`syscall` + `sysretq`),
> 2. **los dos `swapgs`**,
> 3. **la mitad Rust** (`dispatch`), que hoy no se mide.

Y por eso **optimizar el stub instruccion a instruccion es el trabajo
equivocado**: aunque se quitaran diez de las 58, se ganaria un 1%. Lo que movio
la aguja de verdad fue quitar TRABAJO --el `xsave64` de todas las puertas, los
cuatro sellos-- no instrucciones sueltas.

---

## 4. La prioridad, en orden, y el experimento que decide cada una

### P1 -- Repartir los 969 ciclos. **Sin esto, lo demas es adivinar**

Es el 100% de toda puerta y hoy es un solo numero sin partes. El reparto existio
--el 16-08 dijo 318 dentro de `dispatch` y 2345 fuera-- y se retiro porque
cobraba 92 ticks (112 ciclos) en cada puerta de cada programa.

★ **No hay que reescribir nada: el metro sigue en el arbol detras de un
interruptor.** Una tanda con `--features metro_puerta` devuelve el reparto, y lo
que cuesta esa tanda esta medido: 112 ciclos por puerta, un 11%.

```
   que hacer      build con `metro_puerta`, correr `sys/precio.bex`, y volver
                  al build normal
   que contesta   cuanto de los 969 es Rust y cuanto es el camino de ida y
                  vuelta al CPU
   que descarta   si `dispatch` sale pequeno (<150), la mitad Rust queda
                  TACHADA y todo el trabajo se va a las transiciones
```

### P2 -- Resolver una capability: 221 ciclos, el 23% de una puerta con handle

Lo que hace [`cap::resolve`](Ultra_kernel_x86-64/kernel/src/ring0/obj/cap.rs)
para una LECTURA, y los tres sospechosos con nombre y fichero:

```
   pushfq + pop + cli        plat/spin.rs:86    leer y apagar RFLAGS
   LOCK xchg                 plat/spin.rs:89    el cerrojo, aunque no haya nadie
   tabla de 16 x 64 x 24 B   obj/cap.rs:116     24 KiB; el slot puede venir frio
```

★ **Y hay una asimetria que ya es un dato**: `INFO` sobre la misma pseudo-cap
cuesta 36 ciclos y sobre una capability real cuesta 221. La diferencia no es el
trabajo pedido --es el mismo-- es **llegar hasta el permiso**.

[!] La idea obvia --*"quitar el cerrojo, que solo se lee"*-- **no se apunta como
solucion sino como pregunta**, porque el cerrojo protege contra `revoke` en
medio, y el dia que haya un AP en Ring 0 eso deja de ser teorico. Lo que si es
seguro medir antes: cuanto de los 221 es el cerrojo y cuanto la tabla. Se
contesta con la misma tanda de P1.

### P3 -- La operacion pedida: 36 ciclos, el 3,7%

`OP_INFO` es un `match` y una lectura. **Esto ya esta tachado**: no hay nada que
ganar donde solo hay 36 ciclos.

### P4 -- La consola: ~2,7 M ciclos por puerta

Es 2.800 veces una puerta normal, y no porque la puerta sea cara: **dibuja
glifos y hace scroll**. No es un problema del camino de la llamada; es un
problema de dibujo, y vive en otro eje.

★ **Y aqui es donde entra el instrumento nuevo del 17-08**: `sys/precio.bex`
imprime ahora **cuantas veces se pide cada clase de puerta** desde el arranque
(tarea / handle / consola / wait). El kernel lo clasificaba desde el 16-08 y
nadie lo leia. Sin ese numero, `coste x veces` no se puede calcular y la
prioridad es una intuicion -- que en este camino ya se equivoco dos veces.

---

## 5. Las reglas: **cuales se cumplen hoy**

| regla | dice | hoy |
|---|---|---|
| R-CENSO0 | el numero y su denominador, en la MISMA unidad y escrita | ✅ **desde hoy**. El metro decia `ciclos/op` midiendo ticks |
| R-TIME6 | un tick de TSC no es un ciclo de nucleo | ✅ **desde hoy** en los dos testigos |
| R-TIME7 | lo que cuesta una instruccion no es una constante | ✅ se dan los DOS sellos (130 y 84 ciclos), nunca su media |
| "un cero no es una medida barata" | un valor sin medir no recibe veredicto | ✅ **desde hoy en C**. Estaba solo en el juez de Rust, y el metal del 17-08 imprimio `dispatch [META] 0` |
| el reparto | no se imprime una resta contra algo no medido | ✅ **desde hoy**. Decia `en el stub 792` restando de un cero |
| ventana limpia | no se imprime dentro de una ventana de medida | ✅ los dos testigos leen sus contadores antes de imprimir |
| `PUERTA_PELADA` techo 960 / meta 300 | trinquete y deuda | ✅ **EN PLAZO**: 792. Deuda 492 ticks = **602 ciclos** |
| `HANDLE` techo 355 / meta 80 | idem | ⚠ **EN PLAZO** con la cifra de C (313), pero esa cifra mezcla. Con la sonda aislada (181) tambien cumple, y con mas margen |
| `DISPATCH` techo 110 / meta 60 | idem | ⛔ **NO SE PUEDE LEER**: el metro esta retirado. Hoy contesta `[ROTO] medida en cero`, que es la respuesta correcta |
| entry.rs, etiqueta `[fila]` | *"NINGUNA -- la pieza mas cara del arbol sin fila de presupuesto"* | ⚠ sigue abierta. Hoy la fila `PUERTA` hace de trinquete del stub **porque el resto no se mide**, y eso hay que decirlo en vez de dar la fila por cubierta |

---

## 5b. ★★ Y todo esto es de UNA maquina: el presupuesto tiene dueno

Las cifras de arriba son ticks del TSC de **esta** placa. El mismo kernel arranca
en cualquier x86-64, asi que la tabla de techos, tal como estaba --`const` del
kernel-- habria juzgado con ellos en cualquier otro CPU:

```
   un CPU mas lento    -> [SE PASA] REGRESION     por no ser el mismo silicio
   un CPU mas rapido   -> [META]                  aunque hubiera una regresion
```

Es el mismo fallo que este documento lleva persiguiendo entero: **opinar donde
no hay derecho**. Cerrado el 17-08 (R-CPU8), y la mitad ya estaba construida --
`cpu_vendor/profile.rs` dice desde su primera linea que *cambiar de CPU es
cambiar de perfil, nunca editar el kernel*; lo unico que faltaba era que el
presupuesto viviera alli.

```
   cpu_vendor/ryzen_5_5600x/presupuesto.rs   las tres filas + familia, modelo
                                             y TSC en que se midieron
   syscall/presupuesto.rs                    la FORMA y la doctrina: eso si es
                                             del kernel
   INFO_PRESUPUESTO_MAQUINA                  coincide? y, si no, los DOS lados
```

**Estrenar otro CPU es**: copiar el directorio del perfil con su
`presupuesto.rs`, arrancar --dira `SIN TRINQUETE`, que es lo correcto: todavia no
hay medida de esa maquina--, correr `sys/precio.bex` y pegar las tres cifras con
su +5%. **Ni una linea del kernel.**

★ Y el "estandar" que se busca aqui **no es el numero**: es la doctrina --techo y
meta, un cero no es una medida, la unidad con su denominador, doble testigo--,
que vive en `bmo-juicio`, se prueba en un `cargo test` de tres segundos y viaja a
cualquier CPU. El valor absoluto es calibracion, y la calibracion no se hereda.

[!] **Lo que NO resuelve, y hay que decirlo**: un ratio (*"la puerta no puede
costar mas de N veces un `rdtsc`"*) parece la solucion portable y no lo es --
R-TIME7 ya midio que el mismo `rdtsc` vale 107 ticks en un bucle apretado y 69 en
uno flojo. Un trinquete montado sobre una referencia que varia un 55% no es un
trinquete. El ratio sirve para orientarse al estrenar una maquina, no para
condenar.

---

## 5c. ★★ El traje que se cine solo: SUELO y SOBRECOSTE

Lo pidio el dueno el 17-08 con una imagen que es exactamente la buena: *"como el
traje de Spider-Man, que es grande y al pulsar el boton se ajusta"*. Y la
pregunta de debajo: **cuan optimizado esta el kernel**, que no deberia depender
de que CPU haya delante.

**Los 792 ticks no contestan eso**, porque son dos cosas pegadas:

```
   SUELO       cruzar el anillo en ESE silicio. Ni merito ni culpa de BMO.
   SOBRECOSTE  lo que BMO anade encima. Eso SI es este kernel.
```

Separados sale la cifra que viaja:

```
   [MEDIDO]    una puerta            792 ticks
   [ESTIMADO]  el suelo del cruce   ~150 ticks
   ---------------------------------------------
               BMO cuesta 5,3x el suelo del hardware      <- el norte
               la meta declarada (300)                       2,0x
```

Si BMO adelgaza, ese numero baja **en todas las maquinas a la vez** -- que es
optimizar *"a base de perfil"* en vez de *"a base de CPU"*.

### La trampa del boton, y la regla que la cierra

> ★★ **El suelo se MIDE. El multiplicador se ESCRIBE.** (R-CPU10)

Un presupuesto que se recalibrara solo entero **se ceniria tambien a la grasa**:
una regresion pasaria a ser la talla nueva y el juez aprobaria siempre. Un
trinquete que se ajusta solo no es un trinquete. Asi que se ajusta la parte que
es del CPU, y jamas el veredicto.

### Los tres estados del juez

| lo que hay | que hace |
|---|---|
| medida de ESTA maquina | usa el techo medido |
| solo el suelo, **medido** | deriva `suelo x multiplicador` -- **primera talla**, y lo dice (R-CPU11) |
| nada | sin trinquete |

### [!] Y hoy el suelo es una ESTIMACION, no una medida

`~150` sale del analisis de la fila `puerta`, no de un cronometro. Por eso
`INFO_SUELO_CRUCE` lleva un bit `medido` que hoy vale **0**: el ratio se puede
mirar, y **no puede derivar ningun techo**.

### ★★ Y se puede ACOTAR sin tocar el stub, con el metro que ya existe

La idea obvia --una puerta que el stub conteste sin bajar a Rust-- choca con dos
reglas de la casa: **las DOS puertas congeladas** y la ignorancia del stub, que
`entry.rs` prohibe romper por escrito. Pero no hace falta:

```
   [MEDIDO]   puerta - dispatch  =  TODO lo que no es Rust
   [CONTADO]  el stub son 58 instrucciones -> a IPC 1, <= 58 ticks
   ------------------------------------------------------------------
              el suelo esta entre (puerta - dispatch - 58) y (puerta - dispatch)
```

Eso sale de una tanda con `--features metro_puerta`, que **ya existe**. Los dos
testigos imprimen ahora esa cota.

★ **Y la prediccion, que es lo que hace que valga la pena correrla.** Con los
numeros del 16-08 --puerta 895, `dispatch` 104-- lo que no es Rust son **~780
ticks**:

```
   el perfil declara hoy    suelo ~150 ticks     <- estimacion
   la cota diria            suelo ~720-780       <- medido, 5x mas
```

Si sale asi, **la meta de 300 ticks para una puerta entera es fisicamente
imposible** --el suelo solo ya se la come-- y esa fila hay que reescribirla. Un
presupuesto con una meta inalcanzable no es exigente: es ruido, y ensena a
ignorar la fila.

[!] Y se dice *"lo que no es Rust"* y no *"el suelo"* a proposito: ahi dentro van
las dos transiciones --irreducibles-- **y el marco que BMO eligio construir** (la
reserva de 1096 B, el sello, los 20 `push`). Lo segundo se puede cambiar; lo
primero no. Llamarlo suelo a secas seria declarar irreducible una decision de
diseno.

---

## 6. Lo que este documento NO afirma

- **Que los 969 ciclos sean caros o baratos.** Sin el reparto de P1 no se puede
  decir cuanto es "el precio del hardware" y cuanto es codigo de esta casa.
- **Que el cerrojo de `cap.rs` sea el culpable de los 221 ciclos.** Es uno de
  tres sospechosos, ninguno medido por separado.
- **Que quitar el cerrojo sea correcto.** Es una pregunta abierta con una razon
  concreta en contra (`revoke` concurrente, y el dia que un AP entre en Ring 0).
- **Que el reparto de trafico de la seccion 4 este visto.** El instrumento se
  escribio el 17-08 y **ningun CPU lo ha ejecutado**: hasta la proxima tanda, la
  columna "veces por segundo" sigue vacia.
- **Que el modelo de CPU declarado en el presupuesto sea el correcto.** El arbol
  se contradice a si mismo (`19h/01h` en `cpu/mod.rs`, `19h/21h` en el perfil) y
  **nadie ha leido nunca ese byte en este chip**. Se toma el del perfil; lo
  desempata la proxima tanda, y si sale `SIN TRINQUETE` la propia linea trae el
  esperado y el leido.

---

*Ver `docs/CENSO_DE_EJES.md` para la aritmetica que tacha caminos enteros, y
`ring0/syscall/presupuesto.rs` para la tabla de techos y metas con el porque de
cada cifra.*
