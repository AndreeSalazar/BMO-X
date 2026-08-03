# AVANCES — estado de BMO-X (recuperación de contexto)

> Documento vivo para retomar el proyecto desde cero (chat nuevo). Resumen de
> QUÉ funciona, QUÉ falta, DÓNDE está cada cosa y CÓMO se trabaja. Para el
> detalle cronológico ver los commits y `BITACORA.md`.

**BMO-X** = sistema operativo bare-metal en Rust: microkernel de **capabilities**
con **superficie congelada de 3 syscalls** (`INVOKE`/`CHANNEL_KICK`/`WAIT`) +
subsyscalls; arranca en **hardware real** (MSI A320M PRO MAX + Ryzen 5 5600X),
sin QEMU. Toolchain propio (C / COBOL / **Ada** / C++ → BEF → BEX nativo), y los
tres primeros **ya han ejecutado en el Ryzen**.

> **Al 2026-08-02**: **620 tests en verde y CERO rojos**, BMO-X ocupa ~5.4 MiB de 14.8 GiB, y
> el objetivo declarado es **BANCA + Ada**. Lo que ese objetivo descarta (Wine,
> Vulkan, libc completa, ventanas con superficies) vale tanto como lo que exige:
> es lo que hace el proyecto **terminable**.

## ★★★★ 2026-08-02, QUINTA tanda — LA LISTA DE PENDIENTES SE VACIÓ

No queda **nada** escrito-sin-estrenar de las seis cosas que abrieron el día.
Todo con foto:

- **`ls` enseña**, con `-- historial --` al subir con RePág. El arreglo del
  escritor/lector (Ep. 26) confirmado.
- **★ EL FOCO ENTERO**: el conmutador de **Alt+Tab** sale con su ventanita
  (`> Ejecutar` / `Datos (ESTRATOS)`) y el modo escrito debajo
  (`modo: normal (Alt+M)`). 17 tests que llevaban desde el 2026-08-02 sin que
  nadie pulsara esas teclas — pulsadas y correctas.
- **★★ `KIND_MEMORIA`, VERIFICADA POR LOS DOS LADOS.** `info` dice
  **`a Ring 3   8.4 MiB   pedida con KIND_MEMORIA`**. Eso no lo dice el
  programa: lo dice el KERNEL, con el contador que no leía nadie
  (`INFO_MEM_ENTREGADA`). 8.4 MiB = el doble búfer del compositor.
- **★ Ada corriendo desde el escritorio**: `run ada/cierre.bex` →
  `CIERRE EN ADA - BANCO BMO`, `59.97`, `39.98`. Tercer lenguaje, lanzado desde
  Ring 3 y con su salida en la rejilla.
- **`info` entero**: 6 físicos / 12 hilos, TSC 3.70 GHz medido, 14.8 GiB,
  kernel 2.1 MiB, 2 ranuras de 64, disco listo, datos montado para escritura.
- **Las tres ventanas conviven**: Ejecutar + Datos + kernel, con Z-order y foco.

**Estado real del sistema**: el escritorio arranca, lanza los tres lenguajes,
lee el disco, enseña el almacén, deja leer el log de Ring 0, y responde al
teclado y al ratón. **Eso es un sistema operativo usable, no una demo.**

---

## ★★★ VERIFICADO EN EL RYZEN (2026-08-02, cuarta tanda) — la tanda grande

Cinco de las seis cosas que estaban escritas y sin estrenar quedan **cerradas
con foto**, y la sexta la destapó el instrumento nuevo.

- **★ F11 FUNCIONA.** La ventana `RING 0 // lo que dice el kernel` sale con
  `guardadas 61 de 61` y el arranque entero legible **desde el escritorio**. Es
  la primera vez que Ring 3 puede leer lo que dijo Ring 0.
- **★★ EL DOBLE BÚFER FUNCIONA, y lo dijo él mismo**: en esa ventana se lee
  `gui.bex> doble bufer: pintando fuera de la pantalla`. Como el búfer son
  **~8 MB contiguos pedidos con `KIND_MEMORIA`**, esa línea es también la
  **verificación en metal de la capability de memoria**: el kernel entregó el
  bloque, el compositor pinta dentro y sigue en pie.
- **★ F12 / ESTRATOS FUNCIONA**: generación 1, `96.00 KiB de 414.54 GiB`,
  estado holgado, **`identidad: nacio en ESTE disco`**, y `escritura: CERRADA`
  diciendo por qué. El gate de identidad del §5, en pantalla.
- **Las letras se dibujan.** El campo pinta lo que se teclea (`ls` en la foto).
- **El ratón**, confirmado otra vez, y la barra de pulso se llena al moverlo —
  que es lo que esa barra existe para decir.

### ⚠️ Y lo que el instrumento nuevo destapó: `ls` corría y no enseñaba nada

`ls` ejecutaba (la línea de estado decía `listo`) y la rejilla se quedaba en
blanco. **El escritor y el lector del buffer de salida miraban extremos
opuestos**: `Salida::nueva` empezaba a escribir en `fila = 0` y `pintar_salida`
enseña **las últimas 16 filas de 200** (`celdas[184..200]`).

O sea que **las 184 primeras líneas que escribiera cualquier programa eran
invisibles**. `ls` escupe una docena: no llegaba ni de lejos. Llegó con el
historial con scroll (`8ee091e2`), que movió la ventana del lector y dejó al
escritor donde estaba — correcto cuando la rejilla eran 16 filas y punto.
Arreglado escribiendo siempre en la última fila. Ep. 26 de `BITACORA.md`.

---

## ★★ VERIFICADO EN EL RYZEN (2026-08-02, tercera tanda) — con fotos

**El ratón FUNCIONA.** Es el hito de la sesión: el puntero se mueve donde se
mueve la mano, y los ejes ya no van cruzados. **El arreglo de leer el Report
Descriptor (`7ffc4955`) queda CONFIRMADO** — la pregunta de "8 o 16 bits" la
contestó el aparato, y acertó. Tres arranques y dos meses de episodios de USB se
cierran aquí (Ep. 17, 18, 19, 22, 24 de `BITACORA.md`).

También confirmado en las mismas fotos:

- **Arranca directo al escritorio** con el doble búfer desplegado, o sea que
  **el compositor no murió** pidiendo 8 MB. La caja `Ejecutar` se dibuja entera:
  marco, barra de título, línea de pista y campo.
- **El testigo de botones responde**: pulsar cualquier botón del ratón enciende
  el cuadro de 16×16 al final de la barra de pulso. Es lo que tiene que hacer.
- El log de Ring 0 llega hasta `[usb] mouse USB listo`, AHCI con
  `!p0x2 sig=0x101` (el Kingston SATA) y xHCI con `max_slots=0x40`.

### ★ Y por eso existe ahora **F11: la consola del KERNEL**

Lo que bloqueó el diagnóstico no fue la falta de una teoría: fue que **la línea
que decidía no se podía leer**. Desde que el escritorio es el arranque, el panel
del kernel deja de pintarse en cuanto el compositor reclama la pantalla, y con
él desaparecía el relato entero de cómo arrancó la máquina.

- **`ring0/core/klog.rs`**: el log del kernel se GUARDA en un anillo de 64
  líneas. Se guarda **antes** de los `return` que exigen framebuffer — que son
  razones para no pintar, no para no recordar.
- **`TASK_OP_KLOG_INFO` (0x16) y `TASK_OP_KLOG_TEXTO` (0x17)**, calcadas de
  `INFO`/`INFO_TEXTO`. **No dan privilegio, dan vista**: Ring 3 pide texto por
  su número y recibe bytes. En un sistema de capabilities *ver* y *poder* son
  cosas separadas, y juntarlas es como se acaba con un "modo administrador".
- **La ventana (F11)**, con color por emisor y RePág/AvPág para llegar al
  principio del arranque. Y F11 en vez de un comando por una razón de hoy:
  **no hace falta teclear nada para abrirla** — que es justo lo que falla.

### ⚠️ ABIERTO, y es lo siguiente: **no se dibujan las letras que se teclean**

El campo de la caja se queda vacío mientras se escribe. El resto de la ventana
—marco, título, la línea de pista, el cursor— **sí se dibuja**, así que no es
que el compositor esté muerto ni que el texto no sepa pintarse.

**Lo que hay que averiguar primero, y es UNA línea del log de arranque**:
`doble bufer: pintando fuera de la pantalla` o `SIN doble bufer`. Decide entre
dos culpables muy distintos, y hasta saberlo cualquier teoría es teoría (ley 9:
un aviso correcto no implica una teoría correcta).

**Y ahora esa línea se puede leer sin serie ni cámara: se pulsa F11.** Ésa es la
prueba de fuego de la ventana nueva — si al abrirla sale el arranque entero, el
instrumento funciona y el diagnóstico deja de depender de una foto.

**El discriminador de 30 segundos**, si el F11 tampoco dijera nada:
`git checkout 7f6d1085 -- Ultra_userspace/` deja el compositor **justo antes**
del doble búfer, con el arreglo del ghosting puesto. Si teclear vuelve a
pintar, el culpable es el doble búfer y está acotado a un commit.

---

## ★ Lo último que pasó (2026-08-02, segunda tanda) — leer esto primero

Tres frentes, y los tres eran "escrito y sin estrenar". Nada de esto ha tocado
un CPU todavía: es lo que hay que llevar al Ryzen en el arranque siguiente.

- **`KIND_MEMORIA` tiene por fin quien la llame**: `c/memc.bex`
  (`toolchain/lang/c/examples/memoria_C.c`) pide, escribe y relee 1024 bytes,
  marca las dieciséis páginas de un bloque de 64 KiB, y **agota el tope de
  cuatro peticiones** para ver que la quinta devuelve 0.
- **Y al escribirlo salió que el tope no se cumplía.** El `malloc` del codegen
  emitía sus saltos con desplazamientos contados a mano y el de la rama de
  fallo se quedaba **seis bytes corto**: cuando el kernel rechazaba, el CPU
  seguía a media instrucción. La rama buena estaba bien, que es lo que lo hacía
  invisible. Ahora van por etiqueta. (Ep. 23 de `BITACORA.md`.)
- **El emulador modela la capability de memoria.** Antes `TASK_OP_MEMORIA_PEDIR`
  caía en el `_ => {}` del despacho y salía por el epílogo de ÉXITO con el
  valor a cero: contestaba "toma tu bloque" y entregaba el puntero nulo.
- **El contador de memoria entregada se lee.** `total_entregado()` existía y
  **no lo consultaba nadie** (patrón 19). Ahora es `INFO_MEM_ENTREGADA` y sale
  en `info` como `a Ring 3`. Importa porque **la línea de CABINA `mem: bloque
  entregado a Ring 3` no se puede ver desde el escritorio**: mientras el
  compositor tiene la pantalla, el panel del kernel no se pinta.
- **El ratón ya no adivina su formato: lo lee.** Se le pide el Report
  Descriptor (`GET_DESCRIPTOR` tipo 0x22) y `bmo_uhid::formato` saca botones,
  X, Y y rueda con su bit y su ancho. La pregunta de "8 o 16 bits" la contesta
  el aparato, no una foto. Con reserva al formato BOOT si no se entiende, **y
  dicho**.
- **★★ DOBLE BÚFER**, y es el primer cliente de verdad de `KIND_MEMORIA`. El
  compositor pide `stride × alto × 4` (~8 MiB) y **dibuja en RAM normal**,
  volcando al panel una vez por fotograma y **sólo la caja de lo sucio** — que
  la regla de esta casa sigue siendo *repintar el daño, no la pantalla*. Mata
  el ghosting **por construcción** (nunca se lee memoria WC), mata el tearing,
  pintar pasa a ser en RAM cacheada, y es la pieza que hacía falta para las
  superficies. Si no hay bloque, se dibuja en el panel como siempre **y se
  dice**.
- **★ EL GHOSTING TENÍA CAUSA** (Ep. 25). El *save-under* del cursor es el
  **único** sitio que LEE el framebuffer en todo el compositor, y lo hacía
  justo antes del único `sfence` del fotograma: con write-combining, leer sin
  barrera devuelve la pantalla de **hace un fotograma**. Guardaba píxeles
  caducados y la vuelta siguiente los devolvía encima de lo nuevo — un
  rectángulo de 10×16 persiguiendo al puntero. Es el Ep. 20 por el lado del
  lector. Arreglado con un `sfence` dentro de `Bajo::poner`, más el pintado de
  la calculadora que se colaba con el cursor puesto.

---

## ★ Lo anterior (2026-08-02, primera tanda)

El día en que **el escritorio dejó de ser una demostración y pasó a ser el
arranque**. Verificado en el Ryzen con fotos:

- **Arranca limpio al escritorio**, sin panel del kernel encima. Los cinco
  programas de ejemplo ya **no se lanzan solos**: `init_hello` reclamaba la
  pantalla, moría, y el kernel repintaba su panel sobre el escritorio recién
  nacido. Eso costó tres arranques culpando al compositor de morirse — y el
  compositor **nunca estuvo muriéndose**. El kernel adelgazó 37 KB al irse.
- **Teclear pinta al momento.** Faltaba un `sfence`: con write-combining el CPU
  retiene los píxeles hasta que el búfer se llena, y mover el ratón era lo que
  lo llenaba. *"Tengo que apuntar bien para que me pinte las escrituras"* era
  eso. **WC sin barrera no es rápido: es incorrecto.**
- **Write-combining** por PAT (`MSR_PAT` llevaba declarado y **nunca se
  escribía**), y −320 ms de esperas de VBUS en el arranque.
- **El ratón lo confesó él mismo**: `protocolo=0x1 (INFORME: el aparato ignoró
  el BOOT)`. Su informe lleva Report ID, por eso iba corrido un byte y se movía
  al hacer clic. Falta decidir 8 vs 16 bits de desplazamiento — el driver ya
  registra ocho bytes crudos.

Y en el toolchain: **C completo para lo que DOOM pide** (32/32 sondas),
`static`, prototipos, varargs, arrays en agregados, `int a,b;`, y la libc
esencial en L1. Más `KIND_MEMORIA`, que **ningún programa ha llamado aún en
metal**.

---

## Cómo leer este documento

Hay **tres estados**, y confundirlos es lo que hace que uno se sienta perdido:

- ✅ **corre en metal** — se ha visto funcionar en el Ryzen, con foto o con
  línea de CABINA. Es lo único que cuenta como hecho.
- ✍️ **escrito sin estrenar** — compila, enlaza, `bex-link` verifica sus
  direcciones… y ningún CPU ha ejecutado una sola de sus instrucciones. **No es
  lo mismo que hecho.** Es exactamente la clase de cosa que en otros proyectos
  acaba existiendo sólo en la documentación.
- ⬜ **diseño** — pensado o escrito en un documento, sin código vivo.

---

## Estado global

| Componente | Estado |
|---|---|
| Boot chain (UEFI shim + s1_cpu + s2_mem) | ✅ arranca en HW real |
| Ring 0 (kernel: scheduler preemptivo, mm, caps, IPC) | ✅ estable en HW |
| Ring 3 (userspace) | ✅ varios procesos, cada uno con su espacio y sus caps |
| Fault isolation (crash R3 mata la tarea, no el kernel) | ✅ implementado |
| Boot cinemático (logo→RING0→RING3, escenas) | ✅ |
| Teclado USB (xHCI+HID) | ✅ **ESCRIBE en HW** — el Interval del endpoint era un EXPONENTE (2^n x125us) y se escribia el bInterval crudo: un teclado que pedia 24 ms quedaba programado a 35 minutos entre sondeos. Layouts es-latam/es-espana/us, teclas muertas, AltGr, Ctrl, repeticion al mantener, LEDs, historial |
| Mouse USB | ✅ **FUNCIONA EN METAL** (2026-08-02, con foto): el puntero va donde va la mano y los botones responden. El driver **lee su Report Descriptor** y saca bit y ancho de cada campo en vez de suponerlos |
| **CABINA** (telemetría omnisciente) | ✅ **viva**: cockpit + color semántico + bitácora de eventos (narrador) + detección de disco PCI |
| **`KIND_FRAMEBUFFER`** (la pantalla es una capability) | ✅ Ring 3 pinta con `mov`; el kernel contesta 4 preguntas y se aparta |
| **`KIND_INPUT`** (ratón, teclado **y modificadores**) | ✅ en metal; `Ctrl+Alt` detectado sin romper `AltGr` |
| **Compositor** (Ultra_userspace/services/gui) | ✅ **se carga de `sys/gui.bex`**, fuera del kernel (123 KiB; el tope son 256) |
| **Terminal de Ring 3** (caja Win+R + comandos) | ✅ **corre**: historial, TAB que completa, editor de línea con cursor, portapapeles, `ls`, `Ctrl+Alt` para invocar |
| **`KIND_CONSOLE`** (la salida es una capability, en LOS DOS sentidos) | ✅ el hijo escribe y el terminal lee; el terminal escribe y el hijo lee (`ACCEPT`) |
| **`KIND_DIRECTORIO`** (preguntar qué hay en el disco) | ✅ `ls` en el terminal, iteración sin cursor en el driver |
| **Calculadora con botones** | ✅ cara en Rust, cálculo en BMO COBOL |
| **`ring0/lanzar.rs`** (buscar+firma+admitir, un solo camino) | ✅ lo usa `run` en metal |
| **ESTRATOS** | ✅ montado, superbloque leído, **firma verificada antes de ejecutar** |
| Toolchain reorganizado (lang/forge/tools) | ✅ |
| sem-asm (encoder tabla→bytes + intrínsecos) | ✅ C lo usa; fusión sem-asm↔C hecha |
| BMO COBOL | ✅ **banca cerrada en su alcance**: PICTURE de edición en ejecución, File I/O secuencial, OCCURS con guarda de rango, nivel 88. `batch.bex` y `concep.bex` verificados en el Ryzen |
| **BMO C ("CONTROL ABSOLUTE")** | ✅ **32 de 32 sondas del lenguaje** — completo para lo que DOOM pide. 216 tests que EJECUTAN. `static`, prototipos, varargs, arrays en agregados, `int a,b;`. libc 11/15 |
| **BMO Ada** | ✅ **verificado en el Ryzen el 2026-07-30**, el mismo día que nació el compilador. Perfil ZFP + Annex F: Annex F copió el `PICTURE` de COBOL, así que el decimal ya estaba pagado |
| C++ frontend | ◐ ~900 líneas y **desborda la pila con una clase de dos métodos**. Alcance escrito en `lang/cpp/BRECHA.md` |
| **El FOCO del escritorio** (`bmo_input::foco`) | ✅ **EN METAL** (2026-08-02): Alt+Tab con su conmutador, pila MRU, `modo: normal (Alt+M)`, el foco arrastra el Z-order. 17 tests y la foto |
| **`KIND_MEMORIA`** (un proceso pide memoria) | ✅ **EN METAL, por los dos lados**: `info` dice `a Ring 3  8.4 MiB  pedida con KIND_MEMORIA` — lo dice el KERNEL. Su primer cliente es el doble búfer del compositor. Más `c/memc.bex` y 7 tests que EJECUTAN |
| **Write-combining del framebuffer** | ✅ PAT programado + `sfence` por fotograma. Sin la barrera, lo pintado se quedaba en el búfer |
| **c-gen** (la fábrica que mide el compilador) | ✅ sondas que COMPILAN, censo de 91 elementos de C (25 fuera) y 49 de C++ (17 fuera) |
| **Driver de disco (AHCI/SATA)** | ✅ **LEE Y MONTA**: GPT + FAT32 + volumen de datos con escritor. El NVMe de esta maquina es el disco de **Windows** — nunca se toca |
| **XSAVE per-task** | ✅ **resuelto y confirmado en metal** (ver abajo: la causa raíz) |

---

## Lo que corre en metal, verificado (arranque del 2026-07-27)

Esto no es una lista de intenciones — cada línea salió en pantalla o en CABINA:

- Arranque completo **sin pantalla azul**, shell vivo, 54 eventos en CABINA.
- `fs: volumen de datos montado para ESCRITURA` · `estratos: volumen montado y
  es de este disco` · superbloque generación 1.
- `sched: primer switch a CPL3` · `ring3: primer CONSOLE_WRITE` · cuatro
  procesos Ring 3 terminando **por su cuenta** (`EXIT`).
- `usb: primera tecla recibida: el teclado ESCRIBE`.
- **`run apps/COBOL.bex` desde ESTRATOS con la firma verificada** → `tid 7`.
  Y el programa imprimió `3 x 19.99 = 59.97 exacto`: **decimal exacto de COBOL,
  compilado por el toolchain propio, corriendo sobre el kernel propio, en un
  Ryzen de verdad.**
- La tabla `bex` con `asm`, `C`, `COBOL`, `srv`, `cli` y `COBOL.b` — y
  `leeme.t` marcado **RECHAZADO**: la admisión BEX rechaza lo que no es un
  programa en vez de saltar al vacío.

## Verificado en el Ryzen después (2026-07-30, con fotos)

La sesión que cerró dos días de trabajo:

- **`batch.bex`** — `BATCH DE CIERRE - BANCO BMO`, `total del dia: $1,135.00`,
  `cierre escrito en apps/cierre.txt`. **File I/O de COBOL en silicio**: leer un
  fichero, totalizar en decimal exacto, escribir el cierre y cerrarlo.
- **`concep.bex`** — `$105.00 / $25.50 / $60.00 / $0.00`: **OCCURS funciona**.
- **`extracto.bex`** — `$12,345.67`, `*****0.45` y `  120.00CR` alineados:
  **PICTURE de edición en ejecución**, la línea de un banco de punta a punta.
- **`cierre.bex` en ADA** — `CIERRE EN ADA - BANCO BMO`, `59.97`, `39.98`.
  **Tercer lenguaje en silicio real.**
- **El contador de programas**: `info` dijo *17 lanzados* con *ranuras 4 en uso
  de 64*. Antes moría al tercero — `has_room()` miraba una bitácora histórica
  de 8 entradas en vez de preguntarle al planificador.
- **`info` entero**: Zen 3 (Vermeer) 19h/21h, 6 físicos / 12 hilos, TSC medido
  3.70 GHz, **14.8 GiB totales y 5.4 MiB usados**, kernel 2.1 MiB.

## Lo que está escrito y NUNCA ha corrido

Honestidad primero: esto es lo que hay que estrenar antes de construir encima.
La lista completa, con **cómo se comprueba cada cosa**, vive en la memoria de
pendientes de hardware; aquí va el resumen.

- **El ratón, otra vez.** Enumera y da puntero y botones, pero el arreglo del
  **anillo de eventos compartido** (`BITACORA.md` Ep. 18) espera foto. Lo que
  hay que mirar: `apk=total:perdidos:ahora` con **perdidos en 0**, `kev=`
  subiendo al teclear y `raton ev=` subiendo al mover.
  Y ahora también **su formato**: en el arranque tiene que salir
  `[uhid] formato del raton: id=N x=bitN/Nb y=bitN/Nb informe=N bits`. Si sale
  `no entiendo su Report Descriptor`, el parser tiene un caso sin cubrir y los
  ocho bytes crudos del log dicen cuál.
- **`KIND_MEMORIA` en metal.** Y ahora hay **dos** pruebas, porque el doble
  búfer la ejerce en el arranque:
  1. **En el log de arranque**: `doble bufer: pintando fuera de la pantalla`.
     Si sale `SIN doble bufer: no hubo bloque, pinto directo al panel`, la
     capability falló al primer cliente de verdad y el motivo está en CABINA.
  2. `run c/memc.bex` desde la caja: nueve líneas, la primera dirección
     `0xe0000000`, y acaba en `MEMORIA: las cuatro pruebas pasan`.
  3. `info`, fila **`a Ring 3`**: nada más arrancar tiene que marcar **≈8 MiB**
     (el búfer del compositor, `stride × alto × 4`), y **≈76 KiB más** después
     de `memc.bex`. Ese número lo da el KERNEL, no el programa — es la
     confirmación desde el otro lado.
- **El escritorio con foco** (`d29ad7c6`, `9d3f4943`, `345acfc5`): F12 abre la
  consola de datos de ESTRATOS, **Alt+Tab** recorre la MRU con su ventanita,
  **Alt+M** rota el modo, el clic da el teclado y **el foco arrastra el
  Z-order**. Y el cursor del ratón ya no agujerea las ventanas (*save-under*).
- **La escritura de ESTRATOS**: la transacción está escrita y probada (12
  tests) y **nadie la ha cableado al dispositivo**. La ventana de datos lo
  dice en rojo — si algún día aparece en verde sin cablearla, eso es el bug.
- **La calculadora con botones**: el motor `cobol/calcgui.bex` compila y el
  panel dibuja, pero nadie ha pulsado `=` en metal.

Lo que SÍ se estrenó: el terminal dibujando, la fuente en Ring 3, `tecla()`,
`OP_EJECUTAR`, el compositor desde disco, `KIND_CONSOLE`, `ACCEPT` de COBOL con
un importe tecleado, y los tres lenguajes.

---

## Kernel (Ultra_kernel_x86-64/)

Funciona en HW real: boot chain unificado (BOOTX64.EFI embebe s1/s2/kernel),
GDT/IDT propias, paginación (physmap 16 GiB, kernel-half pre-poblado),
scheduler preemptivo por LAPIC timer, Capability Engine, BMO Channel (IPC),
3 syscalls, fault isolation. **Bugs raíz históricos resueltos** (ver BITACORA):
CS fantasma UEFI, split-brain de gs, framebuffer bajo CR3 usuario, stacks no
contiguos.

**Teclado USB — RESUELTO.** El `Interval` del Endpoint Context de xHCI es un
**exponente** (2^n × 125 µs) y se escribía el `bInterval` crudo del descriptor,
que en Low/Full Speed viene en **milisegundos**: un teclado que pedía 24 ms
quedaba programado a **35 minutos** entre sondeos. Hoy `usb: primera tecla
recibida` sale en CABINA en cada arranque. El debug vive en la fila `usb`
(`kev/tev/hev/dci/lev`).

**XSAVE — la causa raíz (2026-07-27, cinco sondas y cuatro pantallas azules).**
`XSAVE` **no inicializa la cabecera XSAVE: hace MERGE.**

```text
XSTATE_BV <- (XSTATE_BV_viejo AND NOT RFBM) OR (XINUSE AND RFBM)
```

con `RFBM = EDX:EAX AND XCR0`, y **no toca** los 48 bytes reservados. Los stubs
tallan su área sobre la pila (`sub`+`and`), o sea sobre basura, y esa basura
sobrevivía al guardado en los bits altos → `XRSTOR` la rechaza con `#GP(0)`.
`trap::fabricate` nunca lo sufrió porque pone a cero los 1024 bytes antes de
nada; los stubs no. **Ésa era la asimetría.** Arreglo: los prólogos ponen a cero
la cabecera **entera** (512..575) antes del `xsave64`.

*La firma que lo delató*: los volcados daban `0x5F0FCB` y `0x37B`, y los dos son
**el valor viejo con los tres bits bajos puestos a 3** — que es exactamente
`XINUSE & 7`. Un campo corrupto con unos pocos bits bajos coherentes no es
corrupción: es una instrucción haciendo merge donde creíamos store.

*Defensas que quedan puestas*: guardia de cabecera en los cinco epílogos
(motivo `PODRIDO_CABECERA`), anillo de las últimas áreas publicadas
(`pub0..pub3`) con su tid, y las sondas `bv0`/`bvX`/`baseX`. El informe de fallo
es el único depurador que hay en esta máquina — por eso se quedan.

**CABINA (ring0/cabina.rs)** — telemetría omnisciente, always-on desde el shell
loop (NO desde el timer IRQ: causaba cuelgue→reset). Da vida a `cabina-core`:
`snapshot()` desde contadores vivos + `render_hud()` pinta bitácora de 9 líneas
(eventos con severidad/capa/color) + 3 de telemetría compacta. `record()`/
`info/warn/fault` = el narrador; ring de 48 eventos. `find_storage()` en dev/pci
detecta el controlador de disco (NVMe/AHCI). Color: verde=bien, ámbar=atención,
rojo=problema. Anti-ghosting por change-detection + SCREEN_GEN.

**Pendiente kernel**: capability de **memoria** — un proceso recibe su imagen
y 64 KiB de pila y no puede pedir más. Bloquea DOS cosas a la vez: cualquier
lenguaje con GC, y las **superficies compartidas** que hacen falta para
ventanas de verdad (hoy `KIND_FRAMEBUFFER` es exclusivo, un solo proceso es
dueño de la pantalla). Después: CABINA caja negra en disco, demand paging,
endpoint RPC (servidores Ring 3), EXIT-reclaim, SMP.

**Hecho desde entonces**: `KIND_DIRECTORIO` (hay `ls`), modificadores en
`INPUT_OP_MODIFICADORES` (hay `Ctrl+Alt`), `KIND_CONSOLE` en los dos sentidos
(hay `ACCEPT`).

**Deuda visible**: `services/input` es una carpeta que promete un multiplexor de
entrada y está vacía — la entrada la reclama el compositor directamente. O se
cablea o se borra, como se borró `apps/terminal`. Y el **manifest BEF**
(`provides`/`requires`, en `platform/abi/bmo-abi/src/bef/manifest.rs`) tiene
struct y parser TOML completos, y **el kernel no compila `bmo-abi`**: `build.ps1`
lo lee como TEXTO para el drift guard y nada más. Es el prerequisito si algún día
se quiere clasificar programas por lo que le PIDEN al kernel (AOT / GC / GIL).

---

## Toolchain (toolchain/)

```
toolchain/
  lang/    frontends (esencia): c, cobol, cpp, base(stdlib)
  forge/   pipeline compartido: sem-asm(encoder ✅), bmo-verify(gate ✅)
  tools/   generadores: bef-bootstrap, hello-bex, fontgen, bmo-linker, cobol-gen(Python)
```

### ★ El emulador, y hasta dónde llega (auditado 2026-08-02)

`bmo-lower::emu` es lo que hace que los tests del toolchain **ejecuten** en vez
de mirar bytes, y es la razón de que 574 pruebas signifiquen algo. Pero su
cobertura **no está repartida — está concentrada**, y confundir eso es cómo se
acumulan cosas verdes que nunca han corrido. El detalle entero vive en la
cabecera de `toolchain/forge/bmo-lower/src/emu.rs`, sección **FIDELIDAD**; el
resumen:

| Eje | Cobertura | Por qué |
|---|---|---|
| ¿los bytes calculan lo que dice la fuente? | **alto** | es para lo que se construyó; cazó el salto corto de `malloc` |
| ¿el kernel hace lo que el modelo dice? | **cero** | **no ejecuta el kernel: lo imita**. Si los dos se separan, los dos parecen sanos |
| lo físico (paginación, anillos, XSAVE, IRQs, DMA, WC, USB, tiempos) | **cero** | por construcción. Los 24 episodios de `BITACORA.md` son de aquí |

**Los agujeros con nombre**: no hay SSE (y por eso los 9 tests de float no
ejecutan ninguno), la memoria es un mapa disperso (toda dirección funciona: sin
fallos de página ni aliasing), no hay tope de pila (el proceso real tiene 64
KiB), y **no hay cargador** — el banco rearma las secciones a mano, así que el
cargador del kernel y la admisión de `bmo-verify` no se ejercen.

**La regla de reparto**: lo que se puede equivocar en la aritmética o en el
flujo, en el emulador; lo que depende del silicio o del kernel, en el Ryzen, y
**con su número escrito antes de arrancar**. El valor del emulador no es un
porcentaje: es el coste por bug — segundos aquí, contra flashear + reiniciar +
fotografiar + una teoría que puede estar mal.

- **sem-asm** ✅: motor que lee `forge/sem-asm/tables/*.toml` y encodea
  instrucciones→bytes. C y COBOL migrados a usarlo (fuera bytes hardcodeados).
- **bmo-verify**: gate que valida el BEF (delega en `bmo-abi::bef::validator`,
  el validador real de 15 tests). `bmo-lower` (descenso ABI) y `bmo-opt`
  (optimización) se recrearán con código real al empezar su fase — no stubs.

---

## BMO C — "CONTROL ABSOLUTE" (toolchain/lang/c/) — MUY completo

C esencial de Ritchie (~C11). **85 tests verdes.** Módulos: `standard.rs`
(versiones C89..C23, tablas en forge/sem-asm), `lexer.rs`, `parser/mod.rs`,
`ast/`, `codegen.rs` (el "diccionario" → bytes exactos, sin cerebro intermedio
tipo LLVM), `module.rs`.

**Fases HECHAS (2026-07-23/24):**
- **F0 — cimientos honestos**: exterminados ~10 "silencios traicioneros" (bytes
  MAL sin avisar): offsets `a->b->c` anidados, `int **pp`, sufijos `10UL`,
  `arr[i]=x` que se descartaba, `TypeSpec::Array(elem,n)` con tamaño real,
  decls anidadas sin slot (for infinito), subscript array-vs-puntero, stores de
  campo con tamaño exacto (`pt.x` ya no pisa `pt.y`), casts reales (movsx/movzx),
  errores con LÍNEA real. Criterio: "un diccionario no adivina".
- **F1 — LA FUSIÓN sem-asm↔C**: `tables/arch/x86_64/intrinsics.toml` +
  `__hlt/__pause/__rdtsc/__outb/__inb/__wrmsr/__cpuid`. El compilador emite los
  BYTES EXACTOS de la tabla (no caja negra tipo `asm()`); agregar instrucción =
  1 entrada TOML, cero Rust.
- **F2 — completo**: punteros a función (`int (*op)(int)`, decadencia, call rax
  indirecto = base de vtables C++), subscript compuesto (`p->arr[i]` = IndexPtr),
  `(*fp)(args)` (CallPtr), **floats SSE** (ruta xmm paralela: literales, +−×÷,
  comparaciones comisd, cvtsi2sd/cvttsd2si, retorno en xmm0; float globales y
  args-de-función = deferido honesto).
  ✅ **Y desde el 2026-08-02 la ruta SSE EJECUTA**: el emulador modela las
  quince instrucciones escalares que emite BMO C, y hay 7 tests que corren de
  verdad. Antes: de los **9 tests de coma flotante, 0 EJECUTABAN** — los nueve comparan ventanas de bytes
  (`bef.windows(3).any(...)`), que es el método que el propio emulador declara
  insuficiente en su cabecera. El emulador **no tiene SSE**, así que esa ruta
  entera compila, da verde y **ningún CPU la ha ejecutado**. Es la misma forma
  que tenía el bug de `malloc` (Ep. 23). Lo que lo arregla es meter `xmm` al
  emulador, no escribir más tests de bytes.

**FALTA C** (por orden de lo que más duele):

1. **ENTRADA. No puede leer NADA** — ni `scanf` ni `getchar`. Tiene `printf` y
   106 tests verdes, o sea que habla y no escucha. Es exactamente el hueco que
   COBOL tenía hasta el 2026-07-28, y ahora es barato: `console::read_line` y
   `fmt::parse_decimal_scaled` ya existen en `bmo-lower` y **no son de ningún
   lenguaje** — se comparten igual que el conversor de enteros.
2. `printf %f` y float args por ABI xmm; float globales.
3. Preprocesador completo.
4. **stdlib (`impl.c`)** — y ésta es la de verdad: *la universalidad de C no
   viene del lenguaje, viene de libc*. Sin biblioteca estándar, C es un
   ensamblador portable con llaves. Es lo que `bmo-rt` tiene que llegar a ser.

Base sólida para C++ (hereda lexer/tablas/intrínsecos/codegen; solo pone RAII
+ vtables encima).

---

## BMO COBOL (toolchain/lang/cobol/)

Ver `ARCHITECTURE.md` y `cobol.md` en esa carpeta.

> **Aquí no se pone un porcentaje, y es a propósito.** "COBOL al 15%" da a
> entender que existe un 100% — un denominador. No existe: el estándar sigue
> creciendo y ningún compilador del mundo lo implementa entero. Medirse contra
> un infinito no informa de nada y sólo sirve para sentirse pequeño. Lo que sí
> se puede afirmar y comprobar es **qué corre**, y cada línea de abajo tiene su
> fila en la matriz de conformidad, que EJECUTA lo que dice soportar.

**CORRE** (verificado ejecutando, no leyendo bytes):
- **Lexer** (`lexer.rs`): Source→Tokens; `.` decimal vs terminador; usa tablas.
- **Parser de tokens** (`tparser.rs`): sentencias + DATA DIVISION + programa
  completo → AST. Camino paralelo al `parser.rs` por-líneas (aún el principal).
- **PIC propio** (`pic.rs`): 100% BMO, sin gnucobol-rs (GPL). Da la escala.
- **Decimal EXACTO** (`codegen.rs`): ADD/SUB/MUL/DIV escalan por el PIC →
  centavos sin float. **El alma bancaria de Grace Hopper.** Confirmado en el
  Ryzen: `3 × 19.99 = 59.97`.
- **Flujo de control real**: IF/ELSE anidado y con AND, PERFORM TIMES,
  PERFORM UNTIL, COMPUTE con precedencia y paréntesis.
- **DISPLAY** de literal y de variable, **ACCEPT** por el anillo de entrada
  de la consola.
- **PICTURE de edición EN EJECUCIÓN** (`edicion.rs`): `$$$,$$9.99`,
  `**,**9.99`, `Z,ZZ9.99CR`, `DB`, signos fijos y flotantes, `99/99/99`.
  El recorrido de la plantilla se emite como INSTRUCCIONES: en el `.bex` no
  queda ni la máscara ni un intérprete que la lea. Atado a `formatear` por
  238 casos ejecutados en el emulador. Ver `examples/extracto.cob`.
- **Fábrica Python** (`tools/cobol-gen/`): genera `generated/words.rs` (556
  reservadas separadas ESENCIA vs VENDOR, 55 intrínsecas). Organizada en
  `defs/{words,verbs,intrinsics,grammar}.py`.
- Pipeline end-to-end probado: Source→lexer→tparser→AST→codegen→BEF (magic BEF1).
- **71 tests verdes.**

**NO CORRE** (y se dice, en vez de fingirlo):
- **File I/O** (`SELECT`/`FD`/`OPEN`/`READ`/`WRITE`/`CLOSE`) — se RECHAZA con
  su motivo en vez de compilar un READ que no lee. **El siguiente grande**: sin
  ficheros no hay batch, y debajo ya están el disco, FAT32 y el gate.
- DATA: records anidados (grupos 01/05/10), OCCURS, REDEFINES, nivel 88/66,
  COMP-3 real.
- Verbos: EVALUATE, PERFORM VARYING, STRING/UNSTRING, INSPECT, SEARCH, CALL,
  SORT.
- Subíndices, 55 intrínsecas (0 implementadas), runtime (bmo-rt), COPY,
  formato fijo/libre.
- Cablear `tparser::parse_program` como principal (jubilar `parser.rs`).

**Regla de la esencia**: "el encoder puede ser compartido; la aritmética de
COBOL jamás. El decimal es sagrado, vive solo en lang/cobol." GnuCOBOL infla a
1130+ palabras porque **traduce a C**; BMO compila **nativo** y separa esencia
de vendor. **COBOL devorado → BMO COBOL.**

---

## Filosofía / arquitectura (los principios)

1. **3 syscalls congelados + subsyscalls**: `INVOKE`/`KICK`/`WAIT` nunca
   cambian; todo lo demás son operaciones sobre capabilities (modelo seL4/Zircon,
   no Windows). Ver README raíz "Subsyscalls".
2. **Contratos y librerías, NUNCA cerebros**: se comparten formatos (BEF, ABI)
   y librerías opcionales; jamás un IR/embudo central (sería monolito).
3. **Library OS + Devour_System**: superficies ajenas (Win32, POSIX) se
   traducen a subsyscalls → nativo. El kernel no sabe que existieron.
4. **Borrar costos, no optimizarlos**: library OS borra la frontera de syscall;
   lenguajes nativos borran el impuesto del C ABI; perfil per-CPU borra el
   impuesto genérico.
5. **Python = fábrica de tablas** (dev-time), nunca entra a BMO. Genera lo
   TABULAR (~40%); la semántica/codegen es Rust (~60%).

---

## Flujo de trabajo

**Compilar + desplegar a hardware (Ring 0 Y los programas, de una vez):**
```bash
cd C:\Users\Salazar\Documents\BMO\Ultra_kernel_x86-64
.\build.ps1 -Flash -Drive A -Data A -Yes
bcdedit /set "{fwbootmgr}" bootsequence "{57cb1744-7f84-11f1-930d-c3a2d7ca848a}"
shutdown /r /t 5
```
En esta máquina el volumen de arranque y el de programas son **el mismo** (A:,
la partición 2 del Kingston SATA), así que las dos banderas llevan la misma
letra — pero siguen siendo dos banderas, porque son dos riesgos.
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
Lo último son **620 verdes y CERO rojos**.

★ **`boot-context` con GUION.** Estaba escrito `boot_context` con guion bajo, que
no es el nombre de ningún paquete — cargo se lo tragaba en silencio y ese crate
llevaba entrando en la suite todo el tiempo. Una exclusión que no excluye nada
es peor que ninguna: hace creer que algo está apartado cuando no lo está. Las cuatro exclusiones no son cosmética: el
kernel y `boot_context` son `no_std` y `cargo test` les mete `std` encima
(`duplicate lang item panic_impl`); `byte-defender` y `bmo-rt` están rotos
desde hace tiempo y son parte de la deuda técnica anotada.

**Copiar los programas de Ring 3 al volumen de datos:**
```bash
cd Ultra_kernel_x86-64; .\build.ps1 -Data A
```
El `.bex` del compositor sale a `staging\BMO-DATA\sys\gui.bex` en cada build y
de ahí se copia. `RUTA_COMPOSITOR` en `phase.rs` es `sys/gui.bex` (8.3: el
driver FAT32 no lee nombres largos y no recorta) — la ruta de dentro del
volumen es el contrato entre el build y el arranque, y el resto va por
categorías: `cobol/ c/ ada/ datos/`. El mapa completo, en
`Ultra_kernel_x86-64/VOLUMEN.md`.
Tres cierres antes de escribir un byte: **nunca el disco del sistema**, tiene que
ser FAT/FAT32, y hay que teclear `DATA <letra> BMO`. Es el ÚNICO sitio del build
que escribe fuera del árbol del proyecto. `-Flash` es aparte y es para Ring 0:
las dos banderas tocan discos distintos a propósito.

**Compilar solo el kernel (sin flashear) para verificar cambios:**
```bash
cd Ultra_kernel_x86-64; .\build.ps1 -BuildOnly
```
(El kernel es bare-metal; `cargo build --workspace` falla al linkear con
link.exe del host — usar build.ps1. Nota commits: mensajes con `->`/comillas/
paréntesis rompen el heredoc de PowerShell — usar `git commit -F archivo`.)

---

## Docs de referencia

- `BITACORA.md` — bitácora de guerra del debugging en HW (11 episodios).
- `README.md` (raíz) — arquitectura, Subsyscalls, boot path.
- `toolchain/lang/cobol/ARCHITECTURE.md` — pipeline COBOL completo + roadmap.
- `toolchain/lang/cobol/cobol.md` — esencia/teoría de COBOL en BMO.
- `toolchain/forge/README.md` + `toolchain/README.md` — pipeline y estructura.
- `toolchain/tools/cobol-gen/README.md` — la fábrica Python.
- `platform/abi/bmo-abi/src/ENDPOINT_RPC.md` — diseño RPC a Ring 3.

---

## Próximos frentes (prioridad)

**HECHO desde el 2026-07-25** (estaban aquí y ya no): FAT32 + volumen de datos
montado, gate de identidad antes de escribir, XSAVE per-task (y su causa raíz),
`.bex` fuera del kernel (el compositor se carga de disco), ESTRATOS montado con
gate de firma.

**HECHO desde entonces** (2026-07-28): la caja estrenada, el terminal con
comandos e historial, modificadores (`Ctrl+Alt`), `KIND_DIRECTORIO` (`ls`),
`KIND_CONSOLE` en los dos sentidos, `DISPLAY <var>` y `ACCEPT` en COBOL, y la
calculadora.

**HECHO desde entonces** (2026-07-29/31), y con eso **COBOL para banca queda
cerrado en su alcance declarado**: PICTURE de edición en ejecución, File I/O
secuencial, OCCURS con guarda de rango, nivel 88, entrada en BMO C
(`getchar`/`scanf`), **Ada verificada en el Ryzen**, el volumen de datos por
categorías, `info`/`cpu`/`mem` desde Ring 3, el historial con scroll, y el
escritorio con foco (F12, Alt+Tab, Alt+M). Lo que le queda a COBOL —`EVALUATE`,
`STRING`, `SEARCH`, `CALL`, `SORT`— es **cola larga del estándar, no banca**.
**COMP-3 ya no está en esa lista: entró el 2026-08-03** y guarda nibbles de
verdad. Lo que sí sigue siendo banca y falta son los **registros binarios** y el
**índice por clave**; ver `toolchain/lang/cobol/BANCA_REAL.md`.

**Kernel/HW (orden vigente 2026-08-02):**

**Antes que nada: EL ARRANQUE PENDIENTE.** Hay tres cosas escritas y sin
estrenar, y cada una tiene su prueba exacta arriba. Cuanto más crezca la pila
sin verificar, más difícil es saber cuál de las tres rompió algo si falla:
el foco entero (F12, Alt+Tab, Alt+M, clic), `run c/memc.bex` + `info`, y el
formato del ratón en el log de arranque.

1. ~~**Capability de MEMORIA**~~ — **HECHA** (`a9ccd4f8`), con su programa y
   su contador en `info`. Falta la foto.
2. **Cablear la escritura de ESTRATOS al dispositivo.** La transacción existe y
   está probada (12 tests); faltan el `write` y el `FLUSH CACHE` de verdad. Es
   lo único que separa "un almacén que se lee" de un almacén. **Es el frente
   grande que queda.**
3. ~~**Write-combining del framebuffer**~~ — **HECHO y verificado** (`952681c7`
   + el `sfence` de `3409ea8e`).
4. **Ada hacia ACATS** — el estándar trae su propio banco de conformidad, que
   es la forma honesta de medir cuánto Ada hay de verdad.
5. **Superficies y ventanas** — hoy `KIND_FRAMEBUFFER` es exclusivo. Wayland
   en pequeño, y ahora **ya tiene debajo lo que le faltaba**: la memoria
   compartida entre procesos se pide con `KIND_MEMORIA`. Es lo que saca la
   calculadora del
   compositor a su propia ventana **sin tocar el COBOL**. La política de foco
   ya está escrita y probada, así que ese día no hay que inventarla.
6. **Endpoint RPC → servicios Ring 3**: el momento library-OS.
7. **SMP al final**: el codigo de despertar los APs YA EXISTE en s1_cpu
   (trampolin, INIT+SIPI, GDT/IDT), pero `smp_startup()` no tiene ni una
   llamada y `ap_entry64` solo cuenta y hace hlt. Va el ultimo a proposito: el
   dia que corra un 2o nucleo, cada `static mut` del kernel es una carrera.

**Palancas de velocidad ARQUITECTONICAS (no micro-optimizacion):** sin cruce de
anillos (library OS), DMA directo al buffer del llamante (hoy hay pagina de
rebote), NCQ (el HBA declara 32 ranuras, se usa 1) e interrupciones MSI en vez
de sondeo.

**Sistemas de ficheros ajenos:** leer NTFS es viable HOY — el crate `ntfs` de
ColinFinck es no_std, MIT/Apache y esta pensado para firmware y drivers de
kernel. Escribirlo no: no hay nada seguro que enlazar. La decision es del dueño,
no una imposibilidad tecnica.

**Filosofía política grabada (2026-07-24)**: BMO-X = "dictadura absoluta pero
benevolente" — cero-confianza en el CÓDIGO (capabilities + bmo-verify), soberanía
del DUEÑO, transparencia total (CABINA lo confiesa todo). Trade-off honesto:
software que exige opacidad (DRM/anti-cheat de kernel) se auto-excluye. No es
piratería; es "esta máquina me obedece solo a mí". Consola-con-esteroides + PC.

**Lenguajes:**
0. **SSE en el emulador** — y va delante de C++ a propósito, porque es barato y
   tapa un agujero que YA existe en vez de abrir uno nuevo: hoy la ruta de coma
   flotante de BMO C tiene 9 tests y **ninguno la ejecuta**. Además C++ hereda
   esa ruta entera, así que construir encima sin ejecutarla es apilar sobre algo
   que nadie ha visto funcionar.
5. **BMO C++ (esencial, ACOTADO)** — SIGUIENTE lenguaje; barato encima de C
   (hereda todo). NO es "todo C++". Alcance deliberado =
   desde Bjarne (origen) hasta lo ESENCIAL de C++17, sin la bola moderna.
   - DENTRO: clases/structs, ctor/dtor (RAII), referencias, sobrecarga,
     herencia + virtuales (vtables, ya presente), namespaces, templates
     básicos, new/delete, auto, range-for, nullptr, constexpr básico, lambdas.
   - FUERA (la "basura" que hunde el barco, cf. Stroustrup "Remember the
     Vasa!"): concepts, coroutines, modules, ranges, STL gigante,
     metaprogramación pesada, C++20/23, el treadmill moderno.
   - Los 3 syscalls + runtime mínimo (bmo-rt) lo hacen FINITO/terminable:
     no necesita std::thread/filesystem/etc. **C++ congelado en su esencia.**

**Desktop (F5)**: compositor sobre Endpoint RPC, estética Win11+Mac cyberpunk.
