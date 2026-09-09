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

24. **El HARDWARE se PERFILA; el SOFTWARE es AGNOSTICO. Y no es simetria: es
   que no son el mismo problema.** (Enunciada por el dueno el 2026-08-23, al
   cazar una estimacion que la incumplia.)

   ```text
      hardware   -> PERFIL      se nombra EXACTO, y estrenar otro es
                                cambiar una tabla, nunca editar el nucleo
      software   -> CONTRATO    no nombra a nadie, y por eso vale para todos
   ```

   **El motivo es de QUIEN MANDA sobre la cosa.** El hardware es un hecho que no
   controlas: no puedes hacer que un Realtek se porte como un Intel, asi que
   genericidad ahi significa una cadena de `if` que tiene que acertar con
   tarjetas que **no tienes y no puedes probar**. Eso no es codigo flexible: es
   **codigo no falsable**. El software lo defines tu -- dos syscalls congelados,
   las palabras en una columna de TOML-- y ahi la genericidad no cuesta nada
   porque el contrato lo pones tu.

   *** Y el corolario que lo hace util: **un perfil se puede PREDECIR y un
   driver generico no.** El perfil dice *"esta tarjeta, esta MAC, este
   registro"*, y entonces se puede escribir la respuesta antes de mirar y
   comparar -- que es el metodo de las cinco sondas (ley 4). Un driver que dice
   *"cualquier AMD"* no tiene experimento que lo confirme ni que lo refute.
   **El perfil es lo que hace que el metal se pueda probar.**

   La evidencia estaba repartida en cuatro sitios y ninguno la nombraba:

   | donde | que demuestra |
   |---|---|
   | los 287 borrados de `net/e1000` | era la NIC **de QEMU**, no la del Ryzen: no habria encendido un LED |
   | `cpu_vendor/profile.rs` | tres sitios llamaban a `ryzen_5_5600x` **por su nombre** -- el contrato roto, y reparado |
   | `tests/agnostico.rs` | el frontend tiene **prohibido nombrar una maquina**, hasta en la prosa |
   | `rdna4: pci_devices: &[]` | el perfil **se niega a reclamar una tarjeta que no ha conocido** |

   [!] **Y el bus NO es el aparato.** Enumerar PCIe es generico --es una
   especificacion-- y los registros de un RTL8168 son un hecho sobre un chip.
   Por eso "mapear los BAR" ya esta hecho y sirve para todo, y "arrancar el PSP
   de un Navi 44" no se hereda de nadie.

   *** La consecuencia que costo la leccion: **una estimacion generica es una
   estimacion de OTRO PROYECTO.** Decir "meses" sobre el driver de GPU era
   ponerle precio a `amdgpu` --cuatro millones de lineas para quince anos de
   tarjetas-- cuando lo que aqui se escribe es un perfil de UNA. Y encima
   incumplia la ley 11 dos veces: **se le supuso a un aparato al que no se le
   habia preguntado.**

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

---

## Ep. 37 -- El icono salio, DOOM no, y el escritorio se cayo al retroceder

**2026-08-09, 23:30.** Tres cosas en una tanda de fotos, y las tres dicen algo
distinto. Se cuentan juntas porque la unica forma de entender la tercera es
haber leido las dos primeras.

### 1. Lo que SI salio, y no es poco

Foto uno: el escritorio con **un icono y la palabra `doom` debajo**. Eso es la
cadena entera funcionando a la primera en metal:

```
   Directorio::open("apps")  ->  filtrar .bex  ->  abrir el .bex
   -> leer su cabecera BEF   ->  encontrar la seccion Resources
   -> leer el indice BRES    ->  sacar el recurso "icono"
   -> descifrar BICO         ->  pintarlo
```

Siete pasos, dos formatos leidos a mano, y ninguno fallo. **El icono vive dentro
de la app** y el escritorio lo saca de ahi: no hay `.lnk`, no hay cache, no hay
fichero de escritorio que se quede huerfano.

[!] Con un asterisco honesto: **el icono salio BLANCO** y deberia ser una cara
roja. La silueta es la correcta --el recorte transparente esta bien-- asi que lo
que llego mal es el color, no el dibujo. Queda abierto, y las dos ramas son
distinguibles a ojo: si fuera el icono por defecto seria un cuadro macizo con
una `D`, y no lo es.

### 2. DOOM no paso la admision

```
   83 WARN proc:   el .bex de disco no paso la admision =4
   84 WARN lanzar: el .bex no paso la admision =3
```

`doom.bex` mide 814.616 B y `MAX_BEX` son 4 MiB, asi que **no es el tope**. Su
tabla de secciones se volco fuera y pasa entera las comprobaciones de
`bex::inspect`: seis secciones, `file_size <= mem_size` en todas, la `Bss` con
`file_size = 0` que es justo lo que esa comprobacion permite, el `entry` dentro
del codigo, y los codigos de seccion del kernel coinciden con los del ABI.

Lo que queda como sospechoso principal, y se dice como sospecha y no como
diagnostico: **es el `.bex` mas grande que se ha intentado cargar nunca**. El
anterior era `gui.bex` con 306 KiB; este es 2,7 veces mayor. En la misma tanda
de fotos aparece `lanzar: el archivo no cabe en el buffer`, que es el mensaje de
cuando `fs::load` no puede traerlo -- y una lectura corta produce exactamente
este sintoma: la tabla apunta mas alla de lo leido y la seccion se declara
invalida.

★ **El siguiente paso no es tocar codigo, es MEDIR**: que diga cuantos bytes
trajo del disco frente a cuantos mide el fichero. Mientras esos dos numeros no
esten en la pantalla, cualquier arreglo es una apuesta.

### 3. El panico, que es el bueno

```
   89 WARN gui: panico en el compositor
   90 WARN gui: range end index 18446744073709551615 out of range for slice
   91 WARN gui: en services\gui\src\main.rs:2834
```

`18446744073709551615` es `usize::MAX`. La linea 2834 es
`pintar_campo(..., &ruta[..n], ...)`. O sea: **`n` se desbordo por abajo**.

El retroceso hacia esto:

```rust
    if cur > 0 {          // <- la condicion de UN contador
        ...
        cur -= 1;
        n -= 1;           // <- la resta del OTRO
    }
```

Guardado por `cur`, restando de `n`. Mientras `cur <= n` --el invariante de
cualquier campo de texto-- los dos son ciertos a la vez y el fallo no existe.
**Lleva ahi desde que existe la caja, y hoy no falto casi nada para que siguiera
sin existir.**

Lo que rompio el invariante fue el camino nuevo del lanzador, y la cadena
completa es esta:

```
   clic en el icono   ->  n = cur = 17   ("run apps/doom.bex")
   Enter inyectado    ->  se presta la pantalla y se lanza
   DOOM no admite     ->  n = 0   ...y cur se queda en 17
   un retroceso       ->  cur > 0 es cierto, n -= 1 da la vuelta
   el repintado       ->  &ruta[..usize::MAX]  ->  panico
```

★★ **Y aqui esta lo que hay que llevarse.** Los tres eslabones son inofensivos
por separado: el clic pone dos contadores, un lanzamiento fallido limpia uno, y
el retroceso mira el que no toca. Ninguno de los tres es un fallo mirandolo
solo. **El fallo es la frase entera**, y solo la escribe un camino nuevo pasando
por codigo viejo.

Arreglado en los dos sitios, y el segundo importa mas que el primero:

- La guarda pasa a ser `cur > 0 && n > 0`, que es lo que la resta pide.
- **`cur = cur.min(n)` una vez por vuelta del bucle.** No se fue a perseguir
  cada `n = 0` del fichero: se restaura el invariante en un solo sitio, antes
  de que nadie pueda teclear. Cuesta una comparacion por fotograma y quita la
  clase entera de fallo -- incluido el proximo camino que se olvide de `cur`.

**Moraleja**: *un invariante que nadie restaura es un comentario.* Este vivia en
la cabeza de quien escribio la caja, se cumplia por costumbre, y aguanto hasta
el dia en que otra persona --con otro camino, meses despues-- lo rompio sin
saber que existia. Un invariante barato se **impone**, no se recuerda.

26. **Un invariante que nadie restaura es un comentario** (Ep. 37). `cur <= n`
   se cumplia por costumbre en todos los caminos que habia, y el primero que
   llego despues lo rompio. Cuando restaurarlo cuesta una comparacion, se
   restaura en un sitio y se acaba la clase de fallo; documentarlo solo protege
   a quien lea el documento.
27. **Un fallo puede no estar en ningun eslabon, sino en la frase** (Ep. 37).
   Poner dos contadores, limpiar uno, y mirar el otro: los tres son correctos
   por separado y juntos tiran el escritorio. Revisar el diff de un camino
   nuevo no basta -- hay que leer la frase que forma con el codigo viejo por el
   que pasa.

---

## Ep. 38 -- El volumen llega al audifono y la nota se va al zumbador que no existe

**2026-08-10, 12:10.** Vivaldi corrio en el Ryzen. `VIVALDI: 48 notas, dos
veces`, salida guardada, el programa termino limpio. Y **no se oyo nada**.

### El diagnostico estaba escrito antes de que pasara

Las lineas del kernel lo cuentan enteras:

```
   info uaudio  el aparato guardo OTRO volumen =35
   info uaudio  audifono USB con volumen =1
   info audio   sonido cedido a Ring 3 =4
```

`=35` es **el eco piano de Vivaldi**, y el audifono USB lo GUARDO. O sea que la
mitad del volumen funciona de punta a punta: capability -> kernel -> descriptor
de audio -> control transfer -> el aparato de verdad.

Lo que no funciona es la otra mitad, y las dos operaciones no hablan con el
mismo aparato:

| | va a |
|---|---|
| `AUDIO_OP_VOLUME` | el altavoz del PC **y** el audifono USB |
| `AUDIO_OP_BEEP` | el altavoz del PC, **y solo el** |

En esta placa --MSI A320M PRO MAX-- el cabezal SPKR no trae zumbador. Asi que
la pieza esta **poniendole el volumen al audifono y mandando las notas a un
altavoz que no existe**.

★ Y lo mejor: `ring0/obj/audio.rs` ya lo decia, en un comentario escrito antes
de que nadie lo intentara -- *"en esta maquina el altavoz del PC no suena, asi
que esta linea es la unica de las dos que se puede OIR"*. La foto no descubrio
el fallo: **confirmo una prediccion que estaba en el codigo**, que es la mejor
clase de sorpresa que puede dar una tanda de fotos.

### Lo que falta, dicho con su tamano

Eddi lo vio antes de que se lo contaran: *"seria como teclado y mouse pero con
audio"*. Exacto, y esa comparacion mide bien el trabajo -- porque **lo caro del
teclado y el raton ya esta pagado**:

```
   xHCI                        HECHO
   enumerar el aparato         HECHO
   leer sus descriptores       HECHO
   control transfers           HECHO  (el volumen sale por ahi)
   transferencias ISOCRONAS    FALTA  <- esto es todo lo que queda
```

`platform/drivers/usb/uaudio` ya lo dice en su primera linea: *"reproducir
muestras por USB pide transferencias isocronas"*. `bmo-xhci` tiene
`queue_interrupt_in` y no tiene su equivalente isocrono de salida.

★ **Y por eso el camino corto NO es HD Audio.** El plan de DOOM tiene el audio
como fase 5 con un driver de HDA entero por delante --enumerar el codec, abrir
un stream, un anillo de buffers con DMA--. Por USB queda **una** pieza, y el
aparato ya esta enumerado y respondiendo. La fase 5 estaba mirando al sitio
equivocado.

**Moraleja**: *dos operaciones de la misma capability pueden hablar con dos
aparatos distintos, y el programa no tiene forma de saberlo.* `bmo_sonido_volumen`
y `bmo_sonido_pitar` se piden al mismo handle y una llega al audifono mientras
la otra se pierde. Cuando una capability agrupa aparatos, **decir a cual fue
cada operacion es parte de la respuesta** -- si no, el programa no puede
distinguir "se oyo bajito" de "no habia donde oirlo".

### Y en la misma tanda, dos que SI

- **El compositor no se rompio al lanzar DOOM.** El panico del Ep. 37 --`n`
  desbordado por abajo tras un lanzamiento fallido-- no volvio. El invariante
  `cur <= n` restaurado una vez por vuelta aguanto justo el caso que lo tumbo.
- **FAT32 lee ficheros grandes en metal**: `archivo abierto para leer =814664`
  --el tamano exacto de `doom.bex`-- y `=4196020`, el WAD entero. La sospecha
  de la lectura corta queda descartada tambien en el Ryzen, no solo en el
  anfitrion.

28. **Dos operaciones de la misma capability pueden ir a dos aparatos
   distintos** (Ep. 38). Y el programa no tiene forma de saberlo: pide las dos
   al mismo handle. Una capability que agrupa aparatos tiene que decir **a cual
   fue** cada operacion, o quien la usa no puede distinguir "no se oyo" de "no
   habia donde".
29. **Una prediccion escrita en un comentario vale mas que el comentario**
   (Ep. 38). El de `audio.rs` decia que en esta placa solo el USB se puede oir,
   meses antes de que nadie lo intentara. Cuando el metal confirma una nota asi,
   lo que hay que revisar no es el codigo: es el PLAN, que estaba mirando a HD
   Audio teniendo el camino corto por USB.

---

# Ep. 39 -- 2026-08-10/11: EL DESTINO IMPORTA

## El arranque, y el numero que lo dice todo

```text
   44 FAULT proc:   cabecera invalida (magic, version o 0 secciones) =800
   45 WARN  proc:   el .bex de disco no paso la admision =1
   46 INFO  lanzar: bytes DIRECTOS del disco al marco =2C00
   47 WARN  gui:    el .bex no paso la admision
```

`=800` son **2048**. Ese numero es `bytes.len()`, o sea **el prologo** -- no los
308.184 de `gui.bex`. Falla la lectura de los **primeros 2 KB** del fichero, que
es la operacion mas pequena que hace el sistema.

Y la linea 46 solo existe en el camino sin mesa: **la pieza B se tomo**, y sus
11.264 bytes directos (el paseo por directorios, la FAT y los cuatro sectores del
prologo) demuestran que las lecturas ocurren.

## La observacion que cierra el caso

En el arranque anterior el fallo salia con `=4B3D8` --el fichero entero-- y en
este con `=800`. Entre las dos fotos cambio **una sola cosa** en ese camino:

```text
   antes    el prologo caia en IMAGE   -- estatico de .bss, alineado a pagina
   ahora    el prologo cae en la PILA  -- un local de 2 KB
```

Mismo fichero, mismo LBA, mismo codigo de lectura.

> **Si una lectura funciona o no segun DONDE pongas el destino, el sospechoso no
> es el disco ni el sistema de ficheros: es la traduccion de direccion.**

Y de paso descarta al otro sospechoso: la mesa compartida **ya no esta en el
camino** y el fallo sigue. No era la mesa.

## Lo que estaba mal, dicho en una frase

`tramo_dma` **preguntaba** a las tablas de pagina donde vive un buffer, para
darle esa respuesta al HBA. Preguntar admite que te contesten mal.

El physmap es un espejo LINEAL --`virt = phys + HIGH_MEM_BASE`-- asi que para una
direccion de esa ventana la fisica **no hay que preguntarla: se resta**. Y dos
direcciones seguidas ahi son dos fisicas seguidas por construccion, asi que la
continuidad tampoco hay que comprobarla.

Ahora el DMA directo **solo** acepta destinos del physmap. Todo lo demas --la
pila, `.bss`, la imagen del kernel-- rebota: mas lento, correcto, y son lecturas
pequenas.

** Y el camino rapido no se pierde donde importa: la pieza B aterriza las
secciones en marcos recien pedidos al asignador, y a esos se llega por
`phys_to_virt`, que **es** una direccion del physmap. El DMA directo se queda
exactamente en el sitio para el que se invento.

## La leccion, que es de metodo

1. **Un error que no puede ensenar su evidencia es un error a medias.** Decir
   *"cabecera invalida"* es una afirmacion; su prueba son ocho bytes. Sin ellos
   hubo dos tandas de fotos y una tarde entre cuatro hipotesis **que se
   distinguen a simple vista**. Ahora la falta los imprime.
2. **Mover una pieza es un experimento.** El prologo se movio a la pila por
   limpieza --para que el camino sin mesa no tocara la mesa-- y sin querer se
   monto la prueba controlada que llevaba dos dias haciendo falta: mismo dato,
   mismo camino, distinto destino.
3. **Preguntar es peor que saber.** Cada vez que el kernel *deduce* algo que
   podria tener escrito, se abre un sitio donde la deduccion falla en silencio:
   `bex::necesita` deduciendo lo que el fichero podia declarar, `tramo_dma`
   deduciendo una traduccion que el mapeo ya garantiza. Las dos se arreglaron
   igual -- **quitando la pregunta**, no mejorandola.

## Lo que sigue sin saberse

Que la traduccion era el problema es **la hipotesis mejor sostenida**, no un
hecho: el arreglo es correcto lo fuera o no --una resta es mejor que un paseo por
tablas para un espejo lineal-- pero quien lo confirma es el proximo arranque. Si
la cabecera sigue invalida, los ocho bytes diran cual de las otras tres es.

---

# Ep. 40 -- 2026-08-13: EL DIA QUE NO SE FLASHEO Y CAYERON DOS BUGS IGUAL

## Por que este episodio esta en esta bitacora

Los treinta y nueve anteriores empiezan igual: **una foto de la pantalla**. Este
no tiene ninguna. No se flasheo la maquina, no se arranco el Ryzen, y aun asi
cayeron dos defectos del compilador que llevaban meses ahi -- uno de ellos desde
el primer dia.

Eso lo convierte en un episodio de metodo y no de metal, y por eso se escribe:
**el cambio de metodo es el hallazgo.**

## El metodo, en una frase

Perseguir bugs de uno en uno con arranques no escala: cuatro vueltas de
flasheo-ver-morir-cazar por tres defectos, el 13 por la manana. Los tres eran la
misma FORMA --un contenedor, una operacion, y un brazo del codegen que no cubria
esa casilla-- y una forma **se puede enumerar**.

Nueve sondas, **143 casillas, medio segundo**:

```text
   language     28 casillas   verde al escribirla
   layout       12            9 ROJAS  -> el alineado
   width        16            limpio
   tables       11            limpio
   signedness   16            4 ROJAS  -> shr/div/setb
   control flow 16            limpio
   assignment   16            1 ROJA, abierta a proposito
   strings      16            limpio (tras anadir `<strings.h>`)
   heap         12            limpio
```

## Lo que cayo, y ninguno se habria visto arrancando

**1. El alineado se deducia del TAMANO del miembro.** Falso para todo lo que no
sea un escalar: un array se alinea como su ELEMENTO. `char name[8]` mide ocho
bytes igual que un `long` y se alinea a UNO. Consecuencia, con los numeros del
formato WAD al lado:

```text
   maptexture_t.patches    offset 24    el disco dice 22
   maplinedef_t entero     16 bytes     el disco dice 14
   mapsidedef_t entero     40 bytes     el disco dice 30
   mapnode_t entero        32 bytes     el disco dice 28
```

Los dos que son el TAMANO son los peores: `p_setup.c` recorre el lump como un
array, asi que **el primer registro del nivel sale bien y todos los demas
corridos**. Ese sintoma no se parece a un fallo de disposicion; se parece a un
nivel roto.

**2. Las cuatro operaciones sin signo emitian la version con signo.** `>>` daba
`sar` donde toca `shr`; `/` y `%` daban `cqo`+`idiv` donde toca `xor rdx,rdx`+
`div`; `<`/`>` daban `setl`/`setg` donde toca `setb`/`seta`.

[!] **En 32 bits acertaban por casualidad**, y ahi esta lo que hay que aprender:
el codegen calcula en `rax`, o sea en 64 bits, y un `unsigned int` llega
**extendido con ceros** -- el bit 63 vale 0 y `sar` da lo mismo que `shr`. Solo
un `unsigned long` lo destapa. El defecto no lo escondia un test que faltara: lo
escondia que **el caso que lo rompe no se puede escribir en el tipo pequeno**.

## La moraleja, y son tres

**Los dos ejes que dieron rojo tenian AUTORIDAD EXTERNA.** El formato del WAD
lleva fijo desde 1993 y la aritmetica de `angle_t` la define C. Los siete que
salieron limpios se comparaban contra lo que nosotros creiamos. Es el patron 15
otra vez: una prueba que escribe el mismo lado que el codigo comprueba lo que se
le ocurrio. **Buscar una autoridad de fuera contra la que medir vale mas que
escribir mas casos.**

**Un fallo confesado en prosa sigue siendo un fallo, y van tres.** El brazo de
`Shr` decia por escrito *"un tipo sin signo querria `shr`; hoy el codegen no
arrastra esa distincion hasta aqui"*. Era falso: la distincion llegaba, y
`expr_is_unsigned` se escribio copiando la funcion de al lado. Cuando un
comentario diga "hoy X no sabe Y", comprobar si de verdad no sabe.

**Y un censo que no encuentra nada tambien es un resultado.** Cuatro ejes
limpios son la respuesta a *"por donde empiezo"* la proxima vez que algo falle en
metal, que es media busqueda ahorrada.

## Lo que sigue sin saberse

Hasta donde llega DOOM. Sigue siendo la unica pregunta que el anfitrion no puede
contestar, y ahora hay bastante mas razon que ayer para que llegue lejos.


---

## Ep. 39 -- La orden que existia donde su dueno no puede entrar
**Sintoma**: *"escribi el ext y no conoce LOL"*. La orden `ext` --el censo de
extensiones del CPU-- estaba compilada, flasheada y contestando. Y no aparecia.

**Culpable**, y son dos capas. La primera: `ext` se cablo SOLO en el shell de
Ring 0, y **a ese shell no se vuelve**. `obj/fb.rs::rescue()` se niega a
proposito a quitarle la pantalla al escritorio --*"no se echa al que sostiene
la casa"*--, asi que `Ctrl+Alt+Esc` rescata de DOOM o del raycaster pero nunca
del compositor. El dueno vive en el escritorio; la orden vivia en el otro lado
de una puerta que no se abre.

La segunda, debajo: `ext` estaba en el despachador **y en ninguna de las dos
listas** -- ni en `help` ni en `ORDENES`. O sea que existia sin poder
descubrirse. El comentario de `ORDENES` avisa literalmente del riesgo de
mantener dos listas a la vez, y ya habian derivado: faltaban tambien `consumo`,
`gasto`, `w`, `apps` y `programas`.

**Moraleja**: **una funcion que no se anuncia no es discreta: no esta.** Es el
patron 33 girado -- alli era un mensaje que salia por un canal que nadie mira;
aqui es codigo correcto en un sitio al que no se llega. Antes de dar algo por
hecho, preguntar *donde va a estar la persona que lo necesita*.

Y el remate que lo prueba: el aviso de arranque del propio censo dice
*"escribe ext"*. Mandaba a teclear una orden en un sitio donde esa orden no
existia.

## Ep. 40 -- Mi aritmetica de ciclos, baja por ocho veces
**Sintoma**: ninguno. Este episodio es sobre una conclusion equivocada, no
sobre una maquina rota.

**Que paso**: `c/coste.bex` midio una puerta en 2.620 ciclos contra 20 de una
llamada. La pregunta siguiente era DONDE se van, y los sospechosos estaban
nombrados leyendo `entry.rs`: el `xsave64` con RFBM=-1, el `xrstor64` y el
`iretq`. Los sume "con honestidad" y me dieron **unos cientos de ciclos, no dos
mil**, asi que escribi que la sospecha no explicaba el numero.

Entonces se escribio el metro (`syscall/meter.rs`, dos `rdtsc` en `dispatch`) en
vez de operar. Resultado en el Ryzen:

```text
   puerta pelada  2663  =  dispatch 318 (12%)  +  stub 2345 (88%)
```

**Culpable**: yo. La sospecha era correcta; lo que fallaba era mi estimacion del
coste de las instrucciones, **baja por ~8x**.

**Moraleja**: **estimar ciclos a ojo no vale ni para descartar.** Lo unico que
salvo la decision fue no actuar sobre ninguna de las dos creencias --ni la
sospecha ni mi refutacion de la sospecha-- y construir el instrumento. Un metro
de dos `rdtsc` costo una tarde y contesto lo que dos razonamientos no pudieron.

**El regalo de propina**: la fila 4 dio el numero que nadie habia visto nunca --
resolver una capability cuesta **83 ciclos**, 76 dentro de `dispatch` y 7 en el
stub. O sea que **el modelo de capabilities, lo que hace especial a BMO-X, es el
3% del coste de una puerta. El otro 97% es fontaneria generica de x86.**

## Ep. 41 -- Un `true` costaba un cuarto del arranque
**Sintoma**: ninguno otra vez. Salio de preguntar *"se puede optimizar mas la
RAM?"* y medir en vez de opinar: `llvm-nm --size-sort -S` sobre el kernel.

**Culpable**:

```rust
static mut EP_RINGS: [[EpRing; 32]; 255] = [[EpRing {
    valid: false, ring_phys: 0, ring_virt: null_mut(), pcs: true, enqueue: 0
}; 32]; 255];
```

255 ranuras x 32 endpoints x 32 bytes = **261.120 bytes**, y todos los campos
eran cero **menos uno**. Un solo byte distinto de cero manda el array entero a
`.data` en vez de a `.bss`, y `.data` viaja DENTRO del binario. O sea que el
kernel guardaba 255 KiB de ceros en disco y los leia en cada arranque para
llevar un bit puesto en cada entrada.

Un bit que ademas **no lee nadie**: esa entrada es una ranura vacia,
`ep_ring_mut` filtra por `valid` antes de devolverla, y al registrar el endpoint
de verdad se reescribe la estructura entera.

```text
   bmo-kernel     1.024.680 -> 755.392 B   (-26,3%)
   BOOTX64.EFI    1.048.576 -> 779.264 B   (-25,7%)
```

**Moraleja**: **un inicializador no-cero en un array grande es un fichero mas
grande, y no se ve en el codigo.** La pregunta no es "cuanta memoria pide esto"
sino "cuanto de esto viaja". Y la herramienta que lo enseno --`llvm-nm
--size-sort`-- deberia usarse antes de discutir tamanos, no despues.

[!] Y hay que decir QUE mejora, porque es facil venderlo de mas: la RAM
reservada es la misma (`.bss` tambien ocupa). Lo que baja un cuarto es la imagen
que el firmware lee en cada arranque.

## Ep. 42 -- El traspaso del firmware que nunca ocurrio
**Sintoma**: *"al reiniciar mi BMO-X arranco normal MENOS el xHCI"*. **En frio
va; en caliente, tras un reinicio desde Windows, el teclado y el raton no
aparecen.**

**Culpable**: cuatro lineas que aparentaban pedirle el controlador al firmware.

```rust
let eecp = ((hcc1 >> 8) & 0xFF) as u32;
if eecp >= 0x40 && (r32(mmio + eecp) & 1) != 0 {
    w32(mmio + eecp + 4, 1);
    for _ in 0..50000 { if r32(mmio + eecp) & 1 == 0 { break; } }
}
```

El puntero a capacidades extendidas (xECP) vive en `HCCPARAMS1[31:16]` y cuenta
en **palabras de 32 bits**. Los bits 15:8 son `MaxPSASize`, `CFC`, `SEC`, `SPC`
y `PAE`. Asi que: el traspaso **nunca ocurrio**; si esos bits daban >= 0x40 se
escribia un 1 en un MMIO cualquiera de la region de capacidades (y si caia en
`USBLEGCTLSTS`, lo que ese 1 enciende es el **SMI de USB**, justo lo contrario
de lo que hace falta); y la espera miraba el campo ID esperando que llegara a
cero, cosa que no pasa nunca.

**Y debajo, el que convierte "raro" en "basura"**: las tres esperas del reset
eran `for _ in 0..N { if listo { break } }` **sin mirar por que salieron**. Un
bucle que acierta y uno que se agota acaban en la misma linea. El xHCI spec dice
que mientras `USBSTS.CNR` este puesto no se puede escribir ningun registro
operacional que no sea USBSTS -- y lo siguiente que hacia el codigo era escribir
`CONFIG`. En frio el reset acaba antes de agotar el bucle y no se nota; en
caliente el controlador tarda mas y se le escribe encima sin estar listo.

**Moraleja**: **una espera que no dice si acerto es un `if` que siempre da que
si.** Las tres ahora fallan con su nombre (`el controlador no PARA`, `el reset
no termina`, `sigue NO LISTO`). Y la otra: **en frio no es una prueba.** Un
driver de bus solo esta probado cuando se ha reiniciado en caliente desde otro
sistema operativo, que es el estado en que el aparato llega sucio.

[!] Diagnosticado leyendo el spec, **sin confirmar en metal todavia**. La prueba
es exactamente la que lo destapo: reiniciar desde Windows.

---

## Ep. 43 -- La calculadora que espero a un fantasma de quince dias
**Sintoma**: primer arranque con la calculadora terminada. La cara sale, las
teclas responden. Se teclea `5+8`, se pulsa `=` **y se queda clavada**: sin
resultado, sin error -- y el escritorio **deja de aceptar teclas**. No se puede
escribir ni `reboot`.

**Culpable**: no era la calculadora. Se lanzaba **otro motor**.

```
   staging\BMO-DATA\cobol\calcgui.bex     5.840 B    3 de agosto   <- este corria
   staging\BMO-DATA\cobol\2\calcgui.bex   3.768 B    ese dia       <- este se construia
```

Los ejemplos de COBOL se reorganizaron de `cobol\` a `cobol\<escalon>\`, y los
planos **se quedaron en `staging`**, que no se limpia entre builds. El
escritorio pedia `cobol/calcgui.bex` --sin escalon-- y esa ruta seguia
existiendo. Ese binario hablaba el protocolo anterior, de UNA sola linea; el
escritorio, estrenado ese mismo dia, esperaba DOS. Se quedo esperando una
segunda linea que aquel motor no sabia mandar.

★★ **Un fantasma no da error: CONTESTA.** Por eso no lo caza ninguna
compilacion, ningun test y ninguna comprobacion de formato. El fichero estaba
bien, el programa era correcto, la ruta resolvia. Sencillamente **no era el que
se creia estar ejecutando**.

**Y debajo, el que convierte "se quedo clavada" en "hay que apagar la
maquina"**: mientras esperaba, la calculadora se quedaba **toda** tecla sin una
sola excepcion. Es la regla que se habia escrito a proposito --*si tiene las
teclas, las tiene todas*-- y estaba bien... para una espera que termina. Con el
motor mudo, esa misma regla apago el teclado del sistema.

**Moraleja**: dos, y la segunda es la que se paga cara.

**Una espera no puede ser una carcel.** Da igual de quien sea el fallo que la
provoco: una app esperando no puede secuestrar la maquina. Ahora `C` la cancela
--la tecla que la mano busca sola-- y ademas se cierra sola si el hijo murio sin
contestar (`has_child` con el anillo vacio: exacto, y sin reloj, que aqui no
hay). El `Ctrl+n` que ya existia funcionaba, pero **un atajo que hay que
saberse no es una salida: es un secreto**.

**Y lo que el build no borra, el build lo esconde.** No limpiar `staging` es una
decision buena --rehacerlo entero cuesta minutos-- pero su precio no era basura
inerte: era un binario viejo TAPANDO al nuevo. El guardian nuevo compara por
fecha, que es una **resta** y no una lista: cualquier `.bex` mas viejo que el
arranque del build. En su primera ejecucion encontro otro (`apps\doom640.bex`).
Un guardian con lista tendria el mismo fallo que vigila.

[!] Y el episodio tiene una coda que vale mas que el episodio: **el mismo dia se
habia arreglado otro fallo mudo en el mismo camino** --`CONSOLE_READ` entregaba
paquetes de 7 bytes y el lector de lineas TIRABA lo que viniera detras del
`\n`--, y aun asi la calculadora no funciono en metal. Dos fallos distintos, en
la misma frase de codigo, encontrados con doce horas de diferencia. **El emulador
encontro el primero; solo el metal podia encontrar el segundo**, porque el
emulador ejecuta el `.cob` que se le da y nunca pregunta que binario hay en el
disco.

## Ep. 44 -- Dos calculadoras en la misma foto
**Sintoma**: la primera foto de la calculadora funcionando de verdad --`40.00`
en el visor, la cara de MAQUETA, el motor en COBOL-- sale con **DOS
calculadoras**: la vieja en su sitio y la nueva encima, las dos con el mismo
numero. La terminal se habia arrastrado.

**Culpable**: `run_relayout` movia **donde se va a pintar** y nadie se ocupaba
de los pixeles de **donde ya no esta**.

Lo mas util del episodio es que el fallo estaba **medio escrito en su propio
comentario**. Decia que sin esa funcion la calculadora se quedaria *"pintandose
en el vacio, y peor, respondiendo a clics en un sitio donde ya no hay nada"*.
Las dos mitades eran ciertas y faltaba la tercera: **se movio el sitio y se dejo
el dibujo**.

**Moraleja**: **mover algo son dos operaciones, no una.** Poner el sitio nuevo y
borrar el viejo. Un comentario que enumera dos consecuencias de tres suena
completo -- y la que falta es justo la que nadie busca, porque el codigo hace lo
que el comentario promete.

★ Y el arreglo va **en la funcion, no en sus tres llamadores**, por el mismo
motivo por el que la funcion existe: todo lo que se coloca a partir de la
terminal se entera en UN sitio. Repartirlo entre `shortcuts.rs` y las dos ramas
de `mouse.rs` es como se consigue que el cuarto llamador se olvide.

[!] El orden dentro es lo unico delicado y quedo escrito: se borra **despues**
de recolocar la terminal y **antes** de mover la calculadora, porque hace falta
el rect VIEJO de una con el fondo NUEVO de la otra. Al reves es el rastro que
`shortcuts.rs` dice haber cazado ya tres veces.

---

## Ep. 45 -- Faltaba una linea, y nunca se guardo un fichero
**Sintoma**: ninguno. `estratos` decia *"NO se hizo, el volumen sigue igual"* y
el motivo en F11 era `fuera de orden`. Nadie lo leyo porque **nadie lo ejecuto**:
el commit que estreno la escritura (`1c96b133`, 18-08) se cerro con "sin probar
en metal", y encima se le anadieron dos dias de funciones.

**Culpable**: la transaccion tiene cuatro fases --`Datos -> Barrera -> Commit ->
Cerrada`-- y el kernel llamaba `reserve` y despues `barrera_hecha` **saltandose
`cerrar_datos()`**, que es la que pasa de la primera a la segunda. La barrera
exigia estar en Barrera, contestaba `FueraDeOrden`, y el commit no ocurria
jamas. `sellar` si la llamaba, y por eso el 13-08 se vio `generacion 3` en el
Ryzen: aquel camino estaba entero y el de guardar un fichero no.

**Moraleja**: **probar cada pieza no es probar el camino.** La maquina de
estados estaba bien probada -- `una_transaccion_normal_recorre_las_cuatro_fases`
recorre la secuencia completa, `cerrar_datos` incluido, y pasa desde el primer
dia. Lo que no estaba probado era **quien la usa**, y ese hueco no lo cierra
otro test de anfitrion: solo lo cierra ejecutarlo una vez.

★ Y el diseno aguanto entero. Dijo que no, con su motivo, y la ventana lo
repitio en pantalla. No hubo silencio, ni medio arbol escrito, ni un superbloque
apuntando a bloques que no llegaron al plato. **El fallo fue de quien la usaba y
la maquina lo paro** -- que es exactamente para lo que estaba.

[!] La casilla de metal no es burocracia. Es el unico sitio donde se descubre
esto, y llevaba dos dias sin marcarse mientras encima se construia.

---

## Ep. 46 -- Todas mis versiones decian ser permanentes
**Sintoma**: tampoco ninguno, y este habria tardado meses en salir: el volumen
creceria para siempre y se notaria **el dia que se llenara**.

**Culpable**: `Estrato::con_nombre()` mira si el motivo esta puesto, y la section 9 dice
que **los estratos CON NOMBRE no los suelta el recolector jamas**. O sea que el
motivo no es una etiqueta descriptiva: es lo que hace PERMANENTE a una version.
Y yo escribia "fichero nuevo", "carpeta nueva", "entrada quitada" en todas.

**Moraleja**: **un campo que tiene consecuencias no es un campo de texto.** Se
llamaba `motivo` y se leia como una descripcion; lo que decide es si algo se
puede borrar. La prueba que lo decia ya existia desde el primer dia y se llamaba
`un_estrato_automatico_no_lleva_nombre` -- estaba escrito, y aun asi se piso.

★ El texto no se perdio: se fue a CABINA, que es donde tenia que estar desde el
principio. **Para contar lo que paso, no para marcar el disco.**

---

## Ep. 47 -- El techo eran dos techos con el mismo numero
**Sintoma**: un fichero de ESTRATOS media como mucho 96 bytes. Se levanto el
tope --`flujo` aprendio a partir el contenido en bloques con su arbol de
indireccion-- y **seguia sin poder pasar de 96**.

**Culpable**: `RESIDENTE_MAX` (lo que cabe dentro del nodo) y `DATOS_MAX` (el
renglon del syscall) valen los dos 96 y no tienen nada que ver. Se habia tumbado
uno y el otro ni se habia mirado.

**Moraleja**: **dos limites con el mismo valor se leen como uno.** El numero
coincidia por casualidad y eso basto para que nadie preguntara si eran el mismo.
Cuando un tope no cede al quitarle su causa, la causa era otra.

★ Y la salida no fue ensanchar el renglon. Meter 4 KiB de ocho en ocho serian
512 llamadas por bloque: esa puerta no esta hecha para eso. **El contenido dejo
de cruzar el anillo** -- `copia` manda dos NOMBRES y el kernel lee la fuente el
mismo, que ademas es lo que de verdad hacia falta: meter en ESTRATOS los
ficheros que ya estan en FAT32.

---

## Ep. 48 -- La tabla crecio y el segundo consumidor no
**Sintoma**: `main` con un test en rojo y nadie enterado. La matriz de
conformidad de BMO C decia `__maxsd no compila: registro de argumento
desconocido: xmm1`. Salio al contar los tests para una cifra del README, no al
trabajar en C.

**Culpable**: `intrinsics.toml` es una tabla **compartida**, y el 22-08 le
entraron tres filas nuevas --`sqrtsd`, `minsd`, `maxsd`-- porque las pidio INTI.
Hasta ese dia el unico flotante de la tabla era ninguno, asi que el emisor de
intrinsecos de C solo sabia volcar argumentos a registros ENTEROS: evaluar a
`rax`, `push`, y `pop` al destino. `xmm1` cayo en el `_ =>` que dice *"registro
desconocido"*, que es lo unico que hizo bien todo el episodio.

**Moraleja**: **una tabla compartida tiene mas de un lector, y crecer por uno la
rompe para el otro.** Es el precio de la decision que este proyecto ya tomo a
proposito --tablas y no cerebros-- y el precio se paga aqui: quien anade una
fila tiene que preguntarse quien mas la lee. Hoy son cinco frontends.

★ Y lo mejor del episodio es que **la matriz de conformidad existia y funciono**.
Se escribio con este motivo textual: *"el codegen valida el nombre de cada
registro al emitir, asi que una fila con `rex` en vez de `rax` no falla hasta que
alguien la usa -- y en una tabla de driver 'alguien la usa' puede ser dentro de
seis meses, en metal, buscando otra cosa"*. Cazo exactamente eso. Lo que fallo
no fue la red: fue que **nadie corrio el workspace**, y una maraton de un dia en
un lenguaje se cierra probando ese lenguaje.

★★ **Y el arreglo corto era el fallo.** Ensenarle `xmm0`/`xmm1` a
`emit_pop_to_reg` habria compilado y habria estado MAL: un `double` no vive en
`rax`, asi que evaluar el argumento por el camino entero lo trunca **antes** de
la instruccion y `__maxsd(1.25, 2.5)` devuelve `2.0`. Compila, pasa el gate, sale
firmado, y da otro numero. El camino bueno ya existia al lado --`emit_fbinop`,
que deja `a` en xmm0 y `b` en xmm1-- porque es literalmente lo que hace `addsd`
desde el primer dia.

[!] Y al conectarlo aparecio el tercero, que es el de siempre: `expr_is_float`
paso a decir que si de un intrinseco, `emit_fexpr` no tenia brazo para
`Expr::Intrinsic`, cayo en su `_ => "cualquier otra cosa es entera"`, que vuelve
a preguntar `expr_is_float`... **y la pila se desbordo antes de emitir un byte**.
El comodin no se equivocaba de respuesta: se equivocaba de pregunta. Cuarta vez.

★★★ Las dos pruebas nuevas **EJECUTAN**, que era lo que faltaba: `flotante.rs`
tenia trece tests de coma flotante y ninguno corria nada -- todos miraban bytes.
Los numeros estan elegidos para que la ruta equivocada se vea (`250 125 150`
contra `200 100 100`), porque un `2` en vez de un `2.5` no se nota y un `250`
contra un `200`, si.

---

## Y lo que quedo PREPARADO ese mismo dia, para cuando vengan mas

Esta bitacora es de episodios, no de planes -- asi que aqui solo va **donde
mirar**, que es lo que hace falta dentro de seis meses:

```
   META-APP_HARD.md               que exige BMO-X de algo que quiera ser una app,
                                  y que le devuelve. Hermano de META-KERNEL_HARD.
   docs/plan/PLAN_DIRECTOR.md     paso 2c: la entrada. La UNICA casilla que
                                  separa la calculadora de ser `calculadora.bex`.
   docs/maestro/IPC_MAESTRO.md    por donde se hablan dos programas, y por que
                                  JSON no era la pregunta.
```

★★ **La frase que resume los tres**: hoy una app de BMO-X **puede ensenar, y no
la puedes tocar**. Cuando eso cambie, no cambia para la calculadora: cambia para
todas las que vengan detras.

---

## Ep. 49 -- El medidor que se veia igual vivo que muerto
**Sintoma**: *"los FPS dependen de un teclado que no tiene sentido: tengo que
pulsar el bloq numerico SOLO para ver 1 frame que cambia"*.

**Culpable**: el suelo de repintado del compositor eran **DOCE MIL VUELTAS DE
BUCLE** (`BLINK`), y ese bucle no tiene freno. Sin tecla, sin raton y sin app
naciendo, el fotograma no pintaba. **La unica fuente de tiempo del compositor
era el teclado.** Se cambio por `Tick::quarter`, medido con el TSC.

**Y entonces empezo el episodio de verdad.** Para saber si el bucle giraba se
puso un numero en la barra --`loops_per_second`, que existia desde hacia meses y
**solo se pintaba dentro de las ventanas de CPU y memoria, que se abren con una
tecla**. El unico numero que dice si el escritorio esta vivo estaba detras de la
cosa cuya muerte habia que diagnosticar.

Se puso en la barra. El dueno lo probo y trajo: *"veo pulso una sola vez y se
congela"*. **Y no era la maquina: era el instrumento.** `loops_per_second` se
calcula UNA vez por segundo, asi que entre dos calculos el numero es constante y
la caja no se repintaba. Correcto, y absolutamente inutil:

```text
   el bucle VIVO y el numero quieto      se ve igual
   el bucle MUERTO                       se ve igual
```

Se le puso una AGUJA que gira con cada cuarto de segundo. Segunda vuelta al
metal y segunda mentira: **`pulso 0/s`**. El cero no era un ritmo bajo -- era
una medida que **nadie tomo**, porque sin reloj de referencia `Tick::pulse` sale
por su rama de emergencia antes de calcularlo. Y ese cero apuntaba al bucle, que
es justo donde NO estaba el fallo.

**Moraleja**: **un medidor cuyo estado sano se ve identico a su estado roto no
mide: es un adorno con cifras.** Y las tres preguntas que hay que hacerle a un
instrumento antes de escribirlo:

```text
   1. se llega a el SIN la cosa que puede estar rota?
   2. su estado sano se ve DISTINTO del roto en todo momento?
   3. cuando NO tiene la respuesta, lo dice, o da un valor por defecto?
```

★ Un cero es una medida. Si esa medida no se tomo, pintarla es inventarla -- con
la cara de un dato bueno. Mismo linaje que el `unwrap_or(0)` del Ep. 45.

★★ Y por eso `scene/pulso` acabo siendo **carpeta con carriles**: sus DOS averias
fueron de significado y ninguna de dibujo, y mientras las dos cosas vivian en la
misma funcion, un cambio de donde cae un numero y un cambio de que DICE ese
numero se leian igual en el diff.

## Ep. 50 -- El syscall que llevaba desde siempre sin estrenar
**Sintoma**: el dueno, mirando la arquitectura: *"tengo 2 syscalls, INVOKE y
WAIT, pero WAIT casi no se usaba. Creo que es momento de darle su oportunidad."*

**Culpable**: tenia razon, y era literal. `WAIT` se usaba en **UN** sitio de todo
el repo --`dormir_un_rato`, con esperable `0`, o sea un `sleep`-- y
`latido_esperar` no lo llamaba **nadie**. La pieza S3 del suelo de Ring 3 se
construyo entera, se documento, se le puso envoltorio de userland y no se
estreno.

> Un sistema con dos puertas donde una solo sabe dormir un plazo no tiene dos
> puertas: tiene una y media.

Se monto el bucle del compositor en el LATIDO. **Y fallo en el metal**, con un
numero que no dejaba duda: `latido 12937/s  cuerpo 1066  puerta 29`. Trece mil
vueltas pidiendo dormir mil.

```text
   latido::claim   cap::grant(..., RIGHT_WAIT, ...)   SOLO ese derecho, y su
                   comentario decia "aqui no se lee nada, se espera"
   latido_cuenta   es un INVOKE -> resuelve con RIGHT_READ -> FALLA siempre
   .unwrap_or(0)   se traga el fallo -> `visto` = 0 PARA SIEMPRE
   WAIT            `current != observed` (0) -> vuelve EN EL ACTO
```

★ El comentario de `claim` era **falso en su propio fichero**: doce lineas mas
abajo, `operation()` contesta a `LATIDO_OP_CUENTA`. O sea que el brazo
`KIND_LATIDO` de `invoke` era **codigo inalcanzable desde el dia uno** y
`latido_cuenta` contestaba `None` a todo el mundo. No se vio leyendo: se vio en
el metal.

★★ **Y lo caro no fue no dormir: fue dejar de CEDER.** `Tick::ceder` habia
sustituido a un `yield_screen()` incondicional, asi que el escritorio se quedo el
nucleo entero y el teclado y el raton del dueno **parecieron ignorados**.

**Moraleja**: **una optimizacion que se apaga sola tiene que apagarse hacia el
LADO SEGURO.** Esta se apagaba hacia el peor que habia. La invariante que faltaba
--y que ahora esta-- es que el bucle no puede acabar una vuelta sin soltar el
turno, haga `WAIT` lo que haga.

[!] Y el arreglo tuvo el mismo agujero que el fallo: deducia "durmio o no"
comparando lo que devuelve `WAIT`, y `WAIT` **miente cuando falla** (un error
trae `value = 0`, que con el testigo en 0 se confunde con "durmio"). Ahora lo
juzga el reloj. **Preguntarle al mecanismo por su propio estado es preguntarle al
sospechoso.**

## Ep. 51 -- EL FANTASMA: un quantum regalado a quien ya dormia
**Sintoma**: con `WAIT` durmiendo por fin, el escritorio **empeoro**:
`latido 9/s  pinta 3  cuerpo 1  puerta 33750`. Un milisegundo de trabajo y
**treinta y tres segundos esperando turno**. El dueno lo vio como *"1 frame cada
10 o 20 segundos"* y pregunto lo que habia que preguntar: *"mi kernel esta como
borracho? Encuentras un fantasma?"*.

**Y lo mas incomodo del episodio**: el escritorio empeoro **al hacer lo
correcto**. Mientras giraba se peleaba por el CPU y arrancaba 12.937 vueltas; en
cuanto se puso a dormir como debe, desaparecio.

**Culpable**, y estaba escrito en tres sitios del propio kernel:

```text
   park_until    llama a `mark_wait`, que marca Blocked y NO reprograma -- no
                 puede: el cambio de contexto se consuma en el epilogo del
                 trap, y esto no es un trap. Se queda haciendo `hlt`.
   on_timer      le daba el resto de su quantum a `s.current` SIN mirar si
                 seguia `Running`  -> hasta 4 ms de CPU HALTADA por parada
   y no hay uno, hay DOS parkers: el hilo del bus late 250 veces por segundo
   choose_next   prioridad ESTRICTA, y su propio comentario lo avisaba:
                 "un orden estricto EXCLUYE"
```

★★★ **La aritmetica que lo nombra**: 250 aparcadas por segundo x hasta 4 ms
retenidos = **hasta 1.000 ms de cada segundo**. El hilo del bus podia retener el
nucleo entero, haltado, sin hacer nada -- y a prioridad 2 nadie se lo quitaba. En
el idioma del tiempo real: `C/T = 100 %`, y con eso todo lo de abajo no es lento,
es **inplanificable**. Es un teorema, no una opinion.

**El arreglo es una condicion, no un mecanismo nuevo**: si la tarea actual ya no
esta `Running`, su quantum no es suyo. **Un quantum es de quien CORRE.** Medido
en el Ryzen el mismo dia:

```text
   antes:   latido 9/s        cuerpo 1     puerta 33750
   despues: latido 79026/s    pinta 4      cuerpo 700    puerta 283
```

De nueve a **setenta y nueve mil**. Y con DOOM lanzado cinco veces, matando
Ring 3 entre medias, sin romper el sistema.

**Moraleja**: **el giro era el disfraz.** El escritorio nunca tuvo una parte
justa -- tenia la parte del que gira, que arrebata cada hueco. Cada capa de giro
que se quito hizo el fantasma mas audible, en proporcion exacta:

```text
   giraban los dos            50/s        20 ms en volver
   el shell se durmio         12937/s     (WAIT roto: seguia girando)
   el escritorio se durmio    4/s         300 ms
                              9/s         3.700 ms
```

★ **El numero no empeoro: el disfraz se fue adelgazando.** Y la PRIMERA lectura
ya lo gritaba --un `yield_screen()` tardando 20 ms en volver en una maquina
ociosa-- y se leyo culpando al shell de Ring 0. El shell era *una* capa; el suelo
era el fantasma.

[!] Y queda dicho lo que NO se arreglo, que es la mitad que falta: `park_until`
suelta el CPU en el tic siguiente y no en el acto, o sea hasta **500 ms de cada
segundo** entre los dos parkers. Y con la maquina ociosa `schedule_locked` no
cambia de tarea --nadie mas esta listo-- asi que `WAIT` vuelve sin dormir y el
compositor gira a 79.000. Funciona y gasta un nucleo. La tecnica que falta tiene
nombre --**reschedule forzado por interrupcion software**-- y vive en
[`PLAN_EL_PLAZO.md`](docs/plan/PLAN_EL_PLAZO.md), escalon P2.2.

---

## Ep. 52 -- El cuello de botella era una PALABRA, y estaba escrita 155 veces

**2026-09-09.** El dueno pidio romper el cuello de botella del camino `Directo`.
La primera respuesta fue descartar cinco sospechosos leyendo --el fusionador de
cajas, `read()`, `sincronizar_lectura`, `paint_background`, `Table::compose`-- y
calcular que un fotograma moviendo el raton mueve ~135 KB, o sea 0,4 ms. No
cuarenta.

★ **Y lo primero fue una correccion mia**: la foto de `cuerpo 795` que yo mismo
habia citado como *"39,7 ms por fotograma"* es **anterior a E1**. En ese arranque
`cuerpo` era reloj de pared y no habia tarea idle, asi que ese numero incluye el
tiempo que el compositor paso EXPULSADO del CPU. Cite reloj de pared como si
fuera CPU -- exactamente el fallo contra el que yo habia escrito la advertencia
dentro de `cuerpo_ms` dos mensajes antes. Los 39,7 ms no valen.

### Lo que si estaba medido, y donde estaba de verdad el cuello

```text
   volcar la pantalla entera   8,3 MB a ~300 MB/s  =  27,6 ms
   el presupuesto a 60 Hz                             16,7 ms
```

Y al abrir `volcar` aparecio la causa, en una palabra:

```rust
(self.panel.add(off + i) as *mut u64).write_volatile(a | (b << 32));
```

★★★ **`volatile` le PROHIBE al compilador tocar ese bucle.** No lo puede
vectorizar, no lo puede convertir en `rep movsb`, no puede juntar dos escrituras.
La semantica es *"emite exactamente esto, tantas veces como lo escribi"*. **El
bucle que se lee es el bucle que corre.**

```text
   volcar la pantalla entera   1.036.800 escrituras de 8 B
   limpiar la pantalla entera  2.073.600 escrituras de 4 B
   rect                        un `write_volatile` POR PIXEL, en 155 sitios
```

### ★★ Lo contraintuitivo: la casa ya lo sabia, y el ABI lo tenia escrito

```text
   bmo-lower/memoria.rs       `memcpy`/`memset` de TODO programa de C son
                              `rep movsb`/`rep stosb` desde el 13-08
   objetos.rs, FB_OP_BYTES    "es lo que hace falta para llenar la pantalla
                              entera con un `rep stosd` sin multiplicar nada"
```

**El ABI declaraba un campo cuyo comentario dice para que existe, y el compositor
era el ultimo sitio de la casa que seguia moviendo pixeles a mano.** Un `.bex` de
C copiaba mas rapido que el escritorio. Y la cifra lo grita: aquel bucle byte a
byte daba **214 MB/s**; el blit de aqui se midio en **~300 MB/s**. Es el mismo
error escrito dos veces.

### El segundo cuello, y estaba en el carril VERDE

`glifo` llamaba a `punto` por cada bit encendido, y `punto` **marca**. `Sucias`
son 136 bytes dentro de una `Cell`, o sea `get()` + `set()` = **272 bytes
copiados por pixel**:

```text
   la barra de tareas, un fotograma    ~110 letras, ~4.950 pixeles
   lo que esos pixeles PINTAN                   19,3 KiB
   lo que su contabilidad COPIA              1.315,0 KiB
   -----------------------------------------------------
   razon papeleo / trabajo                        68 a 1
```

★★★ **Un carril VERDE no es un carril barato.** El color dice lo que arriesgas al
TOCARLO, no lo que cuesta EJECUTARLO. Buscar rendimiento solo en lo rojo es como
se pasan los sesenta y ocho a uno por delante de las narices. `rect` ya marcaba
una vez desde agosto; `glifo` no lo habia copiado.

### La prueba, y son las de esta casa: se desensamblo

```text
   rep movsb    2 sitios con `cld` delante  -> los dos caminos de `volcar`
   rep stosd  172 sitios, TODOS con `cld`   -> `rect` y `limpiar`, incrustados
                                               por el compilador en cada
                                               llamador
```

Los 172 con su `cld` son la prueba de que no quedo ni un bucle viejo escondido:
si alguno hubiera sobrevivido, la cuenta no cuadraria.

[!] **Y una correccion de la casa a la casa**: `memoria.rs` cerro su `cld` con
*"nadie en BMO emite `std`, asi que en la practica sobra siempre"*. **Es falso en
este binario** -- `compiler_builtins` trae un `memmove` con camino hacia atras que
pone `DF` (`4008a400: fd`). Lo restaura antes del `ret`, asi que no hay fallo;
pero la frase de la que colgaba "sobra siempre" no era cierta.

### Lo que esto NO es

★ **No es zero copy.** Zero copy es el **page flip**: no mover nada y cambiar la
direccion que lee el escaner. Sigue bloqueado --tras `ExitBootServices` el GOP no
existe-- y es el escalon 8 de `LA_RAM.md`.

> Mientras el hardware obligue a copiar, el trabajo es que la copia obligada
> cueste UNA instruccion y no un millon.

### El corte: `pantalla.rs` pasa a CARRILES, y el corte lo eligio el cuello

```text
   roja.rs      MOVER PIXELES     memoria cruda, cuenta en un registro  MAQUINA
   amarilla.rs  LA CONTABILIDAD   le dice al rojo CUANTO copiar         APARATO
   verde.rs     LAS LETRAS        `punto` encima de `punto`             NADA
```

★★ La prueba de que el corte era el bueno estaba **dentro del propio fichero
desde agosto**: la cabecera de `Volcador` ya decia que el compositor tiene tres
capas --POLITICA, DIBUJO, VOLCADO-- y que *"solo en el volcado una GPU cambia
algo"*. Los carriles son esas capas con su semaforo puesto.

Y R18 se cobro su primer precio el mismo dia: `userland/src` **no estaba en la
lista de arboles vigilados**, asi que el fichero que mueve todos los pixeles del
sistema se habria partido en carriles sin juez. Un carril sin juez otra vez, y en
el sitio con `[cuesta] MAQUINA`.

[!] **Lo que este episodio NO demuestra**: que el escritorio vaya mas rapido. Eso
lo dice el metal, no el desensamblador. Lo que esta probado es que la instruccion
salio y que no queda ni un bucle del anterior. **El numero lo pone el Ryzen.**

---

## Ep. 53 -- El compositor dejo de ser el cuello, y entonces cambio la pregunta

**2026-09-09**, la misma noche. Con `rep movsb` puesto y el papeleo de los glifos
quitado, el compositor aporta **menos de un milisegundo** a un fotograma normal.
Y ahi la pregunta correcta deja de ser *"cuanto tarda en pintar"* y pasa a ser
**"cuanto tarda desde que muevo la mano hasta que lo veo"**. Caudal contra
latencia: un motor grafico se juzga por la segunda.

### El presupuesto de la mano al pixel

```text
   [2] el hilo del bus drena el anillo      <= 4 ms     BUS_PERIOD_MS
   [3] el compositor se entera              <= 1 ms     el LATIDO, 1 kHz
   [4] pinta y vuelca                        < 1 ms     (desde hoy)
   [5] el ESCANER lo ensena                 <= 16,7 ms  y sin V-Sync
```

★★ **Lo que pone BMO-X de su parte es menos de 1 ms de ~22.** Los milisegundos
estan en los dos EXTREMOS --el aparato que habla cuando quiere y el escaner que
mira cuando quiere-- y ninguno de los dos es codigo del compositor. Seguir
apretando el compositor es apretar la pieza que ya no aprieta.

### ★★★ Y el 4 ms tiene un dueno que ya sabia la respuesta

`BUS_PERIOD_MS = 4` es una constante, y su comentario razona sobre un **teclado**
boot (*"pide que se le sondee cada 8-10 ms"*). El aparato que decide la latencia
que se NOTA es el **raton**, y muchos piden 1 ms.

```text
   uhid/enumera.rs   LEE el bInterval del descriptor
                     se lo PASA al Endpoint Context del xHC
                     lo ESCRIBE en el log
   bus.rs            drena el anillo a 250 Hz, pase lo que pase
```

> El aparato dice cada cuanto quiere hablar, el controlador se entera, y el hilo
> que le escucha no se ha enterado.

### El septimo instrumento donde nadie mira

`ritmo()` y `peor_trabajo()` miden ese hilo desde hace semanas. **Los leia UN solo
sitio: `cabina/cockpit.rs`, que es una pantalla de Ring 0** -- de donde no se
vuelve. Van siete: CABINA, el testigo del USB, el pulso, el volcado, el modo del
lienzo, `cuerpo`, y este.

Ahora suben por `INFO_USB_RITMO` (campo 100 del contrato) y se ven en la barra:
`entrada 4ms peor NNNus purga`. ** El `peor` se enciende cuando pasa del **80%
del periodo**: si un solo trabajo de la vuelta se acerca a lo que dura la vuelta,
el hilo no puede sostener su ritmo. Es `C/T` acercandose a 1 -- la cuenta de
`PLAN_EL_COMPAS` aplicada al hilo que hoy decide la latencia de todo el sistema.
Y es lo que dira si bajar a 1 ms cabe, en vez de adivinarlo.

### La regla del pixel que se hizo pieza, y su excepcion

`scene/huella.rs`: **lo que no cambia no se marca**. `testigo` lo hacia desde
agosto; sus dos vecinos de barra, escritos despues, repintaban 650 px de ancho en
cada fotograma sin que nadie lo decidiera. Era una costumbre de un fichero.

★★ **Y el pulso NO la lleva, a proposito.** Su aguja es la prueba de vida del
bucle: un instrumento de vida que se calla cuando no cambia nada se calla justo
cuando el bucle se muere. Por **L4** --una regla se prueba diciendo que NO-- esa
excepcion es lo que convierte la costumbre en regla.

[!] Y su tamano, dicho sin vender: **~1 MB/s hoy**, porque la barra repinta 3-20
veces por segundo. Lo que evita es el precio del EXITO -- a 60 fps los mismos
chips serian 3,7 MB/s de pintar lo que ya estaba.

Todo escrito en [`docs/plan/PLAN_EL_PIXEL.md`](docs/plan/PLAN_EL_PIXEL.md), con
las siete reglas y su estado real.

> Un orquestador que no manda en sus dos extremos no orquesta: acompana.

---

## Ep. 54 -- La primera foto con los instrumentos puestos, y los dos mentian igual

**2026-09-09.** Arranque con la barra completa. Todo lo que se predijo ayer se
cumplio, y los dos instrumentos nuevos fallaron de la MISMA forma.

### Lo que salio bien, y con el numero

```text
   latido 300/s  pinta 4  cuerpo 2  puerta 997
```

★ **`cuerpo 2`**: dos milisegundos de CPU por segundo. El compositor gasta el
**0,2 % de un nucleo**, y 0,5 ms por fotograma pintado. La prediccion de ayer
--*"menos de un milisegundo"*-- se cumple. `cuerpo + puerta = 999` de 1.000: la
contabilidad cuadra sola.

Y DOOM: **66 fps a 1600x1000 en x5**, con el volcado a **6,0 GB/s** hacia el
framebuffer.

### ★★★ Y los dos instrumentos nuevos dijeron lo mismo, mal

```text
   volcado 8100K cajas 1        <- 8.100 KiB es EXACTAMENTE 1920x1080x4
   entrada 4ms 7666us bombeo    <- 7.666 us contra un periodo de 4.000
```

El primero parece *"el troceado degenero y se vuelca la pantalla entera"*. El
segundo parece *"el hilo del bus no cabe en su periodo, `C/T` = 1,92"*. **Los dos
son casi seguro el ARRANQUE**: el volcado completo que hace
`activar_doble_bufer`, y la enumeracion del USB.

*** Y ninguno de los dos podia decirlo, porque los dos son **maximos desde el
arranque que no bajan nunca**. La cabecera de `Volcado::peor` lo defendia asi, y
tenia razon a medias:

```text
   un maximo que se olvida no es un maximo        <- cierto
   un maximo que no caduca no sabe decir AHORA    <- tambien cierto
```

Hacen falta **los dos numeros**, no uno mejor. Ahora el volcado dice
`12K pico 8100K` --ultimo y pico-- y el ritmo del bus publica el peor del
**ultimo segundo**, con su `/s` en la barra. El de siempre se queda para
`cockpit.rs`, que es donde se audita.

### ★★★ Y EL HALLAZGO: 35 instrucciones para escribir 8 bytes

DOOM: `fotograma 14948 us, blit 7833, de ellos expansion 6738 + volcado 1063`.
El volcado es perfecto. La expansion escribe 192.000 veces en RAM cacheada y
tarda 30,2 M de ciclos: **157 ciclos por escritura**. Eso no lo hace ni un CPU
con la cache apagada.

Se compilo `expandir_fila` con BMO C y se desensamblo. Su bucle interior
--`while (j > 0) { *d8 = par; d8++; j--; }`-- son **35 instrucciones**:

```text
   utiles                  1     movq %rdx,(%rax)
   push/pop a memoria      8
   movabsq de 10 bytes     3     para cargar los literales 0, 1 y 8
   imulq                   1     PARA MULTIPLICAR 1 x 8
```

**El puerto de DOOM ya habia quitado ese `imul` a mano**, y lo dejo escrito:
*"aqui no hay indices: hay dos punteros que caminan"*. El compilador lo devolvio,
ahora para calcular el `sizeof` en tiempo de ejecucion, en cada vuelta.

> Una optimizacion escrita en C que el generador de codigo deshace no es una
> optimizacion: es un comentario.

BMO C emite como una **maquina de pila**: todo por `rax`, operandos por
`push`/`pop`, cada local en su hueco de `%rbp`. Eso no fue un error --es lo que
hace que quepa y se pueda leer-- pero **nunca se midio lo que cuesta**, y por eso
la cifra aparece hoy, por sorpresa, dentro de DOOM. Afecta a **todo `.bex` de C y
C++**; INTI tiene su propio emisor y no pasa por ahi.

[`docs/plan/PLAN_EL_CODEGEN.md`](docs/plan/PLAN_EL_CODEGEN.md) trae el
desensamblado entero y cuatro escalones: tres son MIRILLAS (plegar constantes,
literales sin `movabsq`, no pasar por la pila con un operando constante) y el
cuarto ya es un asignador de registros, que es otro proyecto.

⚠ **No se toca nada todavia**: este backend lo usan todos los `.bex`, y este mes
ya se pagaron cinco fallos de codegen. Primero el numero, y la decision es del
dueno.

---

## Ep. 55 -- El emisor no decide: la regla primero, y luego el codigo

**2026-09-09.** El dueno lo puso en ese orden: *"vamos a cambiar reglas de
modular codegen POR COMPLETO, con reglas que son para facilitar y asi no tener
muchos problemas"*. Primero la ley, despues tocar. Y acerto tres veces.

### La regla, y es toda la carpeta

```text
   DECIDIR   que hay que emitir     PURO: entra un AST, sale un numero
   EMITIR    los bytes              una tabla, sin elegir nada
   COLOCAR   donde va cada cosa     el `.bex`
```

> **El emisor no decide.** Si hay que elegir entre dos secuencias de bytes, la
> eleccion se toma ANTES y en una funcion pura.

★★ Mezclados, una optimizacion es un parche sobre bytes que no se puede probar
sin arrancar la maquina. Separados, es una funcion que devuelve `Some(8)` en vez
de `None` -- y eso se prueba con un `assert_eq!` en milisegundos.

`codegen/decidir/` nace con dos carriles: `roja.rs` (el plegado, `[cuesta] DATO`)
y `amarilla.rs` (las cuentas de la imagen, `TAREA`). **No hay verde, y se dice
por que**: aqui no hay nada que se pueda tocar sin miedo, e inventar un verde
para tener los tres seria decir que algo es seguro porque falta un fichero.

### ★★★ Y el plegador YA ESTABA COMPLETO

`constante_de` lleva meses resolviendo `1 << 16` y las flechas del mapa de DOOM
para los inicializadores. **El emisor de expresiones no le preguntaba.**

```text
   lo que faltaba NO era la maquinaria
   era que el que emite bytes supiera A QUIEN PREGUNTAR
```

### ★★ EL BANCO CAZO LA PRIMERA VERSION, Y ESO ES LA MITAD DEL EPISODIO

Plegar cualquier constante puso **5 de las 500 filas en rojo** en el acto, entre
ellas el propio escalado de DOOM y el censo de signo. El motivo no era el
plegado: era el **RECORTE**. El emisor llama a `recortar_a_32` en unas ramas y no
en otras, y la de un literal suelto es de las que no:

```text
   Expr::Int(0x80000000)   la rama larga NO recorta -> 2.147.483.648
   plegado + recorte       `movsxd` extiende el signo -> ...FF80000000
                           y `span >= 0x80000000` pasa a ser FALSO
```

*** Asi que la puerta del emisor pliega **solo `+`, `-` y `*`**, que son las tres
ramas que SI recortan justo despues. Con eso el camino corto y el largo son el
mismo **por construccion y no por revision**. Y son exactamente las que hacian
falta: la suma de punteros construye un `Mul` de dos constantes.

> Un plegador que contesta a todo es un plegador en el que no se puede confiar
> para nada. **L4**: una regla se prueba diciendo que NO.

### Las tres mirillas, y lo que midieron

```text
   C1  plegar constantes           el `imul` de 1 x 8 desaparece
   C2  literales sin `movabsq`     siete bytes en vez de diez
   C3  sin PILA cuando el derecho
       es constante                dos accesos a memoria menos por operacion
```

El bucle interior de la expansion de DOOM, desensamblado antes y despues:

```text
   instrucciones   35 -> 28
   imul             1 -> 0
   movabsq          4 -> 0
   push/pop        10 -> 2        <- esto es lo que importa
```

★ Y el resultado que no se buscaba, en el build entero:

```text
   doom.bex     911.359 -> 857.087 B   -6,0 %   (54 KB menos de codigo)
   los diez .bex de C          -6,0 % de media, hasta -9,0 %
```

[!] **500 de 500 filas verdes.** Y sigue sin haber una sola medida en metal: lo
que esta probado es el desensamblado y el banco. El `expansion N us` del `[perf]`
de DOOM es el juez, y todavia no ha hablado.

### Y la idea del dueno que se convirtio en escalon: LAS ANTEOJERAS

> *"no es mas velocidad: es que WAIT ponga trabas a otros puntos que no le
> interrumpan. Es concentrar al caballo con todo para ganar la carrera."*

Tiene nombre --**interrupt shielding**, **core isolation**-- y es lo que hace un
sistema de audio o de trading antes que cualquier optimizacion. Hoy el LAPIC va
PERIODICO a 1 kHz: **mil interrupciones por segundo por nucleo**, cada una con su
`xsave`/`xrstor`, aunque DOOM este solo y no las use ninguna. El coste en ciclos
es pequeno; el dano son la cache que ensucian y el punto de expropiacion que
meten **cada milisegundo**.

Y va por `WAIT` sin tocar los dos syscalls congelados, porque `WAIT` ya dice
*"despiertame cuando X"* y solo le falta la otra mitad: *"y necesito C sin que me
toquen, cada T"*. Escalon **E6** de [`PLAN_EL_COMPAS`](docs/plan/PLAN_EL_COMPAS.md).

★ Y de paso contesta lo del 0,1 ms: **hoy no se puede, y no por lentitud**. Con
el LAPIC periodico a 1 kHz, un milisegundo es la unidad mas pequena que el
sistema sabe **NOMBRAR**. Con TSC-deadline la unidad pasa a ser el ciclo -- pero
eso es precision de despertar, no latencia de punta a punta: el bus USB sigue
poniendo 4 ms y el escaner 16,7.

---

## Ep. 56 -- Un compilador no falla como un kernel, y por eso necesita su propio eje

**2026-09-09.** El dueno miro los carriles del codegen y dijo que no bastaban:

> *"el modulo nivel 3 es bueno PERO no es suficiente. Hablo de modular propio
> que tenga enfoque en C, porque si es archivo y codegen hasta AST TODO SON
> valiosos, en sentido de POR QUE cada uno, para no tener sorpresas. El
> compilador de C es algo que considero DELICADO."*

★★★ Y la razon por la que tiene razon cabe en dos lineas:

```text
   en Ring 0      un fallo se paga DONDE ESTA: la maquina se para o se corrompe
   en un          un fallo se paga LEJOS: el compilador acaba en verde, el .bex
   compilador     se escribe, el emulador pasa, y el sintoma sale dentro de un
                  juego de 900 KB, tres semanas despues y en otro fichero
```

**Esa DISTANCIA es el problema entero de un compilador**, y ninguno de los ejes
de la casa la nombraba: `[cuesta]` dice cuanto duele y `[riesgo]` dice por que
fallara -- **ninguno dice DONDE LO VAS A VER**.

### Las dos etiquetas nuevas, y la segunda es la herramienta

```text
   [fase]     LEXICO SINTAXIS ARBOL TIPOS EMISION IMAGEN

   [aparece]  AQUI       el compilador lo dice, en su linea        gratis
              BANCO      una de las 500 filas se pone roja         segundos
              EMULADOR   compila, y el emulador lo caza            minutos
              METAL      el emulador pasa, y falla en el Ryzen     un arranque
              DENTRO     todo pasa y sale dentro de un programa
                         grande, lejos de su causa                 DIAS
```

★ **La escala esta ordenada por lo que cuesta encontrarlo**, y ese orden ES la
informacion. Los cinco fallos de codegen del 01 al 04-09 eran `DENTRO` los cinco.

### El mapa de las sorpresas: 13 de 34

```text
   LEXICO 2   SINTAXIS 7   ARBOL 8   TIPOS 3   EMISION 11   IMAGEN 3
   -> y en TRECE de ellos el fallo aparece LEJOS. Ahi el banco no protege.
```

Y el peor de la lista tiene nombre: **`parser/preprocessor.rs`**. Un `#if` mal
evaluado **compila la rama equivocada sin decir nada**. No hay error: hay otro
programa. Ni un guardian del mundo lo ve.

★★ Y `codegen/decidir/roja.rs` es la prueba de que el eje sirve: plegar de mas
puso **5 de 500 filas rojas en el acto**. Una decision PURA la caza el banco; la
misma decision dentro del emisor habria salido en DOOM. Por eso la regla *"el
emisor no decide"* no es de estilo -- **mueve un fichero de `DENTRO` a `BANCO`**,
que es de dias a segundos.

`toolchain/tools/fases/fases.py`, trinquete en 34 y cableado al build.

### Y de paso, dos respuestas

**El divisor del LAPIC.** El dueno pregunto si se podia partir el tick,
*"0,5 + 0,5 para llegar a 1 kHz, y que cada uno diga que aporta"*. Se puede, y
son dos registros ya escritos (`0x3E0 = 3`, el divisor; `0x380 = hz/1000`, la
cuenta): poner `hz/2000` da 2 kHz con **una escritura**.

⚠ Pero eso no exprime, **multiplica**: cada disparo cuesta lo mismo, asi que el
doble de disparos es el doble de gasto. ★★★ **El truco que buscaba es esa idea
dada la vuelta**: quitar el bit 17 --un solo disparo-- y programar el instante
del proximo evento que importa. Entonces no hay mil disparos ciegos: hay N, y
**cada uno tiene dueno y motivo**. Eso es literalmente *"que cada uno diga que
aporta"*, conseguido no disparando en vez de dividiendo.

**Los quince minutos.** *"Que si pasa 15 minutos se automatice para concentrar
TODO en un objetivo"*. Es `E6` pero **ganada en vez de declarada**, y encaja con
la ley sin anadir nada: `EL ORQUESTAL` ya dice que el foco decide CUANTO y no
QUIEN. Un foco sostenido no sube de prioridad -- **le quitan las distracciones**.
Escalon `E7` de `PLAN_EL_COMPAS`, con su aviso: un sistema que cambia de
comportamiento a los quince minutos se comporta distinto de como lo probaste, y
eso tiene que decirlo la barra o es el Bloq Num otra vez.

---

## Ep. 57 -- El semaforo del compilador se DEDUCE, y el guardian me cazo a mi

**2026-09-09.** Tres peticiones del dueno, y la tercera es la que manda:

> *"el estandar es semaforo de rojo y verde, EL PORQUE. Lo otro es DIVIDIR
> todos los archivos que emiten. Y ya no aplicaremos como lineal sino DINAMICO
> PURO, con reglas que pides en C, **para que el compilador no tenga que
> ADIVINAR**."*

### ★★★ El color no se elige: se DEDUCE de quien te caza

En Ring 0 el semaforo se elige --*"que arriesgo si lo toco"*-- porque el fallo
se paga donde esta. En un compilador eso no vale: lo que decide el riesgo de
tocar una pieza **no es lo que hace, es quien la va a cazar**.

```text
   AQUI      -> VERDE      el compilador te lo dice antes de que salga
   BANCO     -> VERDE      500 filas te sujetan
   EMULADOR  -> AMARILLO   hay que ejecutar para verlo
   METAL     -> AMARILLO   hace falta un arranque
   DENTRO    -> ROJO       nadie te sujeta; sale lejos de la causa
```

** Y `fases.py` lo **COMPRUEBA** en vez de leerlo: un `[carril]` que no cuadra
con su `[aparece]` es una de las dos etiquetas mintiendo.

### ★★ Y en el primer arranque me caza a MI, con cuatro

```text
   decidir/roja.rs      [aparece] BANCO  y [carril] ROJO      no cuadran
   emitir/amarilla.rs   [aparece] DENTRO y [carril] AMARILLO  no cuadran
   emitir/verde.rs      [aparece] METAL  y [carril] VERDE     no cuadran
   emitir/mod.rs        [aparece] BANCO  y [carril] AMARILLO  no cuadran
```

*** Y el guardian tenia razon: **habia reusado tres palabras para dos ejes
distintos.** `roja/amarilla/verde` como NOMBRE de carril es un reparto en tres
de una carpeta; `[carril] ROJO` es quien te sujeta. Coincidian a veces y por
casualidad -- que es el `[riesgo] ESPEJO` de esta casa escrito con etiquetas.

Las carpetas se renombraron **por lo que de verdad separan**:

```text
   decidir/plegado.rs     decidir/imagen.rs
   emitir/valor.rs        emitir/direccion.rs      emitir/orden.rs
```

Y `toolchain/lang/c/src` **salio** de `CARRILES_FUERA_DEL_KERNEL` -- entro y
salio el mismo dia. Un guardian mirando un arbol donde ya no hay nada que juzgar
es el guardian MUERTO que R18 existe para evitar.

### La division de los que emiten

`emit_expr` eran **600 lineas y cincuenta formas** en un solo `match`. Ahora:

```text
   valor.rs      23 formas   aritmetica, signo, ancho, comparaciones   ROJO
   direccion.rs  19 formas   variables, punteros, campos, indices      ROJO
   orden.rs       8 formas   llamadas, cortocircuitos, secuencia       AMARILLO
```

★★ **El despacho se queda EXHAUSTIVO y sin comodin**, y es la unica decision de
diseno del corte: el dia que nazca una forma nueva de expresion, **el compilador
de Rust para ahi** y obliga a decidir de que color es. Partirlo en tres `if` que
devolvieran `bool` habria sido mas corto y habria perdido justo eso.

`codegen/mod.rs`: **2.238 -> 1.617 lineas**. 500 de 500 filas verdes.

### Y lo que queda, que es lo que el dueno pidio de verdad: C5

*"Que el compilador no tenga que ADIVINAR"*. Hoy el emisor **vuelve a deducir el
tipo cada vez que lo necesita** -- `expr_is_unsigned`, `expr_is_float`,
`recorte_de`, `pointer_scale`: **41 preguntas contadas**, y cada una recorre el
arbol otra vez para contestar lo que ya se sabia.

★★★ Y no es rendimiento: **es donde viven los cinco fallos del mes.** `div`
donde iba `idiv`, `shr` donde iba `sar`, un recorte que no se aplica -- los tres
son la misma frase: *el emisor pregunto y le contestaron mal*. Con el tipo
resuelto UNA vez y cargado en el arbol, no hay nada que preguntar. Y `tipos.rs`
pasaria de `[aparece] DENTRO` a `AQUI`, que es de dias a gratis.

Escrito como `C5` en `docs/plan/PLAN_EL_CODEGEN.md`, con su aviso: el arbol gana
un campo, el parser tiene que rellenarlo, y **un arbol medio anotado es peor que
uno sin anotar**. Se hace de una vez o no se hace.

---

## Ep. 58 -- El 68 a 1 estaba clonado en CINCO sitios, y uno era el arrastre

**2026-09-09.** El dueno pidio arreglar el retraso del escritorio que habia
reportado: *"cuando movi todo la pantalla en terminal TODO rapido se ve como que
se retrasa"*.

### Los dos sospechosos que NO eran

```text
   la consola      son SEIS lineas de salida (`LINEAS`), y ya tenia un enum
                   `Repinta` de tres niveles. No es ella
   el scroll       `di()` sube seis filas de un array. Tampoco
```

### ★★★ El que si era: `p.punto` con `scene_color`, en CINCO bucles

```rust
for y in tira.y0..tira.y1 {
    for x in tira.x0..tira.x1 {
        p.punto(x, y, scene_color(c, visible, x, y, p.alto));
    }
}
```

`punto` **marca**, y marcar copia `Sucias` --136 bytes-- dos veces: **272 bytes
de papeleo por pixel**. Es el mismo 68 a 1 que se cazo en `glifo` esa misma
manana, **clonado en cinco sitios de este arbol y sin arreglar en ninguno**.

```text
   scene::erase_box        325.500 px ->  88,5 MB de papeleo   por UNA pulsacion
   scene::erase_moved        4.800 px ->   1,3 MB              por CADA movimiento
                                                               del raton, hasta 250/s
   keys: el conmutador                                         por pulsacion
   desktop: run_relayout                                       por pulsacion
   shell: cerrar la calc                                       por orden
```

★★ **`erase_moved` es el del arrastre**, y es el unico que corre por evento del
raton: **326 MB/s solo en APUNTAR** mientras arrastras una ventana. Eso es lo que
el dueno estaba sintiendo.

Arreglo: una `marcar` por region y `punto_ya_marcado` dentro. Los pixeles son
identicos; lo que se va es la contabilidad.

[!] Y `erase_box` llevaba escrito en su cabecera *"unos 325k pixeles sobre
memoria de video sin cache, que no es gratis"*. **Culpaba a la memoria de
video**: con doble bufer eso escribe en el LIENZO, que es RAM cacheada. Lo caro
nunca fue el pixel -- eran los 88 MB de apuntarlo.

### Y el instrumento que faltaba: `tamano`

El 09-09 tres mirillas del codegen encogieron todos los `.bex` un 6 %, y eso se
supo **sumando a mano dos listados del build**. El dato salia en pantalla las dos
veces y nadie los comparaba.

`toolchain/tools/tamano/` guarda los 30 y ensena el delta en cada pasada. **Y
cazo su primer cambio en su primera pasada**: `sys/d.bex 609624 -> 609112`, que
es este mismo arreglo.

[!] **REPORTA, no manda**, y a proposito: `avisos` y `fases` paran el build
porque un aviso nuevo siempre es malo, pero un programa puede crecer con razon.
Un guardian que grita cada vez que el proyecto avanza se apaga en una semana.

⚠ Y con su aviso puesto: **el tamano no es la velocidad**. Van juntos en el caso
concreto de pasar de pila a registros y no en general. El juez sigue siendo
`expansion N us` del `[perf]` de DOOM, y sigue sin hablar.

---

## Ep. 59 -- EL TROQUEL entra, y el banco me caza un `0x100` en un `u8`

**2026-09-09.** El dueno dijo *"aplicalo, primero en C"*, y se aplico.

### Lo que habilito el camino, y fue una comprobacion de dos minutos

```text
   que registros extendidos emite hoy BMO C?   r8, r9, r10 y r11. Y NINGUNO mas
   -> r12..r15 estaban ENTEROS SIN USAR, y ademas son de los que una llamada
      PRESERVA: un valor ahi sobrevive a un `call` sin que nadie lo guarde
```

**Esa es la matriz.** Cuatro huecos, y no se supusieron: se contaron.

### La regla de seguridad es una frase, y no hay analisis de alias

> **Una local cuya direccion nunca se toma no la puede pisar ningun puntero.**

★★ Y el escaner **contesta que SI en la duda**: su brazo comodin da `true`, asi
que una forma del arbol que no conozca deja la funcion entera en la pila --
correcta y lenta. Es `PTE_NUESTRA` del kernel con otra moneda: *la duda se
resuelve por el lado que solo cuesta*.

### ★★★ Y EL BANCO ME CAZO, con 102 filas de 500

```text
   let reg = 0xE0 + ((r - 8) << 3);
```

Para `r12` eso son `0xE0 + 32 = 0x100`, que en un `u8` de release **ENVUELVE a
0x00**. Y `modrm = 0x00` no es un registro: es `[rax]`, un operando de MEMORIA.
El destino de cada escritura pasaba a ser la direccion que hubiera en `rax`.

*** El desbordamiento fue el sintoma. **El error era haber escrito la constante
de un caso concreto como si fuera la base**: `0xE0` ya llevaba dentro el `<<3`
de `r12`, y sumarselo otra vez era contarlo dos veces. La base es `0xC0`.

Y es la tercera vez esta semana que el banco caza algo mio en el acto. Un
`[aparece] BANCO` no es una etiqueta: es la diferencia entre veinte segundos y
un arranque en el Ryzen.

### La medida, y no es la que se esperaba

```text
   el bucle interior de la expansion    instrucciones   accesos a MEMORIA
   original                                   35              10
   + las tres mirillas                        28               9
   + EL TROQUEL                               28               2
```

★ **Las instrucciones NO bajan; los viajes a memoria caen un 78 %.** `j` vive en
`r14` y `d8` en `r12`, y del bucle desaparecieron todos los `[rbp+disp]`. Lo que
queda son movimientos entre registros -- el baile de la maquina de pila por
`rax`, que es el escalon siguiente y no este.

### Y los `.bex` CRECIERON, que es lo correcto

```text
   apps/doom.bex   858.240 -> 863.872   +5.632   +0,7 %
   c/ray.bex        32.645 ->  33.157     +512   +1,6 %
   ...5 de 30, y el total +0,4 %
```

Son los cuatro `push` y los cuatro `pop` de cada funcion que usa la matriz. **Y
por esto `tamano` REPORTA y no manda**: un trinquete habria parado el build por
un cambio que hace el codigo mas rapido. La cabecera del guardian ya lo decia el
dia que se escribio -- *"un programa que crece puede estar creciendo por una
razon excelente"*-- y le ha tocado el primero.

[!] **Y sigue sin haber una sola medida de velocidad.** 500 filas verdes dicen
que es CORRECTO; los bytes dicen que CAMBIO. Que sea mas rapido lo dice
`expansion N us` en el Ryzen, y no ha hablado todavia.

---

## Ep. 60 -- La pantalla partida de DOOM: el instrumento miraba al otro lado de la valla

**2026-09-09.** El dueno trajo la foto de siempre: DOOM con la **mitad izquierda
jugando y la mitad derecha con la pantalla de TITULO**, y las columnas gordas.
*"Analiza por que se ve asi siempre."*

### Lo que la foto dice, y ya estaba escrito en el fuente

```text
   la vista 3D ocupa la mitad izquierda   porque MIDE la mitad: viewwidth=80
   las columnas se ven gordas             detalle bajo: de dos en dos
   la mitad derecha tiene el TITULO       el motor nunca pasa por ahi
   la barra de estado esta perfecta       la pinta otra ruta
```

No es geometria rota: es una **ventana de vista mal puesta**. Y por la formula
de DOOM, con `setblocks=10` y `setdetail=0` tiene que salir 320:

```text
   scaledviewwidth = setblocks*32        10 -> 320
   viewwidth = scaledviewwidth>>detailshift    con 0 -> 320
```

### ★★★ Y AQUI ESTA EL FALLO, Y ES EL DE TODO EL MES

La linea de `[perf]` decia **`blocks=10 detalle=0`** -- los dos correctos-- con
un `viewwidth=80` que con esos numeros **es imposible**. Y no mentia: imprimia

```c
   viewwidth, viewheight, screenblocks, detailLevel
```

`screenblocks` y `detailLevel` son las variables **del MENU**. Las que calculan
son `setblocks`, `setdetail` y `detailshift`, que es donde `R_SetViewSize` deja
sus argumentos.

> Los tres numeros eran ciertos y no hablaban del mismo lado. **Esa es la unica
> forma de que un instrumento mienta sin equivocarse.**

*** Y el motivo de que leyera esos y no los otros es de manual: `detailshift`
estaba en `r_main.h` y `setblocks`/`setdetail` **no estaban declarados en ningun
sitio alcanzable**. Un instrumento lee lo que tiene a mano.

Van NUEVE con esta forma en dos dias -- y esta es la primera en la que las dos
variables se PARECEN, que es lo que la hizo durar.

### El arreglo, que ademas es un experimento con dos respuestas limpias

La vuelta del 05-09 fijo `screenblocks=10` y `detailLevel=0`, y el Ryzen siguio
dando 80: **esos dos no calculan nada**. Ahora se ponen los de dentro --
`setblocks`, `setdetail`, `setsizeneeded`-- al OTRO lado de la llamada:

```text
   si la pantalla se arregla   el fallo estaba en PASAR LOS ARGUMENTOS
   si sigue partida            el fallo esta en `*` o en `>>`, y entonces
                               es del compilador y no de DOOM
```

[!] Y no es tapar: en BMO-X el menu de DOOM no se alcanza, asi que esto no es
una preferencia del usuario -- es una constante que hasta hoy se escribia en el
sitio equivocado.

El `[perf]` lleva ahora los seis numeros, los del menu y los que deciden:

```text
   [perf] vista: viewwidth=N viewheight=N anchoesc=N (set N/N shift=N | menu N/N)
```

---

## Ep. 61 -- Funciono, y la ventana de un segundo desmintio mi hipotesis

**2026-09-09.** La foto trae DOOM **a pantalla completa**. El experimento del Ep.
60 tenia dos respuestas y contesto la primera: **el fallo estaba en pasar los
argumentos** a `R_SetViewSize`. Poner `setblocks` y `setdetail` al otro lado de
la llamada lo arregla.

★ Y eso deja una pista viva: `R_SetViewSize(10, 0)` no dejaba 10 y 0 en sus dos
parametros. Es un fallo del compilador, `[aparece] DENTRO`, y sigue ahi.

### La barra de este arranque, entera

```text
   latido 301/s  pinta 119  cuerpo 15  puerta 989
   volcado 67K pico 8100K cajas 1     entrada 4ms 7781us/s bombeo
```

```text
   pinta 119/s     antes 3-20. DOOM vivo y el escritorio componiendo
   cuerpo 15 ms/s  el 1,5 % de un nucleo, CON DOOM corriendo
   volcado 67K     el ULTIMO fotograma; el pico sigue siendo el arranque
```

★★ El instrumento del Ep. 54 hace exactamente lo que se le pidio: `67K` y
`8100K` juntos, y ya nadie confunde *"esta pasando"* con *"paso una vez"*.

### ★★★ Y ME DESMINTIO A MI, que es para lo que se puso

```text
   entrada 4ms 7781us/s bombeo      ->  C/T = 1,95
```

**7.781 us en el ULTIMO SEGUNDO** contra un periodo de 4.000. El 08-09 escribi
que aquel `7666us` *"es casi seguro el arranque enumerando el USB"*. **Era
falso.** El hilo del bus se pasa de su periodo casi al doble, sostenido, mientras
DOOM corre -- y con `bombeo` como culpable, que es el drenaje del anillo.

> Una hipotesis comoda duro un dia porque el instrumento no sabia decir *ahora*.
> En cuanto supo, la tiro.

Es el numero abierto mas gordo que queda, y esta en el suelo de la latencia:
`PLAN_EL_PIXEL`, seccion 1.

### Y la barra, que el dueno pidio "elegante, inspirado en Wayland con blur"

[!] **Un desenfoque aqui no se veria, y eso es una respuesta y no una excusa.**
Desenfocar necesita TEXTURA, y detras de la barra hay un degradado vertical que
en sus 40 filas varia un 4 %. El desenfoque de un degradado es el mismo
degradado -- 76.800 pixeles recorridos tres veces para no ver nada.

★ Lo que de verdad despega un panel de Wayland del fondo no es el desenfoque: es
el **FILO**. La barra era `0x0F131D` sobre un fondo que arriba es `0x1B2233`:
casi el mismo color, y por eso se leia como una mancha. Ahora lleva una linea
clara arriba, la oscura que ya tenia abajo, y **separadores entre los tres
instrumentos**, que hasta hoy se leian como un solo parrafo de numeros.

*** El dia que haya un FONDO DE ESCRITORIO de verdad --una imagen-- el
desenfoque pasa a tener sentido. Y entonces se hace **una vez**: el fondo no
cambia, asi que su version borrosa tampoco.

---

## Ep. 62 -- Los 7.781 us del bombeo: partir el numero que no cuadra

**2026-09-09.** El dueno: *"investiga el bombeo ese de 7781us, que es el mas
gordo"*. Y lo es: `C/T = 1,95` sostenido en el hilo que decide la latencia de
toda la entrada.

### Lo que se descarto leyendo

```text
   la politica del barrido   TIENE `MAX_INTENTOS`: un aparato que no se sabe
                             adoptar deja de intentarse. No hay bucle infinito
   `Reabrir`                 solo toca puertos VACIOS, y es contabilidad: no
                             escribe un byte al xHC
   la regla del puerto tomado   un puerto en uso no se vuelve a tocar NUNCA,
                             y su comentario cuenta el bug del 31-07 que la puso
```

★ Y el codigo nombra su propio precio en el sitio correcto: *"adoptar lleva un
reset de puerto y esperas de verdad, **hasta un tercio de segundo**"*. O sea que
**7,8 ms es una ESPERA**, no un bucle de cuentas.

### Lo que no se puede saber leyendo, y por que

`bombeo` no es un trabajo: **son cuatro**.

```text
   dos cambios de CR3    y con ellos dos vaciados enteros de la TLB
   el drenaje del anillo  `bombear_interno`, con el barrido cada 500 ms dentro
   el audio               `audio::latido`, que encola tramas isocronas
   la foto de salud       `salud::refrescar`, que lee MMIO del xHC
```

*** Preguntarle a un numero que suma cuatro trabajos cual de ellos tarda **es
preguntarle al total**. Es exactamente la forma del `cuerpo 1066` del 08-09 y
del `blit 7833` del 09-09, y las dos veces la respuesta fue la misma: **partir**.

### El metodo, que ya lleva tres aciertos esta semana

```text
   los 40 ms del compositor  ->  `cuerpo` y `puerta`      -> era la PUERTA
   el blit de DOOM           ->  `expansion` y `volcado`  -> era la EXPANSION
   `bombeo`                  ->  `anillo`, `audio`, `salud`
```

`PEOR_US` pasa de cinco ranuras a ocho, `pump_bus` se mide por dentro, y la caja
`entrada` de la barra sabe los ocho nombres. El proximo arranque **dice cual**.

[!] Y no se ha tocado ni una linea de la logica del USB. Esto no arregla nada:
convierte una sospecha en una pregunta con respuesta. La diferencia entre las
dos es la unica razon por la que esta semana ha rendido.

---

## Ep. 63 -- El numero que llevaba el dia entero esperando, y lo que NO dice

**2026-09-09.** La foto del metal trae el juez del compilador.

### ★★★ LA EXPANSION: 6.738 -> 2.918 us

```text
                          us      ciclos    por escritura util
   antes (esta manana)   6.738    30,3 M         157,6
   AHORA                 2.918    13,1 M          68,3
```

**Un 57 % menos, y de 157 a 68 ciclos por escritura de 8 bytes.** Es todo lo del
09-09 junto --las tres mirillas y EL TROQUEL-- medido donde importaba: el bucle
mas caliente de DOOM, en el Ryzen.

Y `viewwidth=320 anchoesc=320 (set 10/0 shift=0 | menu 10/0)`: **los seis numeros
de acuerdo por primera vez**. El instrumento arreglado confirma su propio
arreglo.

### ★★ PERO EL FOTOGRAMA CASI NO SE MUEVE, y eso es la mitad de la noticia

```text
   antes   fotograma 14.948   blit 7.833   ->  lo que NO es blit:  7.115 us
   ahora   fotograma 14.536   blit 4.150   ->  lo que NO es blit: 10.386 us
```

El blit baja 3.683 us... y **lo demas sube 3.271**. No es casualidad: la vista
paso de `viewwidth=80` a `320`, o sea que **DOOM renderiza CUATRO VECES MAS
COLUMNAS**. Los dos cambios del dia se cancelaron casi exacto.

> El compilador pago el arreglo de la pantalla. Mismos 68 fps, y ahora se ve el
> juego entero en vez de un cuarto.

[!] Y hay que decirlo asi y no *"el compilador no sirvio"*: sin el, ver la
pantalla completa habria costado bajar a ~40 fps.

### Lo que el log dice del cuelgue, que es menos de lo que parece

```text
   no hay `ring3 fault`      -> no es una excepcion
   no hay mensaje de error   -> `I_Error` no corrio (y su `stderr` SI se ve:
                                `tables/stdio.h` lo manda a la consola)
   `main` es un `for(;;)`    -> DOOM no puede salir por su propio pie
```

*** Las tres juntas dejan una sola lectura: **se queda dentro de un tick**. Y de
las cuatro fases de `doomgeneric_Tick` --`I_StartFrame`, `TryRunTics`,
`S_UpdateSounds`, `D_Display`-- solo una espera a algo.

★ Y ese bucle **tiene salida**: si `I_GetTime()` avanza un tic, sale. O sea que
solo se queda si **el reloj se para**.

### Los dos instrumentos, y ninguno arregla nada

```text
   [vigia] en `TryRunTics`   cuenta las vueltas del bucle de espera y, a las
                             20.000, dice `lowtic`, `gametic`, `counts`, el
                             tiempo y `entertic`. Con eso el cuelgue tiene
                             NOMBRE y numeros en vez de una pantalla quieta
   [vivo]  en `main`         una linea cada 256 fotogramas
```

★★ El segundo separa dos estados que **hoy se ven identicos**: *"se colgo"* y
*"sigue vivo y lo que se callo fue el `[perf]`"*. Se arreglan en sitios
distintos, y hasta hoy no habia forma de saber cual era.

> Un instrumento callado y un sistema muerto se ven igual.

[!] Y lo que NO se ha hecho: inventar un arreglo. Con lo que hay en el log **no
se puede saber** donde se queda, y esta semana ha demostrado tres veces que
adivinar cuesta un dia y partir el numero cuesta un arranque.

---

## Ep. 64 -- El borrado que el fichero daba por hecho, y un log que era de ayer

**2026-09-09.** El dueno trae foto y log otra vez, y dice tres cosas. Dos son
observaciones suyas y una es un aviso que hay que devolverle antes de nada.

### [!] EL LOG ES DE LA IMAGEN ANTERIOR, y por eso "es el mismo patron"

```text
   no hay `[vivo] fotograma N`   -> el latido de `main` no corrio
   no hay `[vigia] ...`          -> el vigia de `TryRunTics` tampoco
   `entrada 4ms 27345us/s bombeo` -> `bombeo` sigue siendo UN numero
```

Los tres instrumentos son de `2641a97e` y `e5531778`, o sea del build que aun no
esta en el disco. **Un patron identico despues de un cambio que no se desplego
no dice nada del cambio**: dice que la imagen es la misma. Se anota aqui porque
es la tercera vez esta semana que un log llega antes que el flasheo, y la unica
forma de que deje de costar una vuelta es tener escrito como se reconoce.

### *** EL FALLO DE VERDAD: DOOM NUNCA LIMPIO LA PANTALLA AL ENTRAR

El dueno lo dijo en cinco palabras --*"no limpia el fondo"*-- y la foto lo
ensena: alrededor del juego se ve **la caja de Ejecutar con su `doom.bex` y la
rejilla de iconos**, tal cual estaban.

```text
   `limpiar_pantalla()`  escrita, probada, y llamada desde DOS sitios:
      Bloq Despl (cambiar escala)   OK
      F12 (volver de ceder)         OK
      DG_Init (reclamar el panel)   NO
```

*** Y el propio fichero afirmaba lo contrario. Cincuenta lineas mas abajo, desde
el 09-01, hay un comentario que dice *"DG_Init reclama la pantalla y la
LIMPIA"*, y hasta razona por que el `printf` va DESPUES del borrado. **Describia
una llamada que no existia.** El comentario era correcto el dia que se escribio
y la llamada se perdio; nadie lo comparo con el codigo porque un comentario no
se compila.

Por que se veia siempre y nunca se miro: a escala x5 DOOM ocupa el 77 % del
panel. El 23 % de alrededor **no lo toca nadie** -- ni DOOM, que solo pinta su
rectangulo centrado, ni el compositor, que ya no es dueno de la pantalla.

> El que reclama la pantalla es el que tiene que dejarla como quiere
> encontrarla. No hay nadie mas: ese es el contrato de `PANTALLA_RECLAMAR`.

★ Y el camino de vuelta SI estaba bien --`lend_screen` repinta degradado, barra,
rejilla, caja y olvida las huellas de los cuatro chips--. El agujero estaba solo
en la ida, que es la mitad que nadie escribio dos veces.

### ★ Y EL REPORTERO DE TAMANOS TIENE UN PUNTO CIEGO, dicho antes de que confunda

`tamano.py` dijo `clean: los 30 ejecutables miden lo mismo`. Con el arreglo
puesto y quitado, `doom.bex` mide **865.408 B las dos veces** -- y las dos
imagenes **difieren en 20.252 bytes**.

```text
   la seccion va rellenada a pagina  ->  5 bytes de `call` caben en el relleno
   el tamano no se movio             ->  el codigo si
```

No es un fallo suyo: mide lo que dice que mide. Pero *"miden lo mismo"* se lee
como *"no cambio nada"*, y no es lo mismo. Queda escrito aqui y en su cabecera.

### El numero que empeoro mientras se investigaba

```text
   `bombeo`   7.781 us/s  ->  27.345 us/s     C/T ~ 6,8
```

Seis veces y media el periodo de 4 ms, y **sigue siendo un solo numero**. La
particion en `anillo`/`audio`/`salud` esta compilada y esperando un arranque:
hasta que corra, preguntarle al total cual de los tres tarda es preguntarle al
total. Se deja tal cual, sin hipotesis -- que es lo que costo un dia el 09-09.

---

## Ep. 65 -- `GREEN: IS TURBO!`, o como una foto encontro el sexto fallo del codegen

**2026-09-09.** El dueno manda una foto de DOOM jugando y dice *"se repite mismo
patron pero cambio mucho"*. El fondo ya sale limpio (Ep. 64). Pero arriba a la
izquierda, en rojo, hay una linea que no deberia estar:

```text
   GREEN: IS TURBO!
```

### ★★★ POR QUE ESA LINEA ES UN DIAGNOSTICO Y NO UN ADORNO

`G_Ticker` solo la imprime cuando `cmd.forwardmove > 50`. Y **50 es el maximo
que DOOM puede grabar** -- `TURBOTHRESHOLD` esta justo ahi por eso. O sea que el
demo estaba entregando un numero **imposible**, y solo hay una forma:

```c
cmd->forwardmove = ((signed char)*demo_p++);   /* g_game.c:1936 */
```

Si ese cast no estrecha, un `-25` --andar hacia atras-- se lee como `231`.

### El banco lo confirmo en 0,02 s, y luego dijo donde

```text
   (signed char)231           ->  231     y debe ser -25   MAL
   signed char c; c = 231;    ->  231                      MAL
   (signed short)65511        ->  65511                    MAL
   short s = (short)65511;    ->  -25                      ok
   char c = -25;              ->  -25                      ok
```

`short` bien y `signed short` mal **es toda la pista**: no era el codegen. Era el
PARSEADOR. `parser/declarations.rs:923`:

```rust
Token::Signed => { self.advance(); TypeSpec::Int }
```

*** **`signed` era un sinonimo de `int` que ademas se tragaba el token de
detras sin mirarlo.** Su hermano `unsigned`, cinco lineas mas arriba, si
preguntaba -- con sus cinco casos y su `_ =>` que no avanza.

Tres fallos en una linea:

```text
   `signed char x`   ->  un entero de 64 bits: no trunca, no extiende signo
   `signed char` en una struct  ->  8 bytes en vez de 1: LA DISPOSICION CAMBIA
   `signed x;`       ->  `advance` incondicional: se come el NOMBRE
```

### Lo que eso le hacia a DOOM, y son dos sitios que se ven en la pantalla

```text
   d_ticcmd.h   `signed char forwardmove;`      ->  el MANDO entero, y el demo
   i_swap.h     `#define SHORT(x) ((signed short)(x))`
                                                ->  CADA numero del WAD
```

★★ El segundo es el gordo: `SHORT()` es como DOOM lee todos los offsets de
sprite, todas las columnas de textura y todos los vertices. Un offset negativo
--que los hay-- salia como 65.511.

### ★ Y EL BANCO ESTABA VERDE CON EL FALLO DENTRO

500 filas, ninguna roja, y **ni una escribia `signed char`**. No es que el test
fallara: es que el test no existia, y la palabra `signed` estaba en la gramatica
desde el primer dia. El census del signo tenia 16 casillas y ninguna preguntaba
por la unica palabra clave de C que este parseador no leia.

> Una gramatica que acepta una palabra y la traduce mal no da un error: da un
> programa que compila y hace otra cosa.

Cuatro casillas nuevas en `probe_signedness`, y las cuatro estaban ROJAS.

### El tamano, otra vez, y ahora en grande

```text
   `doom.bex`   865.408 B antes   865.408 B despues
   bytes que difieren:  479.632   de 864.255
```

**Mas de la mitad del binario cambio y el reportero dijo `clean`.** El aviso que
se escribio en su cabecera esta misma tarde (Ep. 64) queda demostrado el mismo
dia con un caso cinco ordenes de magnitud mayor que el que lo motivo.

### Lo que esto NO arregla, dicho antes de arrancar

No se ha tocado el cuelgue. Lo que si hace es quitar de en medio una causa que
lo explicaria entera --un demo desincronizado acaba antes de tiempo y `demo_p`
se sale-- **sin haberlo demostrado**. El proximo arranque tiene que decir tres
cosas: si `IS TURBO!` desaparecio, si `[vivo]` sigue contando, y que dice
`[vigia]`.
