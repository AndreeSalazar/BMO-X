# Bitácora de guerra — BMO-X en hardware real

Episodios de debugging en metal desnudo (MSI A320M PRO MAX + Ryzen 5 5600X),
sin debugger, sin serial conectado: **solo fotos de la pantalla**. Cada
episodio: el síntoma, el culpable, y la moraleja que quedó grabada en el
código.

---

## Ep. 1 — El firmware que no quería soltar sus archivos
**Síntoma**: "no FAT filesystem found" — la MSI arranca con lector FAT
interno y jamás conecta drivers SimpleFS.
**Culpable**: fast-boot de fábrica sin opción visible.
**Moraleja**: no le pidas archivos al firmware — **embébelo todo** en un solo
BOOTX64.EFI (shim unificado con las etapas y el kernel adentro). Cero
dependencias, cero mercedes.

## Ep. 2 — El triple fault que solo pasaba en hardware
**Síntoma**: bootea en QEMU, reset instantáneo en la placa real.
**Culpable**: el firmware entrega con interrupciones ENCENDIDAS; un IRQ en
plena cirugía de GDT despacha con tablas inconsistentes.
**Moraleja**: `cli` + enmascarar el PIC ANTES de tocar la GDT. QEMU es un
mundo sin ruido; el hardware real tiene tráfico.

## Ep. 3 — Los GUIDs mal copiados (o: por qué nunca hubo framebuffer)
**Síntoma**: meses creyendo que la placa "no tenía GOP".
**Culpable**: los GUID de GOP y SimpleFS estaban mal escritos (data4
corrupto). El proyecto siempre corrió por serial y nadie lo notó.
**Moraleja**: un GUID es una contraseña de 16 bytes: o es EXACTA o el
universo responde "no existe".

## Ep. 4 — El CS fantasma de UEFI (la saga del #GP, capa 1)
**Síntoma**: #GP(0) eterno en el iretq del timer; frame fabricado PERFECTO,
GDT PERFECTA, CR3 compartida PERFECTA. Semanas de misterio.
**Culpable**: `init_gdt` hacía `lgdt` + recargaba los segmentos de datos...
**pero nunca el CS**. El CPU ejecutó Ring 0 entero con el descriptor UEFI
(cs=0x38) cacheado en el shadow register. Todo funcionaba — hasta que un
iretq re-validó ese selector contra NUESTRA GDT (entrada 7: vacía).
**Moraleja**: `lgdt` no recarga CS. El far-return (`push CS; push RIP;
retfq`) no es opcional — es el bautizo real del kernel.

## Ep. 5 — El split-brain de gs (la saga del #GP, capa 2)
**Síntoma**: el contexto se publicaba y el epílogo leía CEROS.
**Culpable**: el asm escribía por `gs:[0x10]` (MSR GS_BASE) y el Rust leía
el static directo. Dos caminos a "la misma" memoria que solo coinciden si
GS_BASE apunta donde crees — y el CS fantasma (Ep. 4) disparaba swapgs
espurios que lo movían.
**Moraleja**: para datos per-CPU, **un solo camino de acceso**. Escritor y
lector deben concordar POR CONSTRUCCIÓN, no por fe.

## Ep. 6 — El framebuffer invisible (la saga del #GP, capa 3)
**Síntoma**: con las capas 1 y 2 arregladas… congelamiento TOTAL sin
pantalla ni fault. El hola mundo Ring 3 SÍ ejecutaba — moría *pintando*.
**Culpable**: el address space de usuario comparte identidad solo 0..1 GiB;
el fb GOP vive en ~3.5 GiB. El flush de consola pintaba bajo la CR3 del
usuario → #PF → el reporter de faults TAMBIÉN pinta → #PF recursivo
infinito en IST1.
**Moraleja**: pregunta siempre **bajo qué CR3 corres** antes de tocar MMIO.
Y un fault handler jamás debe poder causar su propio fault.

## Ep. 7 — El teclado que funcionaba de prestado
**Síntoma**: en BMO/FastOS v0.6–0.9 el teclado escribía; en el BMO-X real,
silencio (solo ruido 0xFE del i8042, ni el LED de Bloq Mayús responde).
**Culpable**: antes los Boot Services estaban vivos y el firmware hacía el
USB por nosotros (emulación SMM USB→PS/2). Al convertirnos en un OS de
verdad (ExitBootServices), el firmware se llevó su magia.
**Moraleja**: la soberanía se paga con drivers. Lo que el firmware te
"regala" es un préstamo con fecha de vencimiento.

## Ep. 8 — El xHC escondido detrás del bridge
**Síntoma**: "[usb] no se encontro controlador xHCI" — con el controlador
ahí, funcionando.
**Culpable**: el scan PCI del boot era plano (bus 0); en Ryzen los xHC
cuelgan de buses detrás de bridges.
**Moraleja**: en PCI, si no recorres TODOS los buses, no has buscado. Y sin
habilitar Bus Master (BME), un controlador DMA es un adorno.

## Ep. 9 — "Nel, llegas tarde" (el CPU impaciente)
**Síntoma**: xHC inicializado perfecto (127 slots, 22 puertos)… y cero
dispositivos en los puertos. El teclado SEISA conectado, ignorado.
**Culpable**: el spec USB exige ~100 ms de debounce para detectar conexión.
El driver (criado en QEMU, donde todo es instantáneo) esperaba
~microsegundos. Para un Zen 3 a 4.6 GHz, 100 ms son una era geológica — y
no estaba dispuesto a esperarla.
**Moraleja**: el hardware real tiene TIEMPOS FÍSICOS. La paciencia no es
una virtud del CPU: hay que programársela (delays por TSC, no spin-counts).

## Ep. 10 — El endpoint que enumera pero no habla (teclado xHCI)
**Síntoma**: el teclado USB (un numpad) ENUMERA — CABINA dice `kbd=OK(s2)`,
control transfers OK — pero al teclear no llega nada: `kev=0`, y el contador
de transfer events `tev=1` queda pegado (y ese 1 era ruido de otro slot).
**Culpable (parcial)**: el Endpoint Context del xHCI escribía DW4 solo con
Average TRB Length, dejando **Max ESIT Payload = 0**. Para un endpoint
periódico (interrupción), payload 0 = el xHC le asigna **cero ancho de banda**
→ nunca lo sirve → las teclas jamás completan. Fix: `DW4 = (max_pkt<<16) | 8`.
Necesario, pero NO bastó: el endpoint del teclado (DCI 5) sigue mudo.
**Estado**: hipótesis viva — el numpad es **low/full-speed detrás de un hub
interno** (aparece un `slot 1` misterioso), y xHCI agenda LS/FS con codificación
de intervalo distinta (+ TT). Pendiente: teclado normal en puerto trasero, o
codificar el intervalo FS/LS.
**Moraleja**: "enumera" ≠ "habla". El control endpoint (EP0) puede funcionar
perfecto mientras el de interrupción nunca arranca — son caminos distintos del
mismo dispositivo. Y sin un contador que confiese `tev`, esto es invisible: la
telemetría (CABINA) fue la que hizo el bug legible.

## Ep. 11 — CABINA abre los ojos (de estructuras muertas a observador)
**Contexto**: debuggear a fotos, panel por panel, era brutal ("brusco y duro").
La cura estaba dormida en el propio repo: `cabina-core`, una librería de
telemetría (Event con severidad/capa, TelemetrySnapshot) que **nadie había
cableado**. Se le dio vida: `ring0/cabina.rs` construye snapshots de los
contadores vivos y pinta un cockpit omnisciente + una bitácora de eventos con
color por severidad. CABINA ahora **narra** lo que ve (kernel operativo, disco
NVMe detectado, teclado sin teclas como FAULT naranja).
**Trampa**: pintarla desde el timer (IRQ) — switch de CR3 + 4 filas de
framebuffer por interrupción — colgaba→reset al arranque. **Moraleja**: dibujar
pesado en contexto de IRQ es veneno; el shell loop (CR3 kernel, sin IRQ) es el
lugar seguro. CABINA se mantiene always-on desde ahí.
**Lo que quedó**: el sistema dejó de ser una caja negra — se explica a sí mismo,
constantemente, con color. Menos adivinar, más ver. El siguiente escalón es que
esa bitácora se persista al SSD (NVMe) = la caja negra forense de verdad.

## Ep. 12 — El teclado que era una lotería, y el exponente

**Síntoma**: el teclado USB enumeraba y su endpoint de interrupción no
completaba JAMÁS. `tev` pegado, `kev=0`. Semanas así.

**Causa**: el campo `Interval` del Endpoint Context **no es lineal, es un
EXPONENTE**: el xHC sirve el endpoint cada `2^Interval × 125 µs`. Se escribía
el `bInterval` crudo del descriptor, que en Low/Full Speed viene en
MILISEGUNDOS. Un teclado que pide 24 ms quedaba programado a `2^24 × 125 µs` =
**35 minutos** entre sondeos. Con 32, 149 horas. Y `Configure Endpoint`
devolvía ÉXITO — el RGB encendía, todo parecía bien, el xHC simplemente no
consultaba nunca.

**Bonus del mismo día**: el Link TRB del anillo del endpoint no llevaba Toggle
Cycle. Habríamos "arreglado" el teclado y se habría muerto a las ~255
pulsaciones, o sea a los pocos minutos de escribir.

**Y después**: la enumeración resultó ser una LOTERÍA entre arranques — mismo
binario, tres resultados en tres encendidos. Tres reintentos con 50 ms lo
estabilizaron. Lo que parecía un bug determinista era un dispositivo que a
veces no está listo para el primer control transfer.

**Moraleja**: cuando un registro se llama "Interval", léete qué unidad usa
antes de meterle el número que traía el descriptor. Y un `FAIL` sin código de
error es un mensaje que no sirve para nada.

---

## Ep. 13 — El disco estaba donde el firmware juraba que no había nada

**Síntoma**: el HBA SATA aparecía en el PCI, sus registros se leían perfectos
(`cap=0xEF36FF27 pi=0x33`), y los cuatro puertos que `PI` declaraba decían
`DET=0`: ningún disco. Pero la máquina había ARRANCADO de ese disco.

**Camino** (cada paso destapó el siguiente):
1. `find_storage()` devolvía "el primero del barrido" — y en esta máquina el
   primero es el NVMe **con el Windows del dueño**. Se pasó a pedir por TIPO.
2. El driver AHCI nunca había tocado silicio: escribía la dirección de la
   command table DENTRO de la propia tabla (dejando la cabecera en ceros, así
   que el FIS se construía en la **página física 0**), metía el puntero
   VIRTUAL en el PRDT, no tenía timeouts y no miraba `PxTFD.ERR` ni `PRDBC`.
   Reescrito contra la especificación.
3. El `GHC.HR` del arranque **tiraba los enlaces** y se leía `PxSSTS` un
   microsegundo después. Fuera el reset: el firmware ya los había dejado listos.
4. El censo inventaba **puertos fantasma** — filtraba por `p.port_number`, que
   en las entradas vacías del array vale 0, así que cada hueco pasaba haciéndose
   pasar por el puerto 0. Catorce líneas idénticas, y una espera de enlace
   concedida a cada fantasma: los 3-4 segundos de arranque de más.
5. El firmware **PARA los puertos al salir** (`cmd=0x6`, ST y FRE en cero).
   Encender el disco no basta: hay que renegociar el enlace con un COMRESET
   por `PxSCTL`, y con esperas de tiempo REAL (contar vueltas de bucle mide la
   velocidad del CPU, no milisegundos).
6. **`PI` MIENTE.** Existe un caso conocido en Linux (parche "ahci: Acer
   SA5-271 SSD Not Detected Fix") donde el mapa de puertos del firmware hace
   al driver saltarse justo el puerto del disco. Se pasó a barrer los 8
   puertos que `CAP.NP` declara, marcando con `!` los que `PI` negaba.

**El hallazgo**: `[ahci] !p0x2 ssts=0x133 sig=0x101`. El disco estaba en el
puerto 2 — uno de los que `PI` decía que no existían. Verificado sector a
sector contra el anfitrión: 447 GiB, ESP en LBA 2048, 32 GiB en 1230848,
414 GiB en 68339712. El Kingston de BMO.

**Moraleja**: los registros del hardware son testimonio, no verdad. `PI` es un
número que escribió un firmware, y un firmware es software de alguien que ya
se fue. Cuando el testimonio y la realidad no cuadran, se duda del testimonio.

---

## Ep. 14 — XSAVE no guarda: hace MERGE

**Síntoma**: `#GP(0)` con `rip=0x4000D0`. Intermitente. A veces a los 8.000
ticks, a veces al primero. El sello del contexto INTACTO.

**Camino** (cinco sondas, cuatro pantallas azules, y cada instrumento mató
una hipótesis mía):

1. **Desensamblar el `rip`.** No era un `iretq` como parecía: era el
   `xrstor64` del epílogo del timer. El kernel se enlaza en `0x400000` — no
   confundir con `USER_IMAGE_BASE = 0x40000000`, ni con el `linker.ld` de la
   raíz del repo, que está desfasado.
2. El sello (`BMO1` en `+1008`) pasaba y el back-pointer (`+1024`) también:
   el área estaba vigilada por los dos **extremos**, y la cabecera XSAVE
   (`+512`) quedaba en medio **sin que la mirara nadie**.
3. **Guardia de cabecera** en los cinco epílogos. Convirtió un `#GP` mudo en
   `ROTTEN CONTEXT: XSAVE header`, con campo y dueño. Saltó a la primera.
4. **Anillo de publicaciones** (`pub0..pub3`): mató la hipótesis del solape
   de áreas — las dos distaban 2624 bytes, no se tocaban.
5. **`bv0`** (la cabecera al entrar al despachador): mató la hipótesis del
   planificador. Ya venía podrida antes de que nadie hiciera nada.
6. **`bvX`/`baseX`**, leídos por el PROPIO STUB una instrucción después del
   `xsave64`, sin ninguna indirección. Ahí la contradicción quedó desnuda: el
   `xsave64` corría y dejaba basura en `XSTATE_BV`.

**El hallazgo**: `XSAVE` no inicializa la cabecera. Hace

```text
XSTATE_BV ← (XSTATE_BV_viejo AND NOT RFBM) OR (XINUSE AND RFBM)
```

con `RFBM = EDX:EAX AND XCR0`. **Conserva todos los bits fuera de XCR0** del
valor anterior, y los 48 bytes reservados no los toca en absoluto. Los stubs
tallaban el área sobre la pila —o sea sobre basura— y esa basura sobrevivía
al guardado. `XRSTOR` la rechaza con `#GP(0)`. `trap::fabricate` nunca lo
sufrió porque pone a cero los 1024 bytes antes de nada; los stubs no. Ésa era
la asimetría.

**La firma que lo delató**: los volcados daban `0x5F0FCB` y `0x37B`, y los
dos son *el valor viejo con los tres bits bajos puestos a 3* — y 3 es
exactamente `XINUSE & 7` (x87 y SSE en uso, AVX en estado inicial). Un campo
corrupto con unos pocos bits bajos coherentes no es corrupción: **es una
instrucción haciendo merge donde creíamos store.**

**Moraleja**: cuando una instrucción tiene pareja —guardar/restaurar,
abrir/cerrar— hay que leer en la spec **qué campos escribe cada una y si hace
merge o store**. Y si un área se talla sobre la pila, alguien tiene que
ponerla a cero: `sub rsp` no limpia nada.

---

## Ep. 15 — Tres minas del mismo tipo, con tres periféricos distintos

El mismo día, tres fallos que parecían no tener relación:

**`#PF` en `cr2=0xFC2004F8`** — el ERDP del xHCI. **`#PF` en
`cr2=0xFC680320`** — los registros del puerto AHCI. Los dos con `err=0`
(lectura/escritura sobre página **ausente**, en supervisor).

**Culpable, el mismo**: en un `SYSCALL` desde Ring 3, **el CR3 sigue siendo
el del llamante**. El espacio de una tarea de usuario mapea el kernel y su
pila, pero **no el agujero de MMIO**. Mientras el único que tocaba hardware
era el shell de Ring 0 —tarea de kernel, CR3 de kernel— no se notaba. En
cuanto `KIND_INPUT` entregó teclas y `OP_EJECUTAR` leyó el disco, los dos
caminos se recorrieron desde dentro de un syscall.

Ya estaba anotado para el framebuffer en `fault_dispatch` ("el CR3 de usuario
puede no mapear el rango identidad") — pero como una nota sobre *el
framebuffer*, no como una regla.

**Y el tercero, `#GP(0x8)` al escribir `ktest`**: `KERNEL_SS` valía `0x08`,
que en esta GDT es el selector de **CÓDIGO** de Ring 0. En modo largo el
`iretq` saca `SS:RSP` **siempre**, también al mismo privilegio, y cargar `SS`
con un descriptor de código da `#GP(selector)`. El informe lo cantó solo:
`err=0x00000008` **era el selector culpable, dicho por el propio CPU**. Sólo
mordía al crear una tarea de kernel, y nadie había creado una nunca.

**Moraleja**: la regla no es "el framebuffer necesita CR3 de kernel". Es
**cualquier dirección del rango identidad alto tocada desde un syscall o un
ISR**. Cada capability nueva que llegue a hardware vuelve a pisar esta mina —
y la simetría de las constantes de al lado (`USER_SS` apunta a datos,
`USER_CS` a código) era la comprobación que faltaba mirar arriba.

---

## Ep. 16 — El teclado que se moría si lo aporreabas al arrancar

**Síntoma**: pulsar teclas *durante el arranque* dejaba el teclado muerto
toda la sesión. Reiniciar lo "arreglaba". Sin aporrear, nunca pasaba.

**Culpable**: `evt_poll_nb` escribía el `ERDP` así:

```rust
w32(..., (erdp & 0xFFFF_FFFF) as u32);
```

y `erdp` va alineado a 16 bytes, o sea que **el bit 3 salía siempre 0**. El
bit 3 del ERDP es **EHB** (Event Handler Busy), *write-1-to-clear*: lo pone
el xHC al publicar un evento y el software lo baja escribiéndole un 1. Nunca
se bajaba. El anillo de eventos se llena, el controlador entra en *Event Ring
Full* y **deja de publicar eventos para siempre**.

Aporrear el teclado mientras nadie drena el anillo lo llenaba. Sin aporrear,
nunca se llenaba y el bug era invisible.

**Moraleja**: un bit que el hardware pone y el software tiene que bajar es un
contrato, no un adorno. Y los bugs que dependen de "cuánto tarda el usuario
en hacer algo" sólo aparecen cuando alguien hace *justo* eso.

---

## Ep. 17 — El ratón que enumeraba y nunca era

**Síntoma**: el ratón se detectaba (`m=OK`), pero `ev=0` para siempre y el RGB
del propio ratón **apagado**. Meses culpando al parseo del informe HID.

**Culpable**: una línea del bucle de puertos de `uhid`:

```rust
if found_kbd && found_mouse { break; }
```

El teclado trae **dos** interfaces HID —la suya y una de protocolo de ratón
para las teclas de medios—, así que al enumerarlo se marcaban las dos banderas
y el bucle **cortaba antes de llegar al puerto del ratón**. A un dispositivo
sin `SET_CONFIGURATION` no le arranca ni el firmware: por eso el RGB apagado
era el mejor diagnóstico de todos y estaba a la vista.

**Moraleja**: un `break` de "ya tengo lo que buscaba" asume que **un aparato
es un dispositivo**, y en USB no lo es. Y cuando algo se enumera pero no habla,
mirar lo que el propio aparato dice de sí mismo (una luz) antes que el
software: el hardware confiesa gratis.

---

## Ep. 18 — El anillo de eventos compartido, o cómo un arreglo dejó mudos a los dos

**Síntoma**: tras arreglar el Ep. 17, teclado y ratón **los dos mudos**.
`k=OK(s3) m=OK(s2)` (slots distintos ✅, RGB encendido ✅) y sin embargo
`kev=0`, `raton ev=0`, y el último Transfer Event venía del **slot 1, EP0** —
de ninguno de los dos. Y `kbd ep=Running`: el endpoint agendado y sin llegar
nada.

**Culpable**: el anillo de eventos del xHC es **uno para todo el controlador**.
`evt_poll_block` devolvía el primero que pasara. `send_cmd` y
`control_transfer` al menos descartaban lo ajeno; `address_device` y
`configure_endpoint` **ni miraban el tipo** y le leían el `cc` — y un Transfer
Event correcto también trae `cc=1`, así que **un informe del ratón se leía como
"el comando salió bien"**.

Llevaba meses dormido porque nada bombeaba mientras se enumeraba. Lo despertó
**quitar el `break` del Ep. 17**: por primera vez un endpoint quedó vivo
mientras se enumeraba el puerto siguiente. Y aquí está lo letal: en un endpoint
de interrupción **el evento ES el permiso para volver a encolar**. Perder uno
no pierde una pulsación: **para la bomba para siempre**, sin un solo error.

**El arreglo**: `Espera::{Comando, Transferencia{slot,ep}}`, un **aparcadero**
de 64 eventos (lo que no es mío se aparca, **jamás se tira**), y las bombas de
interrupción se arrancan **al final** de la enumeración, no al reconocer cada
aparato.

**Moraleja**: ante una cola compartida, la pregunta no es "¿leo bien?" sino
**"¿qué hago con lo que saco y no es mío?"**. Sólo hay una respuesta: aparcarlo
y contar los que se pierden. Y no enciendas una bomba mientras todavía estás
enumerando.

---

## Ep. 19 — La política que nadie consultaba (sin foto, y por eso duele)

**Síntoma**: ninguno. Compilaba, 461 tests en verde, el commit describía tres
modos de foco y una ventanita de Alt+Tab que se pintaba de verdad en pantalla.

**Culpable**: `grep es_para main.rs` → **nada**. La política de foco se había
escrito entera con sus tests, se le notificaba qué ventana se abría y cuál se
cerraba, se pintaba lo que decidía… y **ninguna tecla se enrutaba con ella**.
Todas seguían cayendo en la caja de Ejecutar aunque la consola de datos
estuviera encima: se escribía en una ventana tapada, sin verlo.

Es el módulo nuevo apareciendo en el diff **escribiendo** (se le notifica, se
le pinta) y nunca **respondiendo**.

**Moraleja**: cuando se añada algo que DECIDE, buscar su función de consulta en
todo el repo antes de dar el trabajo por hecho. Si sus únicos llamantes están
en sus propios tests, **no está cableado: está escrito**. Y da igual cuántos
tests tenga, porque prueban la política, no que alguien la obedezca.

El corolario cuesta más de tragar: este episodio **no necesitó una foto**.
Bastó con desconfiar del commit anterior en vez de creérselo. Las fotos
encuentran lo que el hardware hace mal; esto lo encuentra leer lo que el
código **no hace**.

---

## Ep. 20 — El write-combining a medias, o "tengo que apuntar bien para que me pinte"

**Síntoma**, dicho por quien lo sufría: *"cuando muevo el ratón tengo que
apuntar bien para que me pinte las escrituras, y eso no tenía sentido"*.
Tecleabas y no aparecía nada; movías el ratón y aparecía de golpe.

**Culpable**: el write-combining del framebuffer, puesto **el día anterior** y
sin su otra mitad. Con memoria WC el CPU acumula las escrituras en un búfer y
las suelta cuando se llena. El escáner de vídeo lee **la memoria**, no el búfer.
Así que lo tecleado se quedaba esperando — y mover el ratón generaba las
escrituras que llenaban el búfer y lo empujaban todo a la vez.

Buscar `sfence` en todo el userspace daba **cero resultados**.

**Moraleja**: una optimización que cambia *cuándo* se ve un dato no está
terminada hasta que alguien decide *cuándo tiene que verse*. WC sin barrera no
es más rápido: es otra cosa. Y el corolario incómodo — el síntoma no se parecía
en nada a la causa: hablaba del ratón, y el ratón no tenía culpa de nada.

---

## Ep. 21 — Tres arranques culpando al compositor de algo que hacía un demo

**Síntoma**: en cada arranque, CABINA decía `fb: el dueño de la pantalla MURIO`
y el panel del kernel aparecía pintado **encima** del escritorio. Conclusión
evidente y equivocada: *el compositor se muere al arrancar*.

Se construyó un instrumento para cazarlo — guardar las **últimas palabras** de
un proceso y reimprimirlas al morir. Funcionó a la primera. Y lo que dijeron no
era lo que nadie esperaba:

```
gui: BMO-X: hola mundo desde Ring 3
gui: CPL3 -> ¡reclamo pantalla y entrada!
```

**Culpable**: `init_hello.bex`, el demo en ensamblador. Reclamaba la pantalla
para demostrar que Ring 3 podía pintarla, imprimía sus tres líneas y terminaba
— **y terminar es exactamente lo que tenía que hacer**. Al morir, el kernel
recuperaba la pantalla y repintaba su panel sobre el escritorio recién nacido.
El compositor estaba vivo todo el rato.

**Moraleja**: el instrumento acertó; la teoría era del que lo construyó. Cuando
un aviso dice "murió el dueño de X", la primera pregunta no es *por qué murió*,
es **quién era el dueño**. Y la de fondo: los programas de ejemplo que se
arrancan solos dejan de ser ejemplos y pasan a ser **participantes** — compiten
por los mismos recursos que lo de verdad. Se quitaron del arranque, y el kernel
adelgazó 37 KB.

---

## Ep. 22 — El ratón que se movía al hacer clic

**Síntoma**: *"muevo y no funciona, pero al hacer click cualquiera, se mueven"*.

**Culpable**: tres números del panel lo decían entero. `bot=0b01` fijo (nunca
cambiaba), `x=0` al mover, `y` derivando sola. El aparato **ignoró el
`SET_PROTOCOL(boot)`** y seguía mandando su informe de protocolo de informe,
que empieza por un **Report ID**. Todo corrido un byte: donde el driver leía el
desplazamiento en X caían los **botones**, y por eso pulsar movía el puntero.

`SET_PROTOCOL` se mandaba y **nadie miraba si había servido**. Con
`GET_PROTOCOL` detrás, el aparato lo confesó solo: `protocolo=0x1 (INFORME: el
aparato ignoró el BOOT)`.

**Moraleja**: a un dispositivo se le PREGUNTA en qué estado quedó; no se supone
que obedeció. Un `set` sin su `get` es una carta enviada sin acuse de recibo — y
en un bus donde el otro extremo tiene su propio firmware, eso es optimismo.

---

## Ep. 23 — El `malloc` que sólo descarrilaba al fallar

**Síntoma**: ninguno. Ésa es la gracia. `KIND_MEMORIA` se cableó de punta a
punta, compiló, pasó el drift guard y se documentó con su límite declarado —
*"un quinto `malloc` devuelve 0, que es lo que un programa de C ya sabe
comprobar"*. Y era mentira.

Lo destapó escribir el programa que la estrenaba. El emulador dijo:

```
opcode 0x05 no emitido por BMO
```

**Culpable**: el codegen de `malloc` emitía sus dos saltos con
desplazamientos **contados a mano**, y el primero se quedó seis bytes corto —
`jnz +0x1D` cuando el camino hasta el `xor rax, rax` mide 35. O sea que cuando
el kernel RECHAZABA la petición, el salto caía dentro del `jnz` siguiente y el
CPU seguía leyendo a media instrucción. En el Ryzen eso no habría devuelto 0:
habría matado el proceso.

Lo que lo hacía invisible: **la rama buena estaba bien**. Un `malloc` que
funciona cuatro veces y descarrila a la quinta pasa por correcto en cualquier
prueba que no llegue a la quinta — y ninguna llegaba, porque el emulador
tampoco modelaba la petición y todo `malloc` devolvía 0 en silencio. Dos
agujeros tapándose el uno al otro.

**Moraleja**: contar bytes a mano es escribir un enlazador en la cabeza cada
vez que alguien mete una instrucción en medio. Las etiquetas ya estaban en el
codegen; sólo había que usarlas. Y la de fondo: **una rama de error que nadie
ejecuta no está escrita, está redactada.** El límite de cuatro peticiones
existía en la documentación y en el kernel; el camino de vuelta al programa,
no.

---

## Ep. 24 — Ocho bytes de log para una pregunta que contesta el aparato

**Síntoma**: `raton x=-4332` — un desplazamiento que ninguna mano hace.

Después de que el ratón confesara `protocolo=0x1` (Ep. 22) quedó abierto si
sus ejes eran de 8 o de 16 bits. Si eran de 16, el byte que el driver leía
como `dy` era la mitad alta de `dx`: mover en horizontal movería en vertical.
El plan era registrar **ocho bytes crudos** del informe y decidir mirando la
foto: si los bytes 4..7 traen datos, son 16 bits.

**El plan estaba mal**, y no por el instrumento. Un formato no se decide
mirando datos: se pregunta. Todo HID lleva su **Report Descriptor**, que dice
literalmente qué bit es cada campo y de cuántos bits — y este driver nunca se
lo había pedido a nadie porque el protocolo BOOT le ahorraba el parser. En
cuanto un aparato ignoró el BOOT, ese ahorro pasó a ser el problema.

Se le pide (`GET_DESCRIPTOR`, tipo 0x22) y se lee. El parser saca cuatro
campos —botones, X, Y, rueda— con su posición en bits y su tamaño, respetando
lo que de verdad cuesta hacer bien: `Report Size`/`Report Count` en bits, el
desplazamiento acumulado **por Report ID**, el relleno (`Input (Cnst)`) que
ocupa sitio y no significa nada, y el reparto de usages por lista o por rango.

**Moraleja**: es el Ep. 22 otra vez, un nivel más arriba. Allí la lección fue
*a un dispositivo se le pregunta en qué estado quedó*; aquí es **a un
dispositivo se le pregunta qué formato habla**. Los ocho bytes crudos se
quedan en el log, pero ya no para adivinar: para comprobar que lo que el
descriptor promete es lo que el aparato manda.

---

## Ep. 25 — El write-combining, otra vez, y por el lado que nadie miró (sin foto todavía)

**Síntoma**, dicho por el dueño: *"que no me salgan ghosting"* — un rastro que
sigue al puntero.

El Ep. 20 dejó cerrado que **la pantalla** no ve nuestras escrituras sin
`sfence`. Lo que nadie se preguntó es lo simétrico: **¿las vemos nosotros?**

El compositor lee el framebuffer en **un solo sitio** de todo el programa: el
*save-under* del cursor, que guarda los 160 píxeles de debajo para devolverlos
al moverse. Y lo hace **al final del fotograma, justo antes del único
`sfence`**:

```text
  1. quitar        -> escribe (al bufer WC)
  2. pintar todo   -> escribe (al bufer WC)
  3. poner: LEER   <- ve la pantalla de HACE UN FOTOGRAMA
  4. vaciar        -> sfence: ahora sí llega todo
```

**Culpable**: una lectura de memoria WC no está ordenada contra las escrituras
pendientes en el búfer. Así que el paso 3 guardaba píxeles **caducados**, y el
`quitar` de la vuelta siguiente los devolvía **encima de lo nuevo**. Un
rectángulo de 10×16 con contenido viejo persiguiendo al ratón: eso es
exactamente el ghosting.

El comentario de `Pantalla::leer` lo decía sin saberlo — *"el framebuffer es
memoria de este proceso, así que se puede leer"*. Era cierto cuando se escribió.
Dejó de serlo el día que esa memoria pasó a WC, dos días antes, y **nadie
revisó a los lectores** porque el cambio se pensó como una optimización de
escritura.

Y de paso salió un segundo: `pintar_calc` es **el único pintado del bucle que no
dispara la entrada** —lo dispara el hijo al contestar—, así que puede caer en un
fotograma con el cursor todavía puesto. Pintar ahí caduca el guardado igual.

**Moraleja**: cambiar el tipo de memoria de una región no es un cambio local, es
un cambio de **contrato**, y hay que ir a buscar a todos los que lo usaban con
el contrato viejo — incluidos los que sólo leen. La pregunta que lo habría
cazado en el minuto uno es de una línea: *¿quién LEE esto?*. En este programa la
respuesta cabía en un `grep` y daba un solo resultado.

---

## Ep. 26 — El escritor y el lector miraban extremos opuestos del mismo buffer

**Síntoma**, dicho por quien lo sufría: *"el `ls` ya ejecuté normal pero no
muestra nada"*.

Y era literal: el comando corría, la línea de estado ponía `listo`, y la rejilla
de salida se quedaba en blanco. Ni un error, ni un cuelgue. La forma más
incómoda de fallo — la que se parece a "no hace nada" y en realidad es **"lo
hace donde nadie mira"**.

**Culpable**, en dos líneas que están a 220 de distancia en el mismo archivo:

```rust
Salida::nueva()  ->  fila: 0                          // el ESCRITOR empieza arriba
pintar_salida()  ->  base = SAL_HIST - SAL_ROWS       // el LECTOR enseña abajo
```

`SAL_HIST` son 200 filas y `SAL_ROWS` son 16, así que la ventana visible es
`celdas[184..200]` y el primer texto se escribía en `celdas[0]`. **Las 184
primeras líneas de cualquier programa eran invisibles.** `ls` escupe una docena.

Lo trajo el historial con scroll (`8ee091e2`): antes la rejilla eran 16 filas y
escribir desde la 0 era exactamente lo correcto. Ese commit convirtió la rejilla
en una **ventana sobre 200 filas** y movió al lector al final del buffer — y
dejó al escritor donde siempre había estado. Nadie miró al otro extremo porque
el que se estaba tocando funcionaba.

**Y por qué no lo cazó nadie antes**: el arreglo del scroll traía su prueba
escrita —*"llenar la salida con `ls`, subir con PgUp"*— y esa prueba nunca se
ejecutó en metal. Estuvo meses en la lista de pendientes de hardware.

**Moraleja**: cuando un cambio mueve un **extremo** de una estructura
compartida, hay exactamente dos sitios que revisar, y el segundo es el que no se
está tocando. Un buffer con escritor y lector tiene dos contratos, no uno. Y el
corolario: **una prueba escrita y no ejecutada no protege de nada** — es la
misma ley 13, otra vez, sobre otro código.

*Nota de método*: esto se encontró **leyendo**, no adivinando, y sólo porque la
foto traía el dato que discriminaba (`listo` pintado + rejilla vacía = el
comando corrió y la salida se perdió). Sin esa distinción, la teoría fácil era
"el `ls` falla" y se habría buscado en el driver de directorio.

---

## Las leyes que dejó esta guerra

1. **QEMU miente por omisión**: sin IRQs vivos, sin tiempos físicos, sin
   memoria con huecos. Todo lo que "funciona en QEMU" es una hipótesis.
2. **Los bugs viejos disfrazan a los nuevos**: el CS fantasma (Ep. 4)
   causaba el split-brain (Ep. 5) que tapaba el fb invisible (Ep. 6). Se
   pelan como cebolla, en orden, con una foto por capa.
3. **La telemetría en pantalla vale más que mil teorías**: cada episodio
   cayó cuando el sistema mismo confesó (filas de diagnóstico, censos,
   heartbeats). Si no puedes verlo, no puedes matarlo.
4. **Un instrumento que mata tu hipótesis vale más que uno que la
   confirma** (Ep. 14). Las cinco sondas de XSAVE tumbaron cuatro teorías
   antes de acertar. Cada "no era eso" recortó el espacio de búsqueda a la
   mitad; una sonda que sólo hubiera dicho "sí" no habría recortado nada.
5. **El informe de fallo ya sabe más de lo que se lee.** `err=0x00000008` no
   era un número: era el selector culpable, dicho por el CPU (Ep. 15). Antes
   de añadir un campo nuevo, leer entero el que ya está.
6. **Una regla escrita para un caso concreto no protege del siguiente**
   (Ep. 15). "El framebuffer necesita CR3 de kernel" era cierto y era
   inútil: la regla de verdad era *cualquier dirección del rango identidad
   tocada desde un syscall*, y estaba a un periférico de distancia.
7. **Arreglar un bug despierta a los que dormían debajo** (Ep. 17 → 18). El
   `break` de más tapaba un anillo de eventos mal repartido desde el primer
   día; quitarlo no rompió nada nuevo, **destapó** lo que llevaba meses
   escrito y nunca ejercido. Un arreglo que hace aparecer un fallo peor suele
   ser el arreglo correcto.
8. **Verde no es cableado** (Ep. 19). Un módulo puede compilar, pasar todos
   sus tests, aparecer en el commit y no ser consultado por nadie. Los tests
   prueban la política; no prueban que alguien la obedezca. La comprobación
   dura dos segundos: buscar quién LLAMA a la función que contesta.

9. **Un aviso correcto no implica una teoría correcta** (Ep. 21). "Murió el
   dueño de la pantalla" era cierto tres arranques seguidos, y la conclusión
   que se sacó era falsa. Antes de preguntar *por qué pasó*, preguntar **a
   quién le pasó**.
10. **Una optimización que cambia CUÁNDO se ve algo no está terminada**
   (Ep. 20) hasta que alguien decide cuándo tiene que verse. El
   write-combining sin `sfence` no era rápido: era incorrecto.
11. **A un dispositivo se le pregunta, no se le supone** (Ep. 22 y 24). Un
   `set` sin su `get` es una carta sin acuse de recibo, y al otro lado hay un
   firmware con sus propias ideas. La versión fuerte: tampoco se le supone el
   **formato** — el Report Descriptor está ahí para eso, y adivinarlo mirando
   bytes crudos es leerlo en la variable equivocada.
12. **Cambiar el tipo de memoria de una región es cambiar un CONTRATO**
   (Ep. 25), no hacer una optimización local. Hay que ir a buscar a todos los
   que la usaban con el contrato viejo — **y los lectores cuentan**. El WC se
   pensó como un cambio de escritura y rompió la única lectura que había.
13. **Un buffer compartido tiene DOS contratos, no uno** (Ep. 26). Cuando un
   cambio mueve un extremo —dónde empieza a leer, dónde empieza a escribir—, el
   sitio que hay que revisar es **el que no estás tocando**. El escritor
   empezaba arriba y el lector enseñaba abajo, y las dos líneas eran correctas
   por separado.
14. **Una rama de error que nadie ejecuta no está escrita, está redactada**
   (Ep. 23). El camino bueno de `malloc` funcionaba y el de fallo saltaba a
   media instrucción; el límite existía en el kernel y en la documentación, y
   el programa nunca llegaba a verlo. Escribir el programa que ejerce el
   límite es parte de implementar el límite.

*Debuggeado a fotos de pantalla, entre un humano con hardware y una IA sin
ojos. 2026.*
