# Bitacora de guerra -- BMO-X en hardware real

Episodios de debugging en metal desnudo (MSI A320M PRO MAX + Ryzen 5 5600X),
sin debugger, sin serial conectado: **solo fotos de la pantalla**. Cada
episodio: el sintoma, el culpable, y la moraleja que quedo grabada en el
codigo.

---

## Ep. 1 -- El firmware que no queria soltar sus archivos
**Sintoma**: "no FAT filesystem found" -- la MSI arranca con lector FAT
interno y jamas conecta drivers SimpleFS.
**Culpable**: fast-boot de fabrica sin opcion visible.
**Moraleja**: no le pidas archivos al firmware -- **embebelo todo** en un solo
BOOTX64.EFI (shim unificado con las etapas y el kernel adentro). Cero
dependencias, cero mercedes.

## Ep. 2 -- El triple fault que solo pasaba en hardware
**Sintoma**: bootea en QEMU, reset instantaneo en la placa real.
**Culpable**: el firmware entrega con interrupciones ENCENDIDAS; un IRQ en
plena cirugia de GDT despacha con tablas inconsistentes.
**Moraleja**: `cli` + enmascarar el PIC ANTES de tocar la GDT. QEMU es un
mundo sin ruido; el hardware real tiene trafico.

## Ep. 3 -- Los GUIDs mal copiados (o: por que nunca hubo framebuffer)
**Sintoma**: meses creyendo que la placa "no tenia GOP".
**Culpable**: los GUID de GOP y SimpleFS estaban mal escritos (data4
corrupto). El proyecto siempre corrio por serial y nadie lo noto.
**Moraleja**: un GUID es una contrasena de 16 bytes: o es EXACTA o el
universo responde "no existe".

## Ep. 4 -- El CS fantasma de UEFI (la saga del #GP, capa 1)
**Sintoma**: #GP(0) eterno en el iretq del timer; frame fabricado PERFECTO,
GDT PERFECTA, CR3 compartida PERFECTA. Semanas de misterio.
**Culpable**: `init_gdt` hacia `lgdt` + recargaba los segmentos de datos...
**pero nunca el CS**. El CPU ejecuto Ring 0 entero con el descriptor UEFI
(cs=0x38) cacheado en el shadow register. Todo funcionaba -- hasta que un
iretq re-valido ese selector contra NUESTRA GDT (entrada 7: vacia).
**Moraleja**: `lgdt` no recarga CS. El far-return (`push CS; push RIP;
retfq`) no es opcional -- es el bautizo real del kernel.

## Ep. 5 -- El split-brain de gs (la saga del #GP, capa 2)
**Sintoma**: el contexto se publicaba y el epilogo leia CEROS.
**Culpable**: el asm escribia por `gs:[0x10]` (MSR GS_BASE) y el Rust leia
el static directo. Dos caminos a "la misma" memoria que solo coinciden si
GS_BASE apunta donde crees -- y el CS fantasma (Ep. 4) disparaba swapgs
espurios que lo movian.
**Moraleja**: para datos per-CPU, **un solo camino de acceso**. Escritor y
lector deben concordar POR CONSTRUCCION, no por fe.

## Ep. 6 -- El framebuffer invisible (la saga del #GP, capa 3)
**Sintoma**: con las capas 1 y 2 arregladas... congelamiento TOTAL sin
pantalla ni fault. El hola mundo Ring 3 SI ejecutaba -- moria *pintando*.
**Culpable**: el address space de usuario comparte identidad solo 0..1 GiB;
el fb GOP vive en ~3.5 GiB. El flush de consola pintaba bajo la CR3 del
usuario -> #PF -> el reporter de faults TAMBIEN pinta -> #PF recursivo
infinito en IST1.
**Moraleja**: pregunta siempre **bajo que CR3 corres** antes de tocar MMIO.
Y un fault handler jamas debe poder causar su propio fault.

## Ep. 7 -- El teclado que funcionaba de prestado
**Sintoma**: en BMO/FastOS v0.6-0.9 el teclado escribia; en el BMO-X real,
silencio (solo ruido 0xFE del i8042, ni el LED de Bloq Mayus responde).
**Culpable**: antes los Boot Services estaban vivos y el firmware hacia el
USB por nosotros (emulacion SMM USB->PS/2). Al convertirnos en un OS de
verdad (ExitBootServices), el firmware se llevo su magia.
**Moraleja**: la soberania se paga con drivers. Lo que el firmware te
"regala" es un prestamo con fecha de vencimiento.

## Ep. 8 -- El xHC escondido detras del bridge
**Sintoma**: "[usb] no se encontro controlador xHCI" -- con el controlador
ahi, funcionando.
**Culpable**: el scan PCI del boot era plano (bus 0); en Ryzen los xHC
cuelgan de buses detras de bridges.
**Moraleja**: en PCI, si no recorres TODOS los buses, no has buscado. Y sin
habilitar Bus Master (BME), un controlador DMA es un adorno.

## Ep. 9 -- "Nel, llegas tarde" (el CPU impaciente)
**Sintoma**: xHC inicializado perfecto (127 slots, 22 puertos)... y cero
dispositivos en los puertos. El teclado SEISA conectado, ignorado.
**Culpable**: el spec USB exige ~100 ms de debounce para detectar conexion.
El driver (criado en QEMU, donde todo es instantaneo) esperaba
~microsegundos. Para un Zen 3 a 4.6 GHz, 100 ms son una era geologica -- y
no estaba dispuesto a esperarla.
**Moraleja**: el hardware real tiene TIEMPOS FISICOS. La paciencia no es
una virtud del CPU: hay que programarsela (delays por TSC, no spin-counts).

## Ep. 10 -- El endpoint que enumera pero no habla (teclado xHCI)
**Sintoma**: el teclado USB (un numpad) ENUMERA -- CABINA dice `kbd=OK(s2)`,
control transfers OK -- pero al teclear no llega nada: `kev=0`, y el contador
de transfer events `tev=1` queda pegado (y ese 1 era ruido de otro slot).
**Culpable (parcial)**: el Endpoint Context del xHCI escribia DW4 solo con
Average TRB Length, dejando **Max ESIT Payload = 0**. Para un endpoint
periodico (interrupcion), payload 0 = el xHC le asigna **cero ancho de banda**
-> nunca lo sirve -> las teclas jamas completan. Fix: `DW4 = (max_pkt<<16) | 8`.
Necesario, pero NO basto: el endpoint del teclado (DCI 5) sigue mudo.
**Estado**: hipotesis viva -- el numpad es **low/full-speed detras de un hub
interno** (aparece un `slot 1` misterioso), y xHCI agenda LS/FS con codificacion
de intervalo distinta (+ TT). Pendiente: teclado normal en puerto trasero, o
codificar el intervalo FS/LS.
**Moraleja**: "enumera" != "habla". El control endpoint (EP0) puede funcionar
perfecto mientras el de interrupcion nunca arranca -- son caminos distintos del
mismo dispositivo. Y sin un contador que confiese `tev`, esto es invisible: la
telemetria (CABINA) fue la que hizo el bug legible.

## Ep. 11 -- CABINA abre los ojos (de estructuras muertas a observador)
**Contexto**: debuggear a fotos, panel por panel, era brutal ("brusco y duro").
La cura estaba dormida en el propio repo: `cabina-core`, una libreria de
telemetria (Event con severidad/capa, TelemetrySnapshot) que **nadie habia
cableado**. Se le dio vida: `ring0/cabina.rs` construye snapshots de los
contadores vivos y pinta un cockpit omnisciente + una bitacora de eventos con
color por severidad. CABINA ahora **narra** lo que ve (kernel operativo, disco
NVMe detectado, teclado sin teclas como FAULT naranja).
**Trampa**: pintarla desde el timer (IRQ) -- switch de CR3 + 4 filas de
framebuffer por interrupcion -- colgaba->reset al arranque. **Moraleja**: dibujar
pesado en contexto de IRQ es veneno; el shell loop (CR3 kernel, sin IRQ) es el
lugar seguro. CABINA se mantiene always-on desde ahi.
**Lo que quedo**: el sistema dejo de ser una caja negra -- se explica a si mismo,
constantemente, con color. Menos adivinar, mas ver. El siguiente escalon es que
esa bitacora se persista al SSD (NVMe) = la caja negra forense de verdad.

## Ep. 12 -- El teclado que era una loteria, y el exponente

**Sintoma**: el teclado USB enumeraba y su endpoint de interrupcion no
completaba JAMAS. `tev` pegado, `kev=0`. Semanas asi.

**Causa**: el campo `Interval` del Endpoint Context **no es lineal, es un
EXPONENTE**: el xHC sirve el endpoint cada `2^Interval x 125 us`. Se escribia
el `bInterval` crudo del descriptor, que en Low/Full Speed viene en
MILISEGUNDOS. Un teclado que pide 24 ms quedaba programado a `2^24 x 125 us` =
**35 minutos** entre sondeos. Con 32, 149 horas. Y `Configure Endpoint`
devolvia EXITO -- el RGB encendia, todo parecia bien, el xHC simplemente no
consultaba nunca.

**Bonus del mismo dia**: el Link TRB del anillo del endpoint no llevaba Toggle
Cycle. Habriamos "arreglado" el teclado y se habria muerto a las ~255
pulsaciones, o sea a los pocos minutos de escribir.

**Y despues**: la enumeracion resulto ser una LOTERIA entre arranques -- mismo
binario, tres resultados en tres encendidos. Tres reintentos con 50 ms lo
estabilizaron. Lo que parecia un bug determinista era un dispositivo que a
veces no esta listo para el primer control transfer.

**Moraleja**: cuando un registro se llama "Interval", leete que unidad usa
antes de meterle el numero que traia el descriptor. Y un `FAIL` sin codigo de
error es un mensaje que no sirve para nada.

---

## Ep. 13 -- El disco estaba donde el firmware juraba que no habia nada

**Sintoma**: el HBA SATA aparecia en el PCI, sus registros se leian perfectos
(`cap=0xEF36FF27 pi=0x33`), y los cuatro puertos que `PI` declaraba decian
`DET=0`: ningun disco. Pero la maquina habia ARRANCADO de ese disco.

**Camino** (cada paso destapo el siguiente):
1. `find_storage()` devolvia "el primero del barrido" -- y en esta maquina el
   primero es el NVMe **con el Windows del dueno**. Se paso a pedir por TIPO.
2. El driver AHCI nunca habia tocado silicio: escribia la direccion de la
   command table DENTRO de la propia tabla (dejando la cabecera en ceros, asi
   que el FIS se construia en la **pagina fisica 0**), metia el puntero
   VIRTUAL en el PRDT, no tenia timeouts y no miraba `PxTFD.ERR` ni `PRDBC`.
   Reescrito contra la especificacion.
3. El `GHC.HR` del arranque **tiraba los enlaces** y se leia `PxSSTS` un
   microsegundo despues. Fuera el reset: el firmware ya los habia dejado listos.
4. El censo inventaba **puertos fantasma** -- filtraba por `p.port_number`, que
   en las entradas vacias del array vale 0, asi que cada hueco pasaba haciendose
   pasar por el puerto 0. Catorce lineas identicas, y una espera de enlace
   concedida a cada fantasma: los 3-4 segundos de arranque de mas.
5. El firmware **PARA los puertos al salir** (`cmd=0x6`, ST y FRE en cero).
   Encender el disco no basta: hay que renegociar el enlace con un COMRESET
   por `PxSCTL`, y con esperas de tiempo REAL (contar vueltas de bucle mide la
   velocidad del CPU, no milisegundos).
6. **`PI` MIENTE.** Existe un caso conocido en Linux (parche "ahci: Acer
   SA5-271 SSD Not Detected Fix") donde el mapa de puertos del firmware hace
   al driver saltarse justo el puerto del disco. Se paso a barrer los 8
   puertos que `CAP.NP` declara, marcando con `!` los que `PI` negaba.

**El hallazgo**: `[ahci] !p0x2 ssts=0x133 sig=0x101`. El disco estaba en el
puerto 2 -- uno de los que `PI` decia que no existian. Verificado sector a
sector contra el anfitrion: 447 GiB, ESP en LBA 2048, 32 GiB en 1230848,
414 GiB en 68339712. El Kingston de BMO.

**Moraleja**: los registros del hardware son testimonio, no verdad. `PI` es un
numero que escribio un firmware, y un firmware es software de alguien que ya
se fue. Cuando el testimonio y la realidad no cuadran, se duda del testimonio.

---

## Ep. 14 -- XSAVE no guarda: hace MERGE

**Sintoma**: `#GP(0)` con `rip=0x4000D0`. Intermitente. A veces a los 8.000
ticks, a veces al primero. El sello del contexto INTACTO.

**Camino** (cinco sondas, cuatro pantallas azules, y cada instrumento mato
una hipotesis mia):

1. **Desensamblar el `rip`.** No era un `iretq` como parecia: era el
   `xrstor64` del epilogo del timer. El kernel se enlaza en `0x400000` -- no
   confundir con `USER_IMAGE_BASE = 0x40000000`, ni con el `linker.ld` de la
   raiz del repo, que esta desfasado.
2. El sello (`BMO1` en `+1008`) pasaba y el back-pointer (`+1024`) tambien:
   el area estaba vigilada por los dos **extremos**, y la cabecera XSAVE
   (`+512`) quedaba en medio **sin que la mirara nadie**.
3. **Guardia de cabecera** en los cinco epilogos. Convirtio un `#GP` mudo en
   `ROTTEN CONTEXT: XSAVE header`, con campo y dueno. Salto a la primera.
4. **Anillo de publicaciones** (`pub0..pub3`): mato la hipotesis del solape
   de areas -- las dos distaban 2624 bytes, no se tocaban.
5. **`bv0`** (la cabecera al entrar al despachador): mato la hipotesis del
   planificador. Ya venia podrida antes de que nadie hiciera nada.
6. **`bvX`/`baseX`**, leidos por el PROPIO STUB una instruccion despues del
   `xsave64`, sin ninguna indireccion. Ahi la contradiccion quedo desnuda: el
   `xsave64` corria y dejaba basura en `XSTATE_BV`.

**El hallazgo**: `XSAVE` no inicializa la cabecera. Hace

```text
XSTATE_BV <- (XSTATE_BV_viejo AND NOT RFBM) OR (XINUSE AND RFBM)
```

con `RFBM = EDX:EAX AND XCR0`. **Conserva todos los bits fuera de XCR0** del
valor anterior, y los 48 bytes reservados no los toca en absoluto. Los stubs
tallaban el area sobre la pila --o sea sobre basura-- y esa basura sobrevivia
al guardado. `XRSTOR` la rechaza con `#GP(0)`. `trap::fabricate` nunca lo
sufrio porque pone a cero los 1024 bytes antes de nada; los stubs no. Esa era
la asimetria.

**La firma que lo delato**: los volcados daban `0x5F0FCB` y `0x37B`, y los
dos son *el valor viejo con los tres bits bajos puestos a 3* -- y 3 es
exactamente `XINUSE & 7` (x87 y SSE en uso, AVX en estado inicial). Un campo
corrupto con unos pocos bits bajos coherentes no es corrupcion: **es una
instruccion haciendo merge donde creiamos store.**

**Moraleja**: cuando una instruccion tiene pareja --guardar/restaurar,
abrir/cerrar-- hay que leer en la spec **que campos escribe cada una y si hace
merge o store**. Y si un area se talla sobre la pila, alguien tiene que
ponerla a cero: `sub rsp` no limpia nada.

---

## Ep. 15 -- Tres minas del mismo tipo, con tres perifericos distintos

El mismo dia, tres fallos que parecian no tener relacion:

**`#PF` en `cr2=0xFC2004F8`** -- el ERDP del xHCI. **`#PF` en
`cr2=0xFC680320`** -- los registros del puerto AHCI. Los dos con `err=0`
(lectura/escritura sobre pagina **ausente**, en supervisor).

**Culpable, el mismo**: en un `SYSCALL` desde Ring 3, **el CR3 sigue siendo
el del llamante**. El espacio de una tarea de usuario mapea el kernel y su
pila, pero **no el agujero de MMIO**. Mientras el unico que tocaba hardware
era el shell de Ring 0 --tarea de kernel, CR3 de kernel-- no se notaba. En
cuanto `KIND_INPUT` entrego teclas y `OP_EJECUTAR` leyo el disco, los dos
caminos se recorrieron desde dentro de un syscall.

Ya estaba anotado para el framebuffer en `fault_dispatch` ("el CR3 de usuario
puede no mapear el rango identidad") -- pero como una nota sobre *el
framebuffer*, no como una regla.

**Y el tercero, `#GP(0x8)` al escribir `ktest`**: `KERNEL_SS` valia `0x08`,
que en esta GDT es el selector de **CODIGO** de Ring 0. En modo largo el
`iretq` saca `SS:RSP` **siempre**, tambien al mismo privilegio, y cargar `SS`
con un descriptor de codigo da `#GP(selector)`. El informe lo canto solo:
`err=0x00000008` **era el selector culpable, dicho por el propio CPU**. Solo
mordia al crear una tarea de kernel, y nadie habia creado una nunca.

**Moraleja**: la regla no es "el framebuffer necesita CR3 de kernel". Es
**cualquier direccion del rango identidad alto tocada desde un syscall o un
ISR**. Cada capability nueva que llegue a hardware vuelve a pisar esta mina --
y la simetria de las constantes de al lado (`USER_SS` apunta a datos,
`USER_CS` a codigo) era la comprobacion que faltaba mirar arriba.

---

## Ep. 16 -- El teclado que se moria si lo aporreabas al arrancar

**Sintoma**: pulsar teclas *durante el arranque* dejaba el teclado muerto
toda la sesion. Reiniciar lo "arreglaba". Sin aporrear, nunca pasaba.

**Culpable**: `evt_poll_nb` escribia el `ERDP` asi:

```rust
w32(..., (erdp & 0xFFFF_FFFF) as u32);
```

y `erdp` va alineado a 16 bytes, o sea que **el bit 3 salia siempre 0**. El
bit 3 del ERDP es **EHB** (Event Handler Busy), *write-1-to-clear*: lo pone
el xHC al publicar un evento y el software lo baja escribiendole un 1. Nunca
se bajaba. El anillo de eventos se llena, el controlador entra en *Event Ring
Full* y **deja de publicar eventos para siempre**.

Aporrear el teclado mientras nadie drena el anillo lo llenaba. Sin aporrear,
nunca se llenaba y el bug era invisible.

**Moraleja**: un bit que el hardware pone y el software tiene que bajar es un
contrato, no un adorno. Y los bugs que dependen de "cuanto tarda el usuario
en hacer algo" solo aparecen cuando alguien hace *justo* eso.

---

## Ep. 17 -- El raton que enumeraba y nunca era

**Sintoma**: el raton se detectaba (`m=OK`), pero `ev=0` para siempre y el RGB
del propio raton **apagado**. Meses culpando al parseo del informe HID.

**Culpable**: una linea del bucle de puertos de `uhid`:

```rust
if found_kbd && found_mouse { break; }
```

El teclado trae **dos** interfaces HID --la suya y una de protocolo de raton
para las teclas de medios--, asi que al enumerarlo se marcaban las dos banderas
y el bucle **cortaba antes de llegar al puerto del raton**. A un dispositivo
sin `SET_CONFIGURATION` no le arranca ni el firmware: por eso el RGB apagado
era el mejor diagnostico de todos y estaba a la vista.

**Moraleja**: un `break` de "ya tengo lo que buscaba" asume que **un aparato
es un dispositivo**, y en USB no lo es. Y cuando algo se enumera pero no habla,
mirar lo que el propio aparato dice de si mismo (una luz) antes que el
software: el hardware confiesa gratis.

---

## Ep. 18 -- El anillo de eventos compartido, o como un arreglo dejo mudos a los dos

**Sintoma**: tras arreglar el Ep. 17, teclado y raton **los dos mudos**.
`k=OK(s3) m=OK(s2)` (slots distintos ✅, RGB encendido ✅) y sin embargo
`kev=0`, `raton ev=0`, y el ultimo Transfer Event venia del **slot 1, EP0** --
de ninguno de los dos. Y `kbd ep=Running`: el endpoint agendado y sin llegar
nada.

**Culpable**: el anillo de eventos del xHC es **uno para todo el controlador**.
`evt_poll_block` devolvia el primero que pasara. `send_cmd` y
`control_transfer` al menos descartaban lo ajeno; `address_device` y
`configure_endpoint` **ni miraban el tipo** y le leian el `cc` -- y un Transfer
Event correcto tambien trae `cc=1`, asi que **un informe del raton se leia como
"el comando salio bien"**.

Llevaba meses dormido porque nada bombeaba mientras se enumeraba. Lo desperto
**quitar el `break` del Ep. 17**: por primera vez un endpoint quedo vivo
mientras se enumeraba el puerto siguiente. Y aqui esta lo letal: en un endpoint
de interrupcion **el evento ES el permiso para volver a encolar**. Perder uno
no pierde una pulsacion: **para la bomba para siempre**, sin un solo error.

**El arreglo**: `Espera::{Comando, Transferencia{slot,ep}}`, un **aparcadero**
de 64 eventos (lo que no es mio se aparca, **jamas se tira**), y las bombas de
interrupcion se arrancan **al final** de la enumeracion, no al reconocer cada
aparato.

**Moraleja**: ante una cola compartida, la pregunta no es "leo bien?" sino
**"que hago con lo que saco y no es mio?"**. Solo hay una respuesta: aparcarlo
y contar los que se pierden. Y no enciendas una bomba mientras todavia estas
enumerando.

---

## Ep. 19 -- La politica que nadie consultaba (sin foto, y por eso duele)

**Sintoma**: ninguno. Compilaba, 461 tests en verde, el commit describia tres
modos de foco y una ventanita de Alt+Tab que se pintaba de verdad en pantalla.

**Culpable**: `grep es_para main.rs` -> **nada**. La politica de foco se habia
escrito entera con sus tests, se le notificaba que ventana se abria y cual se
cerraba, se pintaba lo que decidia... y **ninguna tecla se enrutaba con ella**.
Todas seguian cayendo en la caja de Ejecutar aunque la consola de datos
estuviera encima: se escribia en una ventana tapada, sin verlo.

Es el modulo nuevo apareciendo en el diff **escribiendo** (se le notifica, se
le pinta) y nunca **respondiendo**.

**Moraleja**: cuando se anada algo que DECIDE, buscar su funcion de consulta en
todo el repo antes de dar el trabajo por hecho. Si sus unicos llamantes estan
en sus propios tests, **no esta cableado: esta escrito**. Y da igual cuantos
tests tenga, porque prueban la politica, no que alguien la obedezca.

El corolario cuesta mas de tragar: este episodio **no necesito una foto**.
Basto con desconfiar del commit anterior en vez de creerselo. Las fotos
encuentran lo que el hardware hace mal; esto lo encuentra leer lo que el
codigo **no hace**.

---

## Ep. 20 -- El write-combining a medias, o "tengo que apuntar bien para que me pinte"

**Sintoma**, dicho por quien lo sufria: *"cuando muevo el raton tengo que
apuntar bien para que me pinte las escrituras, y eso no tenia sentido"*.
Tecleabas y no aparecia nada; movias el raton y aparecia de golpe.

**Culpable**: el write-combining del framebuffer, puesto **el dia anterior** y
sin su otra mitad. Con memoria WC el CPU acumula las escrituras en un bufer y
las suelta cuando se llena. El escaner de video lee **la memoria**, no el bufer.
Asi que lo tecleado se quedaba esperando -- y mover el raton generaba las
escrituras que llenaban el bufer y lo empujaban todo a la vez.

Buscar `sfence` en todo el userspace daba **cero resultados**.

**Moraleja**: una optimizacion que cambia *cuando* se ve un dato no esta
terminada hasta que alguien decide *cuando tiene que verse*. WC sin barrera no
es mas rapido: es otra cosa. Y el corolario incomodo -- el sintoma no se parecia
en nada a la causa: hablaba del raton, y el raton no tenia culpa de nada.

---

## Ep. 21 -- Tres arranques culpando al compositor de algo que hacia un demo

**Sintoma**: en cada arranque, CABINA decia `fb: el dueno de la pantalla MURIO`
y el panel del kernel aparecia pintado **encima** del escritorio. Conclusion
evidente y equivocada: *el compositor se muere al arrancar*.

Se construyo un instrumento para cazarlo -- guardar las **ultimas palabras** de
un proceso y reimprimirlas al morir. Funciono a la primera. Y lo que dijeron no
era lo que nadie esperaba:

```
gui: BMO-X: hola mundo desde Ring 3
gui: CPL3 -> reclamo pantalla y entrada!
```

**Culpable**: `init_hello.bex`, el demo en ensamblador. Reclamaba la pantalla
para demostrar que Ring 3 podia pintarla, imprimia sus tres lineas y terminaba
-- **y terminar es exactamente lo que tenia que hacer**. Al morir, el kernel
recuperaba la pantalla y repintaba su panel sobre el escritorio recien nacido.
El compositor estaba vivo todo el rato.

**Moraleja**: el instrumento acerto; la teoria era del que lo construyo. Cuando
un aviso dice "murio el dueno de X", la primera pregunta no es *por que murio*,
es **quien era el dueno**. Y la de fondo: los programas de ejemplo que se
arrancan solos dejan de ser ejemplos y pasan a ser **participantes** -- compiten
por los mismos recursos que lo de verdad. Se quitaron del arranque, y el kernel
adelgazo 37 KB.

---

## Ep. 22 -- El raton que se movia al hacer clic

**Sintoma**: *"muevo y no funciona, pero al hacer click cualquiera, se mueven"*.

**Culpable**: tres numeros del panel lo decian entero. `bot=0b01` fijo (nunca
cambiaba), `x=0` al mover, `y` derivando sola. El aparato **ignoro el
`SET_PROTOCOL(boot)`** y seguia mandando su informe de protocolo de informe,
que empieza por un **Report ID**. Todo corrido un byte: donde el driver leia el
desplazamiento en X caian los **botones**, y por eso pulsar movia el puntero.

`SET_PROTOCOL` se mandaba y **nadie miraba si habia servido**. Con
`GET_PROTOCOL` detras, el aparato lo confeso solo: `protocolo=0x1 (INFORME: el
aparato ignoro el BOOT)`.

**Moraleja**: a un dispositivo se le PREGUNTA en que estado quedo; no se supone
que obedecio. Un `set` sin su `get` es una carta enviada sin acuse de recibo -- y
en un bus donde el otro extremo tiene su propio firmware, eso es optimismo.

---

## Ep. 23 -- El `malloc` que solo descarrilaba al fallar

**Sintoma**: ninguno. Esa es la gracia. `KIND_MEMORIA` se cableo de punta a
punta, compilo, paso el drift guard y se documento con su limite declarado --
*"un quinto `malloc` devuelve 0, que es lo que un programa de C ya sabe
comprobar"*. Y era mentira.

Lo destapo escribir el programa que la estrenaba. El emulador dijo:

```
opcode 0x05 no emitido por BMO
```

**Culpable**: el codegen de `malloc` emitia sus dos saltos con
desplazamientos **contados a mano**, y el primero se quedo seis bytes corto --
`jnz +0x1D` cuando el camino hasta el `xor rax, rax` mide 35. O sea que cuando
el kernel RECHAZABA la peticion, el salto caia dentro del `jnz` siguiente y el
CPU seguia leyendo a media instruccion. En el Ryzen eso no habria devuelto 0:
habria matado el proceso.

Lo que lo hacia invisible: **la rama buena estaba bien**. Un `malloc` que
funciona cuatro veces y descarrila a la quinta pasa por correcto en cualquier
prueba que no llegue a la quinta -- y ninguna llegaba, porque el emulador
tampoco modelaba la peticion y todo `malloc` devolvia 0 en silencio. Dos
agujeros tapandose el uno al otro.

**Moraleja**: contar bytes a mano es escribir un enlazador en la cabeza cada
vez que alguien mete una instruccion en medio. Las etiquetas ya estaban en el
codegen; solo habia que usarlas. Y la de fondo: **una rama de error que nadie
ejecuta no esta escrita, esta redactada.** El limite de cuatro peticiones
existia en la documentacion y en el kernel; el camino de vuelta al programa,
no.

---

## Ep. 24 -- Ocho bytes de log para una pregunta que contesta el aparato

**Sintoma**: `raton x=-4332` -- un desplazamiento que ninguna mano hace.

Despues de que el raton confesara `protocolo=0x1` (Ep. 22) quedo abierto si
sus ejes eran de 8 o de 16 bits. Si eran de 16, el byte que el driver leia
como `dy` era la mitad alta de `dx`: mover en horizontal moveria en vertical.
El plan era registrar **ocho bytes crudos** del informe y decidir mirando la
foto: si los bytes 4..7 traen datos, son 16 bits.

**El plan estaba mal**, y no por el instrumento. Un formato no se decide
mirando datos: se pregunta. Todo HID lleva su **Report Descriptor**, que dice
literalmente que bit es cada campo y de cuantos bits -- y este driver nunca se
lo habia pedido a nadie porque el protocolo BOOT le ahorraba el parser. En
cuanto un aparato ignoro el BOOT, ese ahorro paso a ser el problema.

Se le pide (`GET_DESCRIPTOR`, tipo 0x22) y se lee. El parser saca cuatro
campos --botones, X, Y, rueda-- con su posicion en bits y su tamano, respetando
lo que de verdad cuesta hacer bien: `Report Size`/`Report Count` en bits, el
desplazamiento acumulado **por Report ID**, el relleno (`Input (Cnst)`) que
ocupa sitio y no significa nada, y el reparto de usages por lista o por rango.

**Moraleja**: es el Ep. 22 otra vez, un nivel mas arriba. Alli la leccion fue
*a un dispositivo se le pregunta en que estado quedo*; aqui es **a un
dispositivo se le pregunta que formato habla**. Los ocho bytes crudos se
quedan en el log, pero ya no para adivinar: para comprobar que lo que el
descriptor promete es lo que el aparato manda.

---

## Ep. 25 -- El write-combining, otra vez, y por el lado que nadie miro (sin foto todavia)

**Sintoma**, dicho por el dueno: *"que no me salgan ghosting"* -- un rastro que
sigue al puntero.

El Ep. 20 dejo cerrado que **la pantalla** no ve nuestras escrituras sin
`sfence`. Lo que nadie se pregunto es lo simetrico: **las vemos nosotros?**

El compositor lee el framebuffer en **un solo sitio** de todo el programa: el
*save-under* del cursor, que guarda los 160 pixeles de debajo para devolverlos
al moverse. Y lo hace **al final del fotograma, justo antes del unico
`sfence`**:

```text
  1. quitar        -> escribe (al bufer WC)
  2. pintar todo   -> escribe (al bufer WC)
  3. poner: LEER   <- ve la pantalla de HACE UN FOTOGRAMA
  4. vaciar        -> sfence: ahora si llega todo
```

**Culpable**: una lectura de memoria WC no esta ordenada contra las escrituras
pendientes en el bufer. Asi que el paso 3 guardaba pixeles **caducados**, y el
`quitar` de la vuelta siguiente los devolvia **encima de lo nuevo**. Un
rectangulo de 10x16 con contenido viejo persiguiendo al raton: eso es
exactamente el ghosting.

El comentario de `Pantalla::leer` lo decia sin saberlo -- *"el framebuffer es
memoria de este proceso, asi que se puede leer"*. Era cierto cuando se escribio.
Dejo de serlo el dia que esa memoria paso a WC, dos dias antes, y **nadie
reviso a los lectores** porque el cambio se penso como una optimizacion de
escritura.

Y de paso salio un segundo: `pintar_calc` es **el unico pintado del bucle que no
dispara la entrada** --lo dispara el hijo al contestar--, asi que puede caer en un
fotograma con el cursor todavia puesto. Pintar ahi caduca el guardado igual.

**Moraleja**: cambiar el tipo de memoria de una region no es un cambio local, es
un cambio de **contrato**, y hay que ir a buscar a todos los que lo usaban con
el contrato viejo -- incluidos los que solo leen. La pregunta que lo habria
cazado en el minuto uno es de una linea: *quien LEE esto?*. En este programa la
respuesta cabia en un `grep` y daba un solo resultado.

---

## Ep. 26 -- El escritor y el lector miraban extremos opuestos del mismo buffer

**Sintoma**, dicho por quien lo sufria: *"el `ls` ya ejecute normal pero no
muestra nada"*.

Y era literal: el comando corria, la linea de estado ponia `listo`, y la rejilla
de salida se quedaba en blanco. Ni un error, ni un cuelgue. La forma mas
incomoda de fallo -- la que se parece a "no hace nada" y en realidad es **"lo
hace donde nadie mira"**.

**Culpable**, en dos lineas que estan a 220 de distancia en el mismo archivo:

```rust
Salida::nueva()  ->  fila: 0                          // el ESCRITOR empieza arriba
pintar_salida()  ->  base = SAL_HIST - SAL_ROWS       // el LECTOR ensena abajo
```

`SAL_HIST` son 200 filas y `SAL_ROWS` son 16, asi que la ventana visible es
`celdas[184..200]` y el primer texto se escribia en `celdas[0]`. **Las 184
primeras lineas de cualquier programa eran invisibles.** `ls` escupe una docena.

Lo trajo el historial con scroll (`8ee091e2`): antes la rejilla eran 16 filas y
escribir desde la 0 era exactamente lo correcto. Ese commit convirtio la rejilla
en una **ventana sobre 200 filas** y movio al lector al final del buffer -- y
dejo al escritor donde siempre habia estado. Nadie miro al otro extremo porque
el que se estaba tocando funcionaba.

**Y por que no lo cazo nadie antes**: el arreglo del scroll traia su prueba
escrita --*"llenar la salida con `ls`, subir con PgUp"*-- y esa prueba nunca se
ejecuto en metal. Estuvo meses en la lista de pendientes de hardware.

**Moraleja**: cuando un cambio mueve un **extremo** de una estructura
compartida, hay exactamente dos sitios que revisar, y el segundo es el que no se
esta tocando. Un buffer con escritor y lector tiene dos contratos, no uno. Y el
corolario: **una prueba escrita y no ejecutada no protege de nada** -- es la
misma ley 13, otra vez, sobre otro codigo.

*Nota de metodo*: esto se encontro **leyendo**, no adivinando, y solo porque la
foto traia el dato que discriminaba (`listo` pintado + rejilla vacia = el
comando corrio y la salida se perdio). Sin esa distincion, la teoria facil era
"el `ls` falla" y se habria buscado en el driver de directorio.

---

## Ep. 27 -- El teclado que "se desconectaba solo" (arreglo escrito, sin foto todavia)
**Sintoma**: contado de memoria por el dueno -- *"mi teclado al presionar se puso
como que se desconecta sin sentido"*. Deja de responder **sin que nadie lo
toque**, y sigue enchufado.

**Culpable**: un endpoint USB **Halted** y ninguna forma de levantarlo. Un error
de transaccion del bus --ruido, un paquete mal, un cable regular-- para el
endpoint; a partir de ahi `rearmar()` encola y toca el timbre para nada, porque
**el xHC ignora el doorbell de un endpoint Halted**. El aparato sigue enumerado,
sigue teniendo anillo, y no vuelve jamas.

Lo que hacia el fallo invisible: el driver **sabia verlo** --`ep_state` documenta
desde hace tiempo que 2=Halted significa muerto-- y solo lo miraba. Los dos
comandos que resucitan un endpoint, **Reset Endpoint (14) y Set TR Dequeue
(16), no estaban escritos**. Y el teclado, a diferencia del raton, **no tenia
rama de error**: `if cc == 1 || cc == 13 { ... }` y a rearmar. El unico aparato
que fallaba en silencio absoluto era justo del que se sospechaba.

**Moraleja**: *saber diagnosticar no es saber curar.* Un driver que sabe leer el
estado de averia y no tiene el comando que lo deshace esta a medio escribir, y
la mitad que falta no se nota hasta que el hardware falla de verdad -- que es el
peor momento para descubrirla. El corolario practico: **el paso que se olvida es
el segundo.** Resetear sin recolocar el puntero de la cola deja el endpoint
listo para leer TRBs viejos con el ciclo cambiado; el reset "no sirve de nada" y
parece que el problema era otro.

*Sin foto todavia*: hay que **provocar** el fallo para verlo. La senal buena es
`[uhid] teclado: transferencia con error cc=` seguido de `[xhci] endpoint
RESUCITADO`, y que el teclado siga escribiendo despues.

---

## Ep. 28 -- El SMP que llevaba meses escrito y no podia funcionar
**Sintoma**: `smp_startup()` existia en `s1_cpu` desde hacia tiempo, con
trampolin, INIT+SIPI y GDT. **Nadie lo llamaba.** La lectura facil era "esta
hecho y falta enchufarlo".

**Culpable**: no estaba hecho. Leido de cerca, tenia cuatro fallos que lo hacian
imposible, y el primero es el que ensena algo:

1. **El trampolin estaba ensamblado como codigo de 64 bits** --`mov rax, ...`,
   `retfq`-- para un nucleo que arranca en **modo real de 16 bits**. Ahi un
   prefijo REX no existe: `0x48` es `dec ax`. Ejecutaba basura desde la primera
   instruccion, y ninguna cantidad de llamarlo lo habria arreglado.
2. Las tablas de paginas se pisaban entre si: la PML4 en `0x7000` ocupa 4 KiB y
   el PDPT se ponia en `0x7100`, dentro.
3. El contador de nucleos vivos estaba en `0x7FF8`, **dentro de esa misma PML4**
   que el paso anterior ponia a cero.
4. La GDT no tenia segmento de datos de 32 bits: cargaba `0x18` creyendo que lo
   era, y en esa tabla `0x18` era el codigo de 64 bits.

Y un quinto que no era de codigo sino de sitio: vivia **antes de
`ExitBootServices`**, donde los otros nucleos todavia son del firmware (UEFI los
tiene en su MP Services) y la memoria baja tampoco es nuestra.

**Como se arreglo**: reescrito en el kernel, despues de EBS, con `.code16` de
verdad, los saltos lejanos emitidos byte a byte (`66 EA imm32 imm16`) y **usando
el `CR3` del kernel** en vez de construir tablas nuevas -- una tabla menos que
pueda quedarse desincronizada. Se comprobo sacando los bytes del ELF ya
enlazado: `fa - 31 c0 - 8e d8 - 66 0f 01 16`, cero bytes `0x48`.

**Resultado**: `nucleos en pie: 12 de 12`, a la primera, en el Ryzen.

**Moraleja**: *codigo escrito no es codigo que funcione, y "esta hecho, solo
falta llamarlo" es una hipotesis, no un hecho.* Lo que decidio el diagnostico
fue **leerlo entero antes de ejecutarlo** -- y en un trampolin de modo real eso
importa el doble, porque ahi no hay quien te cuente lo que paso: un fallo son
doce nucleos que no contestan y ni una linea de log.

*Nota de metodo*: la comprobacion que valio no fue compilar, fue **mirar los
bytes emitidos**. Un `.code16` mal puesto compila perfectamente.

---

## Ep. 29 -- Un `& 0xFF` de diferencia entre un programa y un secuestro
**Sintoma**: `run c/ray.bex` pintaba cielo y suelo, sin una sola pared, y no
respondia a nada -- ni a su propio ESC. La maquina quedaba de rehen, y el
diagnostico del dia anterior fue *"no consiguio la entrada"*. Era falso.

**Culpable**: `INPUT_OP_TECLA` no contesta el caracter, contesta `0x100 | byte`
-- el `0x100` significa "SI hay tecla", y hace falta porque el byte 0 tambien es
una respuesta valida. El ejemplo comparaba el valor entero, asi que
`tecla == 27` comparaba **283 contra 27**, que no es cierto jamas. **El programa
leia el teclado perfectamente y descartaba todo lo que leia.**
`bmo::Entrada::tecla()`, en Rust, ya lo separaba bien; el ejemplo en C se lo
comia entero.

Y las paredes eran otros dos, cualquiera de ellos suficiente: salir del bucle
del rayo con `t = 20 * UNO` en vez de `break` **borra la distancia**, que es lo
unico que el bucle habia averiguado; y `fdiv(alto, t) >> 16` sobra el
desplazamiento, porque `alto` son pixeles pelados y `fdiv` ya devuelve enteros.

**Como se diagnostico, y esto es lo nuevo**: sin encender la maquina. Se
reprodujo la aritmetica 16.16 del programa en el anfitrion y se dibujo el
fotograma; salio **identico a la foto** -- franja de cielo, franja de suelo, y
las barritas de ayuda al pie. Una foto borrosa de un monitor se convirtio en una
prueba reproducible.

**Moraleja**: *un diagnostico que no explica TODOS los sintomas no es el
diagnostico.* "No consiguio la entrada" explicaba que no saliera, pero no que no
hubiera paredes; dos fallos distintos se estaban leyendo como uno. Y el segundo
corolario: **si puedes simular la aritmetica, la foto deja de ser la unica
prueba.**

## Ep. 30 -- Un acento que se manifestaba como medio megabyte
**Sintoma**: escribir un comentario en `raycaster_C.c` **rompia el compilador**,
con un error en una linea que no tenia nada malo, cuatro mas abajo.

**Culpable**: dos, y los dos por la misma causa raiz -- el estandar de C borra
los comentarios en la **fase 3**, antes de mirar una directiva, y BMO C no lo
hacia.

1. `#define UNO 65536 /* 1.0 en 16.16 */` guardaba el comentario **dentro del
   cuerpo**. Como la expansion se aplica tambien dentro de los comentarios,
   nombrar esa macro en un comentario inyectaba un `*/` que lo cerraba antes de
   tiempo y convertia el resto del parrafo en codigo.
2. Buscandolo aparecio el gordo: `b[i] as char` lee cada byte como Latin-1, asi
   que los DOS bytes UTF-8 de una `n` con tilde salian como dos caracteres que
   al recodificarse ocupan CUATRO. Y el bucle repite mientras algo cambie, hasta
   16 veces: **2^16**. Un `hola mundo` con una sola letra acentuada daba un
   `.bex` de **492.032 bytes** -- ahora 512 -- con 65.536 bytes de basura donde
   iba la letra.

**Moraleja**: *un fallo de codificacion no se presenta como un fallo de
codificacion.* Con `MAX_BEX` en 1 MiB, dos palabras con tilde dejan un programa
que no carga -- y el sintoma es "el binario es enorme", que es el ultimo sitio
donde uno busca un acento. Por eso las fuentes de BMO-X son ASCII: no por
estetica, porque el sistema tiene DOS codificaciones y no se hablan.

## Ep. 31 -- La garantia que se comprobaba a si misma
**Sintoma**: la herramienta que paso 423 ficheros a ASCII prometia no tocar nada
fuera de los comentarios, y lo comprobaba: quitaba los comentarios del antes y
del despues y exigia que los dos resultados fueran identicos. Cero ficheros
rechazados. Todo verde.

**Culpable**: **se comprobaba con el mismo tokenizador que hacia el cambio.**
Eso demuestra "solo toque lo que YO llamo comentario", no que mi idea de
comentario sea correcta. Y no lo era: `'"'` --un literal de caracter cuyo
contenido es una comilla, como en `trim_matches('"')`-- se leia como lifetime, y
la comilla siguiente abria una cadena falsa que se tragaba nueve lineas de
comentarios.

Arreglado el tokenizador, la re-auditoria de los 379 ficheros contra HEAD ya
tenia un juez independiente: **4 divergencias, y las cuatro eran cambios
escritos a mano a proposito.**

El mismo escaner causo el segundo: corta un tramo de codigo en cada `/ " ' r b`,
asi que **parte los identificadores** (`nombre` llega como `nom`+`b`+`re`).
Renombrar por tramos no casaba casi nada, y renombro las referencias entre
backticks de los comentarios **dejando las funciones sin tocar**. Compilaba
--nada renombrado sigue siendo coherente-- y la documentacion apuntaba a nombres
que no existian.

**Moraleja**: *una prueba que usa el mismo modelo que el codigo que prueba no
prueba nada.* Es el patron del Ep. 26 con otra ropa: el escritor y el lector
mirando extremos opuestos del mismo buffer, aqui el verificador y el
transformador compartiendo el error. **El juez tiene que ser otro.**

Y de aqui salio lo que hace que esto no sea un parche: `build.ps1` comprueba la
codificacion **en el mismo sitio donde comprueba el contrato de syscalls**. Sin
eso, la regla era una limpieza que hicimos una vez.

---

## Ep. 32 -- El `#if` que no fallaba: contestaba mal
**Sintoma**: ninguno. Ese es el episodio.

Al medir BMO C contra los 81 ficheros de DOOM aparecieron cinco causas, cuatro
de ellas ruidosas -- el compilador se paraba y decia algo. La quinta no decia
nada.

**Culpable**: el evaluador de `#if` buscaba **el primer operador de una lista
fija, en cualquier posicion de la cadena**, y partia ahi. Con `a == b && c`
encuentra `==` antes que `&&`, asi que calculaba `a == (b && c)`.

Eso no da error. **Da una respuesta**, y lo que esa respuesta decide es que
mitad del fichero existe. Un preprocesador que elige la rama equivocada produce
un programa que compila limpio, pasa los tests que se le pongan, y **no es el
programa que se escribio**. No hay linea que mirar, porque la linea que sobra ya
no esta en el texto.

Al lado de eso, `#if (0 == 0)` --que no sabia evaluar por los parentesis-- era
el sintoma amable: se para y avisa.

**Y una segunda, encontrada por el test de otra cosa.** Escribiendo la fila que
comprueba que un `//` dentro de una cadena NO es un comentario, salto que
`sin_comentarios` cortaba en el primer `//` estuviera donde estuviera:
`#define PATH "http://x/y"` se guardaba como `"http:`. El mensaje que salia era
*"'PATH' no esta declarado (...) si venia de un #define, la cabecera no llego a
expandirse"* -- o sea, te manda a revisar los includes. La macro expandia
perfectamente. Se partia **al guardarla**, tres pasos antes.

**Moraleja**: *un fallo que se para es un regalo; el que contesta es el caro.*
Y el orden en que se buscan importa -- las cuatro ruidosas se veian en la
primera pasada de la sonda, y la silenciosa solo aparecio al leer el codigo que
las cuatro tenian al lado. Por eso ahora es un parser con precedencia de verdad:
no porque fuera mas elegante, sino porque el modo de fallo de la version vieja
**no tiene sintoma**.

## Ep. 33 -- La tecla que no existia, y las tres cosas que colgaban de ella
**Sintoma**: `Ctrl+Alt+ESC` --el rescate escrito el mismo dia, el que le quita la
pantalla a un programa que no la suelta-- **no hizo nada** en el Ryzen.

**Primera hipotesis, y era razonable**: en la distribucion espanola `Ctrl+Alt`
ES `AltGr`. El propio codigo lo avisa: *"un atajo que dispare al PULSAR Ctrl+Alt
rompe escribir `@`, `#`, `[`, `]`"*. Asi que parecia que el atajo se comia el
tercer nivel del teclado.

**Culpable**: nada de eso. **El scancode 0x01 no estaba en NINGUNA tabla.** Ni
en la comun, ni en `nav_key`, ni en las tres distribuciones. `resolve`
contestaba `Out::Nothing`, o sea que **el byte 27 no se producia jamas en todo
el sistema**. La traduccion USB ya entregaba el scancode correcto; lo que
faltaba era el ultimo salto, una fila de tabla.

Y encima de una tecla que no llegaba habia **tres cosas** escritas:

1. `ESC cierra`, en el pie de todas las ventanas del escritorio.
2. `if (tecla == 27) vivo = 0;` en el raycaster -- **su unica salida**.
3. El rescate, que empieza por `let b = t?`: sin byte sale por el `?` y **no
   llega ni a mirar los modificadores**.

Las tres se leian como fallos distintos y eran uno. Y explica la forma exacta
del sintoma que conto el dueno: *"al raycaster pude entrar a jugar y no pude
salir"*.

**De propina, el patron de siempre**: hay DOS tablas de teclado en el arbol, y
`platform/drivers/usb/input/keyboard.rs` SI tiene `0x01 => 0x1B`. **La que sabia
no era la que decodifica.**

**Moraleja**: *cuando tres cosas fallan a la vez, no son tres fallos.* Y el
sitio donde buscar no es donde se nota, es donde nace el dato -- aqui, tres
capas por debajo del atajo.

## Ep. 34 -- Los dos que NO fallaban, y por eso costaron
**Sintoma**: ninguno. Los dos compilan, corren y contestan.

Salieron llevando BMO C contra los 81 ficheros de DOOM, y ninguno lo encontro
una foto: los encontro **ejecutar el programa y comparar la salida**.

1. **`p->x++` se ignoraba en silencio.** El brazo del postfijo era `_ => {}`: si
   el operando no era un nombre suelto, el `++` **se consumia y no se emitia
   nada**. `s->count++` compilaba, corria, y el contador no se movia. Ni error,
   ni aviso. Aparecio yendo a arreglar el PREfijo, que si daba error.

2. **`*p` sobre un `int*` sacado de una tabla leia OCHO bytes.** Salio
   `85899345930` donde tocaba un `10`. Es `(20 << 32) | 10`: devolvia el entero
   pedido **y el de al lado en la mitad alta**. La funcion que da el ancho no
   sabia mirar dentro de `tabla[i]` cuando el elemento es un puntero, y caia en
   el caso por defecto, que lee ocho.

**Y debajo del segundo habia un tercero**: al escalar un indice por doce, el
compilador emite `imul rax, rax, imm8`. **El emulador no tenia ese opcode** --
lo llevaba emitiendo desde siempre para cualquier paso que no fuera potencia de
dos, y ningun test lo habia ejecutado nunca. El emulador hizo lo correcto: dio
panic con el opcode en la mano en vez de seguir con un valor inventado, y ese
panic es el que destapo lo de arriba.

**Moraleja**: *un fallo que se para es un regalo; el que contesta es el caro.*
Y la regla que los caza no es mirar el binario -- es que cada fila del banco
EJECUTE. Un `.bex` con los bytes correctos y un indice mal escalado se ven
identicos en un volcado.

## Ep. 35 -- La operacion que casi suelta la pantalla al leer un informe
**Sintoma**: ninguno todavia, y ese es el episodio.

Al anadir la AUTOPSIA --el informe que el kernel redacta cuando mata una tarea--
se le dieron los opcodes `0x1D` y `0x1E`. **Ya eran `PANTALLA_SOLTAR` y
`ENTRADA_SOLTAR`.**

O sea: **leer el informe de un fallo habria soltado la pantalla.**

**Y el fichero ya avisaba.** El comentario de `PANTALLA_SOLTAR` cuenta, con
nombre y fecha, que `MEMORIA_PEDIR` se puso en `0x12` --ya ocupado por
`REINICIAR`-- y que pedir memoria habria reiniciado la maquina. La regla estaba
escrita: *"elegido tras listar los opcodes ORDENADOS"*.

**Culpable**: que esa regla es **prosa**. Un comentario no para un build. Lo
unico que separaba al proyecto de repetir el mismo fallo, dos meses despues, era
que alguien se acordara de leer un parrafo.

**Arreglo**: `build.ps1` saca ahora TODOS los opcodes del kernel y falla si
alguno se repite. No contra una lista escrita a mano -- una lista a mano es lo
que ya se quedo congelada una vez en ese mismo guion, treinta lineas mas arriba.

```
    operaciones: 32 opcodes, ninguno repetido
```

**Moraleja**: *una regla que solo vive en un comentario no es una regla, es un
recordatorio.* Y la prueba de que hacia falta automatizarla es que la escribio
la misma persona que despues la incumplio.

## Las leyes que dejo esta guerra

1. **QEMU miente por omision**: sin IRQs vivos, sin tiempos fisicos, sin
   memoria con huecos. Todo lo que "funciona en QEMU" es una hipotesis.
2. **Los bugs viejos disfrazan a los nuevos**: el CS fantasma (Ep. 4)
   causaba el split-brain (Ep. 5) que tapaba el fb invisible (Ep. 6). Se
   pelan como cebolla, en orden, con una foto por capa.
3. **La telemetria en pantalla vale mas que mil teorias**: cada episodio
   cayo cuando el sistema mismo confeso (filas de diagnostico, censos,
   heartbeats). Si no puedes verlo, no puedes matarlo.
4. **Un instrumento que mata tu hipotesis vale mas que uno que la
   confirma** (Ep. 14). Las cinco sondas de XSAVE tumbaron cuatro teorias
   antes de acertar. Cada "no era eso" recorto el espacio de busqueda a la
   mitad; una sonda que solo hubiera dicho "si" no habria recortado nada.
5. **El informe de fallo ya sabe mas de lo que se lee.** `err=0x00000008` no
   era un numero: era el selector culpable, dicho por el CPU (Ep. 15). Antes
   de anadir un campo nuevo, leer entero el que ya esta.
6. **Una regla escrita para un caso concreto no protege del siguiente**
   (Ep. 15). "El framebuffer necesita CR3 de kernel" era cierto y era
   inutil: la regla de verdad era *cualquier direccion del rango identidad
   tocada desde un syscall*, y estaba a un periferico de distancia.
7. **Arreglar un bug despierta a los que dormian debajo** (Ep. 17 -> 18). El
   `break` de mas tapaba un anillo de eventos mal repartido desde el primer
   dia; quitarlo no rompio nada nuevo, **destapo** lo que llevaba meses
   escrito y nunca ejercido. Un arreglo que hace aparecer un fallo peor suele
   ser el arreglo correcto.
8. **Verde no es cableado** (Ep. 19). Un modulo puede compilar, pasar todos
   sus tests, aparecer en el commit y no ser consultado por nadie. Los tests
   prueban la politica; no prueban que alguien la obedezca. La comprobacion
   dura dos segundos: buscar quien LLAMA a la funcion que contesta.

9. **Un aviso correcto no implica una teoria correcta** (Ep. 21). "Murio el
   dueno de la pantalla" era cierto tres arranques seguidos, y la conclusion
   que se saco era falsa. Antes de preguntar *por que paso*, preguntar **a
   quien le paso**.
10. **Una optimizacion que cambia CUANDO se ve algo no esta terminada**
   (Ep. 20) hasta que alguien decide cuando tiene que verse. El
   write-combining sin `sfence` no era rapido: era incorrecto.
11. **A un dispositivo se le pregunta, no se le supone** (Ep. 22 y 24). Un
   `set` sin su `get` es una carta sin acuse de recibo, y al otro lado hay un
   firmware con sus propias ideas. La version fuerte: tampoco se le supone el
   **formato** -- el Report Descriptor esta ahi para eso, y adivinarlo mirando
   bytes crudos es leerlo en la variable equivocada.
12. **Cambiar el tipo de memoria de una region es cambiar un CONTRATO**
   (Ep. 25), no hacer una optimizacion local. Hay que ir a buscar a todos los
   que la usaban con el contrato viejo -- **y los lectores cuentan**. El WC se
   penso como un cambio de escritura y rompio la unica lectura que habia.
13. **Un buffer compartido tiene DOS contratos, no uno** (Ep. 26). Cuando un
   cambio mueve un extremo --donde empieza a leer, donde empieza a escribir--, el
   sitio que hay que revisar es **el que no estas tocando**. El escritor
   empezaba arriba y el lector ensenaba abajo, y las dos lineas eran correctas
   por separado.
14. **Una rama de error que nadie ejecuta no esta escrita, esta redactada**
   (Ep. 23). El camino bueno de `malloc` funcionaba y el de fallo saltaba a
   media instruccion; el limite existia en el kernel y en la documentacion, y
   el programa nunca llegaba a verlo. Escribir el programa que ejerce el
   limite es parte de implementar el limite.
15. **Saber diagnosticar no es saber curar** (Ep. 27). Un driver que sabe leer
   el estado de averia y no tiene el comando que lo deshace esta a medio
   escribir. Version general, la que salio del barrido de las 57 agujas: **un
   fallo o se maneja o se GRITA con su numero, nunca se descarta callando** -- y
   lo que hay que cazar no es el `panic`, es el fallo que se convierte en un
   valor con pinta de buen dato: un `unwrap_or(0)` donde 0 es una direccion
   fisica, un cluster libre, un pid con dueno o un indice de cadena. No
   revientan: **mienten**, y el sintoma sale despues y lejos.
16. **"Esta hecho, solo falta llamarlo" es una hipotesis** (Ep. 28). El SMP
   llevaba meses escrito y tenia cuatro fallos que lo hacian imposible, el
   primero de ellos codigo de 64 bits para un nucleo que arranca en 16. La
   comprobacion que valio no fue compilar --un `.code16` mal puesto compila
   perfectamente-- sino **mirar los bytes emitidos**. Donde no hay quien te
   cuente lo que paso, se lee antes de ejecutar.
17. **Un diagnostico que no explica TODOS los sintomas no es el diagnostico**
   (Ep. 29). "No consiguio la entrada" explicaba que el raycaster no pudiera
   salir, pero no que no tuviera paredes. Eran dos fallos leyendose como uno, y
   el primero tapo al segundo durante un dia entero.
18. **Un fallo de codificacion no se presenta como un fallo de codificacion**
   (Ep. 30). Se presento como un binario de medio megabyte. El sistema tiene
   dos codificaciones --fuentes en UTF-8, consola en Latin-1-- y no se hablan;
   por eso las fuentes son ASCII, y por eso lo comprueba el build y no la buena
   voluntad.
19. **Una prueba que usa el mismo modelo que el codigo que prueba no prueba
   nada** (Ep. 31). El verificador y el transformador compartian tokenizador,
   asi que la garantia decia "solo toque lo que yo llamo comentario" y no "mi
   idea de comentario es correcta". El juez tiene que ser otro. Es el Ep. 26
   otra vez, con otra ropa.
20. **Lo que no comprueba el build, no es una regla: es una costumbre.** Las
   doce cadenas que imprimian mojibake se arreglaron a mano; nada impedia la
   trece. Una limpieza es un parche hasta que hay un portico que la exige --
   por eso el idioma de las fuentes se valida en el mismo sitio que el contrato
   de syscalls, y no en un documento.

21. **Cuando tres cosas fallan a la vez, no son tres fallos** (Ep. 33). El
   `ESC cierra` del escritorio, la salida del raycaster y el rescate del teclado
   se leian como tres carencias distintas, y eran una fila de tabla que nadie
   escribio. El sitio donde buscar no es donde se nota: es donde nace el dato.
22. **Un fallo que se para es un regalo; el que contesta es el caro** (Ep. 34).
   `p->x++` no incrementaba y `*p` leia ocho bytes en vez de cuatro. Ninguno da
   error: los dos dan un numero. Por eso cada fila del banco EJECUTA el
   programa -- un binario con los bytes correctos y un indice mal escalado se
   ven identicos en un volcado.
23. **Una regla que solo vive en un comentario no es una regla, es un
   recordatorio** (Ep. 35). El fichero avisaba, con nombre y fecha, de que
   elegir un opcode ya usado habia reiniciado la maquina una vez. Dos meses
   despues se volvio a elegir uno ocupado, y **la prueba de que hacia falta
   automatizarlo es que lo incumplio quien lo habia escrito**. Es la ley 20 otra
   vez, y que se repita es el argumento.

## Ep. 36 -- El `#elif` que entraba tambien, y DOOM compilando

**Sintoma**: `no existe la funcion 'swapeLE16'`. Un nombre que **no esta escrito
en ningun sitio de DOOM** salvo dentro de un `#ifdef SYS_BIG_ENDIAN` que en una
maquina x86-64 no se recorre jamas.

**Primera hipotesis, y era razonable**: que el evaluador de `#if` estuviera
contestando mal otra vez -- es el Ep. 32, y el sitio es el mismo. La sonda
minima lo desmintio: `#if (0)` / `#elif (1)` daba la rama correcta.

**Culpable**: el estado de un grupo `#if / #elif / #else` era **un solo bit** --
"esta rama esta activa"--, asi que `#elif` miraba la rama de justo antes y no si
alguna ya habia entrado. Con las dos condiciones ciertas, **las dos ramas se
compilaban**.

Y las dos son ciertas mas a menudo de lo que parece, porque C manda (C11
6.10.1p4) que un identificador desconocido en un `#if` valga 0. Asi que
`#if (A == B)` con las dos sin definir es **cierto**. Eso es `i_swap.h`:

```c
#if   ( __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__ )
#define SYS_LITTLE_ENDIAN
#elif ( __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__ )
#define SYS_BIG_ENDIAN
#endif
```

Quedaban definidos **los dos**. No falla ruidosamente: un `#define` repetido se
pisa y gana el ultimo, o sea que **la configuracion que queda puesta es la que el
programa habia descartado**. El sintoma solo aparecio porque la rama muerta
llamaba a una funcion que no existe; si hubiera llamado a una que si existe, el
programa habria corrido con el endianness al reves y nadie lo habria sabido.

★ **Es el Ep. 32 con otra ropa, y por eso vale contarlo dos veces**: aquel
partia la expresion por el operador equivocado, este olvidaba el estado del
grupo. Los dos modos de fallo son el mismo -- **el preprocesador no se para: te
entrega otro programa.**

**Y lo que habia detras**: era el ultimo desconocido entre BMO C y DOOM. Con eso,
mas el formateador de ejecucion, el `va_list` como puntero y `double` como
parametro, **las 56.465 lineas del nucleo de DOOM compilan a un `.bex` de
1.299.512 bytes**. Con `MAX_BEX` en 1 MiB no cabia por 248.936 -- y ahi la
decision fue del dueno y queda escrita: *"DOOM es el MAS optimizado, asi que
vamos a respetar y ejecutar SEGUN lo que exige"*. El tope subio a 4 MiB.

**Moraleja**: *lo que un programa ajeno mide es una medida del sistema, no un
capricho del programa.* Un tope que un programa de 1993 no cabe es un tope mal
elegido -- y el `.bin` del kernel no crecio ni un byte al subirlo, porque eso es
`.bss` y el cargador ya reservaba diecisiete veces mas.

24. **El preprocesador no se para: te entrega otro programa** (Ep. 36, y el
   Ep. 32 antes). Dos fallos distintos --partir la expresion por el operador
   equivocado, y olvidar que una rama del grupo ya entro-- con el mismo modo:
   ninguno da error, los dos deciden que mitad del fichero existe. Cuando un
   componente no puede fallar ruidosamente, sus filas de prueba no son un lujo:
   son el unico sintoma que va a haber.
25. **Lo que un programa ajeno mide es una medida del sistema** (Ep. 36). Un
   `MAX_BEX` que DOOM no cabe es un tope mal elegido, no un DOOM demasiado
   grande. El tope se puso mirando los binarios propios, que es exactamente la
   forma de elegir un numero que solo vale mientras nadie traiga nada de fuera.

*Debuggeado a fotos de pantalla, entre un humano con hardware y una IA sin
ojos. 2026.*
