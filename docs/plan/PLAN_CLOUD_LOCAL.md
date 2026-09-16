# PLAN CLOUD LOCAL -- el movil es la ANTENA, BMO-X es la pantalla

> Escrito el **2026-09-14** como PLAN_SATELITE, el dia que `red ping` llego a
> Internet desde el Ryzen, y renombrado ese mismo dia a **Cloud local**, que es
> como lo llamo el dueno. Lo pidio despues de chocar con el muro de la web:
>
> > *"mi celular Android se convierte en antena y eso puedo ver YouTube, por
> > completo; BMO ya no navega pero si la ANTENA. Es como Steam, si quiero jugar
> > en mi PC"*.
>
> Y la respuesta es que si: es el mismo reparto que Steam Link / Remote Play. Un
> aparato hace el trabajo pesado y otro solo ensena lo que le llega. Aqui el
> pesado es el movil y la pantalla es BMO-X. Un cloud, pero en casa.

---

# 0. POR QUE ESTO Y NO UN NAVEGADOR DENTRO DE BMO-X

Ver un video de la web desde BMO-X solo pide, a la vez:

```text
   HTTPS          TLS 1.3 -- G6 de PLAN_RED_TX, el muro
   la web         JavaScript, que las plataformas de video exigen
   el codec       VP9 / AV1 / H.264 en software: miles de lineas de lo mas duro
   el audio       Opus / AAC, sincronizado
```

Cada una son meses. **La antena se come las cuatro**: habla con la web como
cualquier movil y le entrega a BMO-X algo que BMO-X ya sabe tratar.

```text
   ANTENA (Android)                                     BMO-X (el Ryzen)
   ----------------------------------                   ----------------------
   la web, HTTPS, JavaScript, el codec   --  LAN, TCP  --> recibe MPEG-1 y MP2
   moderno; lo convierte a MPEG-1 +          sin cifrar     y lo pinta con
   MP2 640x360, que pesa ~1,5 Mbit/s         solo en casa   pl_mpeg en su ventana
```

** Con el enlace a 10 Mbit de hoy entra de sobra. Sin comprimir (640x360 RGB a
30 fps, unos 160 Mbit/s) NO entraria: por eso la antena convierte y no manda la
pantalla tal cual.

---

# 1. LO QUE ESTE PLAN SE NIEGA A PROMETER

```text
   [!] sin antena encendida no hay video: BMO-X NO navega, y no finge que si
   [!] el canal va SIN CIFRAR. Vale dentro de casa y con UNA sola IP permitida;
       fuera de la LAN, no
   [!] las plataformas de video tienen condiciones de uso: descargar o convertir
       su contenido puede incumplirlas. La antena de este arbol sirve ficheros
       que YA estan en su carpeta y no descarga nada; lo que se meta ahi es
       decision de su dueno, y las pruebas se hacen con videos propios o libres
   [!] el modo ESPEJO (S6) cuesta mucho mas que el modo PEDIDO: no se empieza por el
   [!] el kernel sigue sin saber lo que es una IP: la lista de "solo la antena"
       vive en Ring 3, en la app, no en el grifo
```

---

# 2. LOS DOS MODOS

```text
   PEDIDO (primero)   BMO-X dice "dame ESTE video"; la antena lo convierte y lo
                      sirve. Basta con Termux en el movil: NO hay app que escribir
   ESPEJO (despues)   la antena manda lo que tiene en pantalla, en vivo, y BMO-X
                      le devuelve teclado y raton. Es Steam Link al reves, y pide
                      una app Android propia (captura de pantalla + codificador)
```

---

# 3. LOS ESCALONES

- [ ] **S0 -- lo que tiene que estar antes.** G5 de `docs/plan/PLAN_RED_TX.md`:
      TCP de verdad desde `platform/shared/bmo-pila/src/tcp` contra un servidor
      de la LAN. Y el latido del GATE RED medido (hoy los tiempos salen de 16 en
      16 ms: `red ping` dice el real desde el 2026-09-14). **Como se sabe:** una
      conexion TCP abre, pasa bytes y cierra limpia contra la antena de S3.

- [ ] **S1a -- LEER de ESTRATOS.** Medido el 2026-09-14 en
      `Ultra_userspace/userland/src/estratos.rs`: Ring 3 puede ESCRIBIR un
      fichero grande (`crear_desde`, `copiar` desde FAT32 de cualquier tamano)
      pero **no hay operacion para LEER su contenido**. Falta la espejo de
      `crear_desde`: el kernel deja N bytes del fichero, desde un desplazamiento,
      en un bloque `KIND_MEMORIA` del proceso -- dos llamadas para cualquier
      tamano, sin punteros de Ring 3. **Como se sabe:** un `.mpg` de 20 MiB
      copiado a ESTRATOS con `copiar` se lee entero y su suma coincide con la
      del original en FAT32.

- [ ] **S1b -- la Biblioteca ensena lo de ESTRATOS.** Hoy
      `Ultra_userspace/services/director/src/scene/data/biblioteca.rs` solo
      recorre DATOS (FAT32). Los videos viven en ESTRATOS (seccion 6), asi que la
      Biblioteca los lista de ahi y dice de que volumen es cada uno. **Como se
      sabe:** el `.mpg` copiado sale en la Biblioteca marcado como de ESTRATOS.

- [ ] **S1 -- el reproductor LOCAL.** Una app `.bex` en BMO C con pl_mpeg
      (licencia MIT, un solo fichero) que abre un `.mpg` del disco y lo pinta en
      su ventana, con el audio por el tubo. Sin red: primero se prueba que BMO-X
      sabe ENSENAR video. **Como se sabe:** un `.mpg` hecho en Windows con
      `ffmpeg -i video.mp4 -c:v mpeg1video -c:a mp2 -s 640x360 video.mpg` se ve y
      se oye en el Ryzen, sin desfase notable en 60 segundos.

- [ ] **S2 -- el protocolo ANTENA/1, escrito y con banco.** En codigo el
      2026-09-14: `platform/shared/bmo-antena` (HOLA, LISTA, ENTRADA, PIDE, VIDEO,
      NO; lista blanca con motivo por nombre, y el juntador de lineas que dice
      UNA vez una linea eterna y sigue). **Como se sabe:** `cargo test -p
      bmo-antena` en verde, con 20.000 lineas mutadas; y la antena de S3 habla
      exactamente lo que ese banco acepta.

- [ ] **S3 -- la antena, modo PEDIDO, en el movil.** En codigo el 2026-09-14:
      `toolchain/tools/antena/antena.py` (Termux: sirve UNA carpeta a UNA IP,
      convertida en vivo con `ffmpeg`) y `toolchain/tools/antena/cliente.py`, que
      hace de BMO-X desde Windows. **Como se sabe:** `python cliente.py <antena>
      v1 -s 20` deja un `prueba.mpg` que se reproduce en Windows.

- [ ] **S3b -- el movil por CABLE USB, sin router.** El movil en "anclaje por
      USB" se presenta como una tarjeta de red USB: RNDIS (casi todos los
      Android) o NCM. Es el unico escalon que toca el kernel, y va por partes.
      Pedido por Eddi el 2026-09-14 con su HONOR X7a enchufado. **Como se sabe:**
      `red perfil` ensena una segunda tarjeta, la del movil.

- [ ] **S3b.0 -- BMO-X dice QUE llego.** En codigo el 2026-09-14: el portero
      (`Ultra_kernel_x86-64/kernel/src/ring0/dev/usb/portero.rs`) nombra cada
      interfaz con `platform/shared/bmo-usbred` (`clase.rs`) en vez de "no es
      HID". **Como se sabe:** con el movil en el Ryzen y el anclaje por USB
      encendido, F11 dice `llego una RED POR USB (RNDIS...)` o `(NCM)`: esa foto
      decide cual de los dos drivers se escribe.

- [ ] **S3b.1 -- los mensajes, probados sin movil.** En codigo el 2026-09-14:
      `platform/shared/bmo-usbred/src/rndis.rs` (INITIALIZE, la MAC, el filtro y
      la cabecera de 44 de cada trama, con `DataOffset` contado desde su propio
      campo). **Como se sabe:** `cargo test -p bmo-usbred` en verde, con 20.000
      transferencias mutadas.

- [ ] **S3b.2 -- BULK en el xHCI.** En codigo el 2026-09-14 en
      `platform/drivers/usb/xhci/src/transferencia.rs`: los tipos 2 y 6 del
      contexto, `configure_endpoint` sin intervalo ni ancho periodico para ellos,
      y `queue_bulk`. SIN PROBAR EN METAL, y su prueba pide una pieza mas: un
      driver minimo de almacenamiento (Bulk-Only Transport, un INQUIRY).
      **Como se sabe:** un pendrive contesta a un INQUIRY de almacenamiento, que
      es BULK y no necesita ningun movil.

- [ ] **S3b.2a -- la FUGA de DMA del xHCI.** Medido el 2026-09-14: el HAL de
      `platform/drivers/usb/xhci/src/lib.rs` solo sabe `alloc_dma_pages`, nunca
      devolverlas, y `control_transfer` (`transferencia.rs`) pide una pagina NUEVA
      en cada transferencia con datos. Eso incluye cada cambio de LED del teclado
      (Bloq Mayus), y cada enchufe vuelve a pedir el anillo EP0 y sus dos
      contextos. Sin arreglar esto, un driver de red USB -- que habla por control
      a menudo -- se comeria la memoria. En codigo el 2026-09-14:
      `platform/drivers/usb/xhci/src/paginas.rs` da a cada ranura (y a cada
      endpoint que no esta vivo) SU pagina, pedida la primera vez y reutilizada
      despues, con banco que cuenta cien LEDs = una pagina. Toca el camino del
      teclado: se prueba en metal ANTES de seguir.
      **Como se sabe:** `save` da los mismos `marcos libres` antes y despues de
      100 pulsaciones de Bloq Mayus y de 10 enchufes del raton.

- [x] **S3b.2b -- el tope de una pagina en `control_transfer`.** HECHO el
      2026-09-14 en `platform/drivers/usb/xhci/src/transferencia.rs`: la etapa de
      datos copiaba `buf.len()` bytes en UNA pagina de DMA sin mirar el largo; un
      bufer de mas de 4096 habria escrito en la memoria fisica de al lado. Hoy el
      mayor era 512, y ahora uno mas grande se niega con su linea en el log.

- [ ] **S3b.2c -- HUBS.** `platform/drivers/usb/xhci/src/enumerar.rs` solo
      direcciona aparatos en los puertos RAIZ: no construye la ruta (route string)
      ni habla con un hub. Un movil, un teclado o un pendrive detras de un hub (o
      del hub de un monitor) no enumera. **Como se sabe:** un teclado enchufado a
      un hub escribe en el escritorio.

- [ ] **S3b.3 -- el driver RNDIS.** En `Ultra_kernel_x86-64/kernel/src/ring0/dev/usb`,
      con el control encapsulado por el endpoint 0 y las tramas por BULK.
      **Como se sabe:** el movil contesta a INITIALIZE y da su MAC.

- [ ] **S3b.4 -- la segunda tarjeta en el GATE RED.** `ring0/red` hoy conoce
      una sola NIC (la RTL8168); el pase y el grifo tendrian que elegir por cual
      salir. **Como se sabe:** `red ip` pide IP al movil por el cable USB.

- [ ] **S4 -- BMO-X pide y ve.** La app de S1 lee de la conexion en vez del
      disco, con la IP de la antena en `director.cfg` del disco de BMO (nunca en
      el repositorio: seccion 5 de `docs/plan/PLAN_RED_TX.md`) y rechazando
      cualquier otra IP. **Como se sabe:** un video pedido desde BMO-X se ve en
      el Ryzen mientras llega.

- [ ] **S5 -- el mando.** Pausa, seguir y parar desde el teclado de BMO-X; parar
      es cerrar la conexion. **Como se sabe:** la antena deja de enviar en menos
      de un segundo (lo imprime `antena.py`).

- [ ] **S6 -- modo ESPEJO (lejos).** App Android propia: captura de pantalla,
      codificador y canal de vuelta para teclado y raton (seccion 5). **Como se
      sabe:** la pantalla del movil se ve y se maneja desde el Ryzen.

---

# 4. EL PARECIDO CON STEAM, Y DONDE SE ACABA

```text
   Steam Link / Remote Play    el PC potente juega; el Deck, la tele o el movil
                               solo ensenan y mandan los botones
   este plan                   el MOVIL es el potente para la web; BMO-X ensena
```

** El Steam Deck en si es otra cosa: es un PC entero con SteamOS (Linux con
Proton). Traer eso aqui seria traer POSIX por la puerta de atras, que es justo
lo que la antena evita: lo sucio queda FUERA de BMO-X, en un aparato que ya lo
sabe hacer.

---

# 5. LO QUE NECESITA EL ANDROID

## Modo PEDIDO (S3): sin app

```text
   1. Termux, de F-Droid (la de Google Play esta abandonada y no actualiza)
   2. pkg install python ffmpeg
   3. termux-setup-storage          para poder leer ~/storage/movies
   4. termux-wake-lock              para que Android no la duerma a los minutos
   5. copiar antena.py al movil y:  python antena.py --carpeta ~/storage/movies
                                                     --permitir <IP de BMO-X>
```

** Convertir en vivo a 640x360 lo aguanta cualquier movil de los ultimos anios,
pero calienta y gasta bateria: mejor enchufado.

## Como llega "por cable"

```text
   adaptador USB-C a Ethernet   movil -> router -> Ryzen. BMO-X ya habla
   (RECOMENDADO)                Ethernet y DHCP: funciona el dia de S3
   WiFi                         movil por aire, Ryzen por cable, el router en
                                medio. Igual de facil; mas latencia
   Ethernet directo, sin router nadie reparte IPs: pide una IP fija en
                                `director.cfg`, que BMO-X aun no tiene
   USB directo (anclaje)        S3b: BMO-X tiene que aprender la tarjeta de red
                                USB. El unico que toca el kernel
```

## Modo ESPEJO (S6): una app de verdad

```text
   Android Studio + Kotlin      un servicio en primer plano, con su notificacion
   MediaProjection              captura la pantalla; Android pide permiso en
                                CADA sesion, y eso no se puede saltar
   AudioPlaybackCapture         el audio (Android 10+). [!] Cada app puede
                                prohibir que la graben, y muchas lo hacen
   FLAG_SECURE / DRM            lo protegido sale NEGRO en la captura: es asi a
                                proposito y no se rodea
   el codificador               Android no trae MPEG-1: o se lleva uno en software
                                (FFmpeg compilado con el NDK: pesa y calienta) o
                                BMO-X aprende H.264, que es otro decodificador
```

---

# 6. POR QUE ADMINISTRA ESTRATOS, Y FAT32 SOLO ES LA PUERTA

Eddi: *"que ESTRATOS sea el que administra los archivos, y que BMO-X diga por
que"*. Los dos volumenes estan en el mismo disco y NO hacen lo mismo:

```text
                  ESTRATOS (el de BMO-X)            FAT32 (DATOS)
   escribir       copia lo que cambia: el arbol     sobreescribe: lo de antes
                  de ayer sigue entero               se pierde
   versiones      `historial`, `vuelve N`, marcas    ninguna
   firmas         `:firma` por fichero               ninguna
   quien lo lee   solo BMO-X                         BMO-X y Windows
   su papel       ADMINISTRA: aqui vive lo que       la PUERTA: por aqui entra y
                  importa                            sale lo que viene de Windows
```

** Por eso un video de la antena o de Windows **entra por FAT32 y se queda en
ESTRATOS**: se copia una vez (`copiar`, que ya no tiene techo de tamano) y a
partir de ahi tiene historial, no se pisa por accidente y se puede firmar.
FAT32 es donde lo dejas; ESTRATOS es donde vive.

[!] Lo que hoy frena ese reparto, dicho: leer el contenido desde Ring 3 (S1a) y
que la Biblioteca mire ESTRATOS (S1b). Y el tope de **36 entradas por carpeta**
de ESTRATOS: una carpeta de videos pasa de ahi enseguida, asi que se reparten en
subcarpetas hasta que el 1.3 de
`platform/drivers/storage/estratos/ESTRATOS.md` lo levante.

---

# 7. EL AISLAMIENTO: USB y ANTENA, por categoria y por que

Eddi: *"intenta aislar TODO en USB, como siempre en categoria y por que, pero
MAS ESTRICTO ANTENA"*.

## USB -- REGLA 0: TODO NEGADO

La politica vive en `platform/shared/bmo-usbred/src/politica.rs` (con banco que
recorre las 256 clases) y el portero la dice en F11 al enchufar:

```text
   MANOS    teclado y raton        entra solo
   SONIDO   audio USB              solo cuando se pide
   RED      RNDIS / NCM / ECM      NUNCA sola: orden explicita, UNA a la vez, su
                                   corral prestado en el titular del DMA
   PASO     hubs                   solo el paso
   ALMACEN  discos                 NEGADO (el dia que haya driver: solo lectura)
   MOVIL    MTP y ADB              NEGADO SIEMPRE: la puerta a sus ficheros y shell
   OJOS     camaras                NEGADO
   OPACO    lo que no dice que es  NEGADO
```

** El movil de la antena, enchufado por USB, **no le da nada a BMO-X salvo su
red**, y ni eso sin orden. Su MTP y su ADB estan negados aunque el dueno los
pida: no son una red, son la llave del movil.

## ANTENA -- estricta por los dos lados

```text
   BMO-X (`bmo-antena`, `Conversacion`)   el ORDEN tambien es lista blanca: una
                                          linea valida en el momento equivocado
                                          cierra la conversacion; 72 lineas sin
                                          video, cierra; un byte de mas del
                                          video declarado, cierra
   la antena (`antena.py`)                UNA IP, UNA conexion, UN video; saludo
                                          en 10 s y charla en 120; 72 lineas;
                                          lo que no es ANTENA/1 se cuelga sin
                                          contestar; solo ficheros de DENTRO de
                                          su carpeta; y ffmpeg solo lee
                                          ficheros (`-protocol_whitelist`)
```

---

# 8. LA VISION: de un movil a una ANTENA de verdad (2026-09-14)

Eddi: *"que la ANTENA se enfoque en MASTICAR, y BMO-X recibe lo que ella
EMPAQUETA, pidiendolo por el router principal. Y una version 2: una distro Arch
Linux convertida en antena, ultra optimizada... o el PCI en RISC-V u otro chip,
eso ya depende de empresas. Y si la antena se lleva Python para que BMO-X lo
ejecute?"*.

## La idea de fondo: masticar y empaquetar

La antena no "le pasa Internet" a BMO-X: le entrega **paquetes que BMO-X ya sabe
digerir**, cada uno en el formato mas simple que sirve para lo suyo:

```text
   lo pesado de fuera            lo que mastica la antena     lo que recibe BMO-X
   un video de cualquier codec   ffmpeg                       MPEG-1 + MP2 (pl_mpeg)
   una pagina web                la web, JavaScript           texto / gemtext
   una imagen (JPEG, WebP, AVIF) el decodificador             QOI o BICO (ya se pintan)
   un audio (Opus, AAC)          el decodificador             PCM (el tubo de audio)
   un calculo en Python          CPython y sus librerias      el RESULTADO, no el codigo
```

Todo va por la LAN a traves del router de casa, que es el "principal": BMO-X y la
antena son dos maquinas de la misma red. **No por Internet**: por Internet haria
falta TLS, y eso es justo el muro (G6 de `docs/plan/PLAN_RED_TX.md`) que la
antena existe para quitar de en medio.

## Las tres versiones de la antena

```text
   V1  el MOVIL (hoy)           Termux + ffmpeg por software. Sirve para probar el
                                modelo; el HONOR X7a tira de 720p y se calienta
   V2  la ANTENA DEDICADA       un mini-PC (o un portatil viejo) con Arch Linux
                                reducido a UNA cosa: arranca directo al servicio de
                                antena, sin escritorio. Codifica por HARDWARE
                                (VAAPI/NVENC): 1080p en vivo, varios a la vez
   V3  la ANTENA DENTRO DE LA   una tarjeta PCIe con su propio chip (RISC-V u otro)
       CAJA                     y su propio sistema. Existe con otro nombre: DPU o
                                SmartNIC (un ordenador entero en una tarjeta de red)
```

** V3 es el mismo modelo que BMO-X ya tiene con la RTL8168 y el GATE RED -- un
aparato PCI y un anillo de mensajes -- pero con alguien LISTO al otro lado. Y trae
el riesgo mas serio de todo el plan: **una tarjeta con DMA es un maestro del bus**,
y sin IOMMU (E4 de `docs/plan/PLAN_RED_TX.md`) puede escribir en toda la RAM. Es el
NEUTRO en su forma mas pura (`NEUTRO/LEY.md`): una V3 sin IOMMU no se enchufa.

## Python en la antena: "BMO-X ejecuta Python" sin llevar Python dentro

Es inesperado y tiene sentido, con sus limites dichos:

```text
   como           BMO-X pide un TRABAJO con nombre ("resume este texto",
                  "grafica estos numeros"); la antena lo corre en su Python y
                  devuelve el RESULTADO (texto, numeros, una imagen QOI)
   lo que gana    todo el ecosistema de Python -- numpy, IA -- sin portar CPython
   la regla dura  NUNCA codigo arbitrario desde BMO-X: solo scripts de una lista
                  blanca y FIRMADOS con la misma ancla que firma los `.bex`
                  (`toolchain/tools/bmo-firmar`). Uno sin firma se niega por nombre
   lo que cuesta  1. es ejecucion REMOTA: la antena ve los datos que se le mandan
                  2. la latencia de la LAN en cada llamada
                  3. sin antena, el trabajo contesta NO por su nombre: BMO-X no
                     se vuelve dependiente, se vuelve CAPAZ cuando la hay
```

[!] No sustituye al Python nativo de BMO-X: son dos caminos. La antena da el
ecosistema entero ya; el nativo da Python sin nadie al lado.

## Los escalones de la vision

- [ ] **V2.0 -- la antena en un PC con Linux.** `toolchain/tools/antena/antena.py`
      sin cambios, en una maquina con codificacion por hardware. **Como se sabe:**
      sirve 1080p en vivo sin pasar del 50% de CPU.

- [ ] **V2.1 -- ANTENA/2: mas que video.** Verbos nuevos en
      `platform/shared/bmo-antena`: `PAGINA <url>` (texto), `IMAGEN <id>` (QOI) y
      `SONIDO <id>` (PCM), con la misma `Conversacion` estricta. **Como se sabe:**
      `cargo test -p bmo-antena` con los tres, y una imagen de la antena pintada
      en el Ryzen.

- [ ] **P1 -- TRABAJOS de Python firmados.** `EJECUTA <trabajo>` en ANTENA/2: solo
      nombres de una lista blanca, cada script firmado con `toolchain/tools/bmo-firmar`
      y verificado por la antena antes de correrlo. **Como se sabe:** un script sin
      firma contesta `NO sin firma`, y uno firmado devuelve su resultado a BMO-X.

- [ ] **V3.0 -- la tarjeta.** Solo despues de E4 (IOMMU) de
      `docs/plan/PLAN_RED_TX.md`. **Como se sabe:** `placa` dice AMD-Vi, y la
      tarjeta vive en un dominio propio con su corral mapeado y nada mas.

---

# 9. LAS METAS, REDEFINIDAS: la antena mastica, BMO-X presta (2026-09-14, noche)

Eddi: *"cuando el celular funcione se convierte en ANTENA para tener apps tipicas
como Google, porque mi celular MASTICA TODO y mi BMO-X es el core principal de
potencia real. Es la primera vez que Linux ES ANTENA -- Android es Linux. Y que
mi BMO-X le PRESTE su parte, pero si intenta pasarse de listo, BMO-X le corta y
se reinicia hasta aclarar"*.

## Lo que se aclara primero: el cable USB NO va primero

El camino por el ROUTER ya funciona en metal: BMO-X tiene IP (DHCP), hace ping al
router y a Internet. El movil por WiFi esta en esa misma red. **Lo que de verdad
falta para que se hablen es TCP en BMO-X** (G5 de `docs/plan/PLAN_RED_TX.md`), no
el cable. El cable USB (S3b) es COMODIDAD -- "conecto y ya" -- y es lo que mas
kernel pide: va en paralelo y no bloquea nada.

Por eso ESTRATOS no ensena el movil al enchufarlo, y es correcto: el movil ofrece
MTP y ADB, que son la LLAVE del movil y estan negados (seccion 7). BMO-X nunca va
a "abrir" el movil como un pendrive; el movil le HABLA por red.

## El reparto

```text
   la ANTENA (Linux: Android lo es)        BMO-X (el core)
   ------------------------------------    ------------------------------------
   la web, Google, codecs, apps, Python    la pantalla, el teclado, el disco con
   MASTICA: convierte a formatos simples   historial (ESTRATOS), la potencia
   es LISTA, y por eso no se fia de ella   es CELOSO: decide, presta y corta
```

## Las apps "tipicas" (Google y compania)

[!] BMO-X no ejecuta apps de Android, ni lo intentara: seria meter POSIX y el
mundo Android por la puerta de atras. Hay dos caminos, y el barato va primero:

```text
   VERBOS (C)    la antena usa la app o la web y devuelve el RESULTADO masticado:
                 BUSCA <texto> -> lineas de resultados; PAGINA -> texto. Barato
   ESPEJO (S6)   la pantalla del movil como video y el teclado de vuelta: la app
                 entera, tal cual. Caro (seccion 5) y el ultimo
```

## EL PRESTAMO: BMO-X presta, y corta si la antena se pasa de lista

**Lo que presta** son SERVICIOS CON NOMBRE, nunca "ejecuta lo que te mande":
contratos y formatos, no cerebros ajenos dentro de BMO-X.

```text
   ALMACEN    el movil guarda en ESTRATOS: copia con historial y firma, que el
              movil no tiene. Lo mas util que un Ryzen le da a un telefono
   CALCULO    trabajos de una lista de BMO-X (un hash, una firma; mas adelante
              INTI), con los datos que manda la antena, nunca su codigo
   PANTALLA   lo que ya es: ensenar lo que la antena mastico
```

Cada prestamo lleva **CUPO** fijado por BMO-X antes de empezar (tiempo de CPU y
bytes), UN trabajo a la vez, y **prioridad de fondo**: lo que Eddi tiene delante
gana siempre.

**Pasarse de lista**, dicho uno por uno: hablar fuera del protocolo, pasarse del
cupo, pedir un servicio que no se presta, pedir otro trabajo con uno en marcha,
traer un trabajo sin firma.

**Cortar y reiniciar hasta aclarar:**

```text
   falta 1..4   se corta la conexion y la puerta queda cerrada 1, 2, 4, 8 s
                (x2 cada vez, techo 5 min)
   falta 5      DESTERRADA: solo vuelve si el dueno lo dice en BMO-X
   aclarar      una conversacion entera sin faltas borra UNA falta
```

** "Reiniciar" es la SESION, nunca la maquina. Si una falta reiniciara BMO-X, la
antena tendria un boton para apagarte el PC: le bastaria con portarse mal.

## Las metas en orden, y por que ese orden

```text
   A  la antena madura sola     el movil sirve y Windows recibe. Separa problemas:
                                si A va, lo que falle despues es de BMO-X
   B  BMO-X pide                TCP (G5), el reproductor (S1a, S1), S4, S5. Es el
                                cuello de todo: sin TCP no hay nada que hablar
   C  la antena mastica mas     BUSCA, PAGINA, IMAGEN, SONIDO (V2.1) y Python (P1)
   D  BMO-X presta              ALMACEN y CALCULO, con la ley del castigo
   E  cable y lejos             S3b (USB directo), S6 (espejo), V2 y V3
```

## Los escalones nuevos

- [ ] **A1 -- la antena contesta a Windows.** La guia paso a paso es
      `toolchain/tools/antena/GUIA_MOVIL.md`. **Como se sabe:** `cliente.py` en
      Windows recibe la LISTA y un `prueba.mpg` de 10 s que VLC reproduce.

- [x] **PR0 -- la ley del castigo, pura y con banco.** HECHO el 2026-09-14 en
      `platform/shared/bmo-antena/src/cuarentena.rs`: `Cuarentena` (faltas,
      espera doblada, destierro, perdon del dueno) y `Cupo`. **Como se sabe:**
      `cargo test -p bmo-antena`, seis pruebas de la ley.

- [ ] **PR1 -- la ley, cableada.** La app de S4 lleva una `Cuarentena` por
      antena y la ensena en CABINA con el texto de la falta; `antena perdona` en
      el DIRECTOR es la unica salida del destierro. **Como se sabe:** una antena
      que manda una linea basura aparece en CABINA con "hablo fuera del
      protocolo" y no puede volver a saludar en 1 s.

- [ ] **PR2 -- PRESTA ALMACEN.** Un verbo `GUARDA <nombre> <bytes>` en
      `platform/shared/bmo-antena`, con cupo de bytes, que escribe en ESTRATOS.
      **Como se sabe:** una foto del movil aparece en ESTRATOS y `historial` la
      ensena.

- [ ] **PR3 -- PRESTA CALCULO.** `CALCULA <trabajo>` solo con nombres de la lista
      de BMO-X y cupo de CPU. **Como se sabe:** un trabajo que se pasa del cupo
      se corta y cuenta como falta.

- [ ] **V2.2 -- BUSCA.** El verbo de busqueda en `platform/shared/bmo-antena`,
      respuesta en lineas de texto. **Como se sabe:** `cliente.py` recibe los
      resultados de una busqueda hecha por la antena.

---

# 10. LOS DOS KERNELES: AOT aqui, JIT alli (2026-09-15)

Eddi: *"seria como tener 2 kernels: el kernel del celular vive masticando
Internet, Google y las apps famosas, y entrega BYTES LIMPIOS, no video. Mi BMO-X
vive TODO AOT; la antena vive GLOBAL de JIT, JVM, Java, Python, Go, y luego se
convierte en .bex para que mi BMO-X lo ejecute. Y que le preste RAM: la RAM del
celular y la RAM del PC"*.

## Esto ya existe, y lo tiene en el bolsillo

Todo movil lleva **dos ordenadores con dos sistemas distintos**: el de las apps
(Android, Linux) y el del modem, que corre su propio sistema en tiempo real. Se
hablan por mensajes en una zona de memoria acordada, y en los disenios modernos
el modem esta detras de una IOMMU **porque no se confia en el**.

```text
   el movil                        este plan
   ---------------------------     -------------------------------------------
   procesador de aplicaciones      la ANTENA: el mundo sucio, listo y cambiante
   procesador del modem            BMO-X: lo esencial, predecible, sin sorpresas
   memoria compartida + IOMMU      V3, la tarjeta PCIe (y por eso pide E4)
```

** La idea NO es rara: es el reparto que ya usa el aparato desde el que se pidio.

## Lo que SI se reparte y lo que NO: prestar CAPACIDAD, no DIRECCIONES

Aqui esta la trampa, con numeros de esta casa:

```text
   la RAM del Ryzen, por su bus      decenas de GB por segundo, ~80 ns de espera
   la LAN medida en este metal       ~10 Mbit (poco mas de 1 MB/s), ping 16 ms
```

Son **decenas de miles de veces** menos ancho y unas cien mil veces mas espera.
Si una direccion de memoria de BMO-X viviera en el movil, CADA lectura seria un
viaje por la red: un programa de un segundo tardaria horas. Eso es lo que hundio
a los sistemas de "memoria compartida distribuida" de los anios 90.

Lo que si funciona, y es lo que Eddi describe cuando dice *"entrega datos
limpios"*:

```text
   [x] cada RAM con SU trabajo    la del movil aguanta el navegador, la JVM, el
                                  modelo; la de BMO-X solo el resultado limpio
   [x] bloques terminados         llega un dato entero, no un puntero a otro sitio
   [ ] direcciones compartidas    NO por red. Solo tendria sentido en V3 (PCIe),
                                  donde las dos memorias estan en el mismo bus --
                                  y ahi manda `NEUTRO/LEY.md`: sin IOMMU, no entra
```

** Regla corta: **la antena presta CAPACIDAD, no direcciones**. Contratos y
formatos, nunca punteros ajenos dentro de BMO-X.

## Por que AOT aqui y JIT alli

```text
   un JIT necesita                  y eso en BMO-X significa
   -----------------------------    ------------------------------------------
   memoria que se escribe Y se      romper W^X, la regla que impide que un dato
   ejecuta                          se convierta en codigo
   un recolector de basura          pausas que nadie pidio, justo cuando el
                                    compositor tiene 16 ms
   un monton enorme y elastico      un asignador que crece sin techo
```

Por eso el reparto es limpio: **BMO-X es AOT entero** (`.bex` compilados antes,
firmados, sin sorpresas) y **la antena es el mundo JIT entero** (JVM, CPython,
Go, navegadores). Si la JVM de la antena se cae, BMO-X ni se entera: es otra
maquina.

## "y luego se convierte en .bex"

Hay una parte real y una parte que no lo es, dichas sin adornos:

```text
   [x] la antena COMPILA para BMO-X   es una maquina con disco, red y potencia:
                                      corre el toolchain, saca el .bex y lo FIRMA
                                      con `toolchain/tools/bmo-firmar`. BMO-X solo
                                      ejecuta lo firmado; lo demas se niega
   [x] Go o Java a nativo             existe (compilacion anticipada), pero da un
                                      binario que espera un sistema POSIX debajo:
                                      habria que portarlo a la superficie de BMO-X,
                                      no es apretar un boton
   [ ] traducir un programa de        NO. Un programa de Python no se vuelve `.bex`
       Python en vivo                 solo; lo que cruza es su RESULTADO (P1)
```

## Las apps famosas, una por una

```text
   Discord, WhatsApp, chats     DATOS: la antena habla con el servicio y manda
                                canales, mensajes y avatares; BMO-X los pinta en
                                SUS ventanas. Es "WhatsApp Web", pero el navegador
                                esta en el movil. Lo mejor de todo el plan
   YouTube y video              PIXELES ya masticados: MPEG-1 (S4)
   Steam y juegos               PIXELES en vivo: es ESPEJO (S6), y es Steam Link
                                tal cual. No hay "dato limpio" de un juego: un
                                juego ES una imagen por fotograma
```

[!] Cada servicio tiene sus condiciones de uso. Donde hay puerta oficial (Discord
la tiene) se usa esa; automatizar la web de un servicio que lo prohibe no se hace
en este arbol, y la antena no trae ninguna cuenta puesta.

## Los escalones nuevos

- [ ] **C1 -- APP/1, los verbos de DATOS.** En `platform/shared/bmo-antena`:
      `CANALES`, `MENSAJES <canal> <desde>`, `ENVIA <canal> <texto>` y `AVATAR
      <id>` (QOI), con la misma `Conversacion` estricta y su cupo. **Como se
      sabe:** los mensajes de un servicio con puerta oficial se leen en una
      ventana de BMO-X, y uno escrito en BMO-X llega al movil.

- [ ] **C2 -- la antena COMPILA (AOT remoto).** La antena corre el toolchain y
      devuelve un `.bex` firmado con `toolchain/tools/bmo-firmar`. **Como se
      sabe:** un fuente compilado en la antena corre en BMO-X, y el mismo `.bex`
      sin firma se niega por nombre.

- [ ] **V2.0b -- DESMONTAR el Arch.** La antena dedicada como electrodomestico:
      sin escritorio, raiz de solo lectura, UN servicio al arrancar, ningun shell
      escuchando por la red, y nada de cuentas guardadas que no sean suyas.
      **Como se sabe:** se apaga tirando del cable y al encender vuelve sola a
      servir, sin tocar un teclado.

---

# 11. LAS DOS VIAS: BMO-X habla PROTOCOLOS, la antena habla la WEB (2026-09-16)

Eddi: *"BMO-X puede tener 2 vias, no? La ANTENA, mi celular, procesa lo suyo,
pero BMO-X tendra lo suyo, que es para conectar con servidores principales. Son
2 elementos independientes: como tener 2 kernels, pero Linux enfoca en Internet
y caos -- aunque un Arch podria convertirse en antena profesional -- y BMO-X se
enfoca en AOT PURO"*.

## Si: son dos vias, y una de ellas YA EXISTE

La via directa no hay que inventarla: es `platform/shared/bmo-pila` (TCP/IP
propio, determinista, de lista blanca) y su escalera en
`docs/plan/PLAN_RED_TX.md`. La via de la antena es `platform/shared/bmo-antena`.
Las dos van por el MISMO tubo, y se separan arriba, en Ring 3:

```text
                 BMO-X (Ring 3)
   +-------------------------+    +-------------------------+
   |  VIA DIRECTA            |    |  VIA ANTENA             |
   |  protocolos propios o   |    |  ANTENA/1, APP/1        |
   |  de una pagina de RFC:  |    |  (bmo-antena): la web   |
   |  DHCP, DNS, ping, TCP,  |    |  masticada, con cupo y  |
   |  Gemini, HTTP simple,   |    |  cuarentena             |
   |  y TLS cuando llegue    |    |                         |
   +-----------+-------------+    +------------+------------+
               |                               |
               +------ bmo-pila (TCP/IP) ------+
                               |
                    GATE RED (kernel, E3)          <- no sabe cual es cual
                               |
                           RTL8168
```

** El kernel no distingue las vias, y es correcto: entrega tramas y se aparta
(`docs/maestro/RED_MAESTRO.md`). La diferencia entre las dos no esta en el tubo:
esta en QUE SE ACEPTA por cada una.

## La regla que las separa

```text
   VIA DIRECTA   habla PROTOCOLOS: cosas que caben en una pagina de RFC y que
                 bmo-pila puede aceptar por lista blanca. NUNCA JavaScript, NUNCA
                 un navegador. Lo que entra es un formato que BMO-X POSEE (un
                 `.bex` firmado, un `.mpg`, gemtext) o un dato de tamano fijo
   VIA ANTENA    habla la WEB: todo lo que pide un motor de navegador, sesiones,
                 codecs, Python. Lo que entra son DATOS en formatos simples, con
                 cupo, y la antena en cuarentena si se pasa de lista
```

"Servidores principales", dicho con nombres: un repositorio de `.bex` firmados,
otro BMO-X, un NAS de casa por un protocolo simple, la hora (NTP), una capsula
Gemini (G7). Es decir: **servidores que hablan el idioma de BMO-X o uno que cabe
en una pagina**. La web no cabe en una pagina, y por eso es de la antena.

## Independientes de verdad, y en que se parecen

```text
   SI  si la antena esta DESTERRADA, la via directa sigue: `red ping` no depende
       del movil
   SI  si la via directa no tiene TLS (G6), la antena sigue: ella lleva su TLS
   SI  comparten el tubo (bmo-pila, GATE RED, la tarjeta) y por tanto el ancho:
       una lamina bajando y un `.bex` bajando se reparten los mismos ~10 Mbit
   SI  NO comparten la confianza: por la directa entra lo FIRMADO; por la antena
       entra lo MASTICADO. Un `.bex` que llegue por la antena vale lo mismo que
       uno del pendrive: nada hasta que `bmo-firmar` lo reconozca
```

## El numero incomodo: hasta donde llega HOY la via directa

```text
   DHCP, ping, DNS           HECHOS en metal (2026-09-14): BMO-X ya toca Internet
   contenido (TCP, G5)       en codigo, probado en el anfitrion, NO en metal
   confidencial fuera de     TLS 1.3 (G6): meses. Hasta entonces la via directa
   casa (TLS, G6)            fuera de la LAN va EN CLARO
```

** Y aqui esta el matiz que vale: **un `.bex` firmado puede viajar en claro**.
La firma protege el contenido; TLS solo protegeria el canal. Por eso para el
repositorio de `.bex` la pieza que manda es `toolchain/tools/bmo-firmar`, que ya
existe, y no G6. Lo que NO puede viajar en claro es lo privado (una contrasena,
un documento) -- y eso, hasta G6, o se queda en casa o va por la antena.

## Arch como antena PROFESIONAL: que anade "profesional"

V2.0 y V2.0b (secciones 8 y 10) ya ponen el Arch desmontado como antena
dedicada. "Profesional" son tres cosas medibles, y ninguna es del movil:

```text
   codec por hardware        1080p en vivo sin calentarse (V2.0)
   navegador sin pantalla    Chromium sin cabeza existe en Linux: la LAMINA de
                             abajo sale de ahi con mas fidelidad que de un WebView
   varios BMO-X a la vez     una antena de casa para mas de una pantalla
```

Y el reparto que Eddi nombra queda escrito: **el caos vive donde ya sabe vivir**.
Linux lleva 30 anios aguantando la web, JITs y drivers de terceros; BMO-X no va
a competir en eso, va a ser el sitio donde el caos NO entra (AOT, firmado, sin
sorpresas). Dos kernels, cada uno en lo suyo.

## L1 -- LAMINA: navegar "por completo" sin ser navegador

Hoy el plan solo llega a "pagina como TEXTO" (seccion 8: `PAGINA` -> texto).
Navegar de verdad tiene exactamente TRES formas, no una:

```text
   TEXTO     la antena manda el texto de la pagina       barato; se pierde la forma
   LAMINA    la antena hace TODO el navegador (JS, CSS,  lo bueno: BMO-X pinta cajas,
             fuentes, layout) y manda la pagina YA       texto e imagenes con lo que
             MAQUETADA: rectangulos con coordenadas,     ya tiene (rasterizador,
             tiras de texto, imagenes en QOI/BICO        fuente.h, imagen.h)
   ESPEJO    pixeles del navegador del movil (S6)        caro y ciego: 160 Mbit sin
                                                         comprimir, o video con lag
```

La LAMINA es la que casa con `docs/plan/PLAN_MAQUETA.md`: MAQUETA *"emite las
coordenadas ya calculadas"*, y lo caro de CSS es el texto (metricas, kerning,
shaping). Eso lo calcula la antena; BMO-X recibe **la salida de un compilador,
nunca HTML**. L7 se respeta: el navegador no entra en BMO-X, entra su RESULTADO.
Hay precedente exacto: Opera Mini (2005) hacia justo esto con los servidores de
Opera; aqui el servidor es el movil de uno en su LAN, y esa es la diferencia de
confianza.

Lo que cuesta, con lo medido en esta casa:

```text
   [!] LAN ~10 Mbit, ping 16 ms: una lamina son decenas o cientos de KB, o sea
       0,1..0,5 s por pagina. Vale para "clic -> pagina". NO vale para hacer
       scroll por red: la lamina llega ENTERA (no solo lo visible) y BMO-X hace
       scroll y hover en LOCAL. Cada clic o tecla es un viaje mas el re-layout
       en el movil (cientos de ms en el HONOR X7a): se siente como Opera Mini
   [!] lo que NO cabe en LAMINA: animaciones JS, canvas, WebGL, Google Docs y
       parecidos. Eso es ESPEJO (S6) o nada. El video dentro de una pagina va
       por S4 (MPEG-1)
   [!] NO lo hace Termux: un motor de navegador entero no corre ahi. Hace falta
       la app Android con un WebView y un recorrido del DOM en JavaScript
       (getBoundingClientRect, estilos calculados, nodos de texto, imagenes
       reescaladas a su tamano en pantalla). Son cientos de lineas, y es LA MISMA
       app que S6 ya pide
   [!] la antena lo ve TODO: cookies, sesiones y lo que se teclee en BMO-X
       viajan en claro hasta el movil (seccion 1). Con una contrasena en un
       formulario deja de ser abstracto: la LAMINA no es para iniciar sesion en
       el banco
   [!] el "core de potencia real" se queda parado: la antena no presta
       POTENCIA, presta SUPERFICIE (TLS + JavaScript + codecs son meses de
       codigo, no ciclos). El Ryzen no gana calculando aqui; gana no teniendo
       que saber
```

El formato, en una linea por tipo, todo de tamano fijo y sin punteros
(`platform/shared/bmo-antena/src/lamina.rs`):

```text
   CAJA    x y ancho alto color                 un rectangulo relleno
   TEXTO   x y escala color <bytes>             una tira ya partida en lineas (escala 1..4 de 8x16)
   IMAGEN  x y ancho alto <id>                  el `id` se pide con IMAGEN <id> (QOI)
   ENLACE  x y ancho alto <id>                  zona clicable: CLIC <id> trae otra lamina
   CAMPO   x y ancho alto <id>                  zona de teclado: TECLA <id> <texto>
```

## La antena va DELANTE, no MANDA (2026-09-16, segunda vuelta)

Eddi: *"vamos a guiar que mi celular guie a mi BMO-X, pero si es para convertir
en antena lo mastique TODO por mi BMO-X; mientras eso pasa solo entregan
codigos procesados y BMO-X procesa todo en interior, historial. Es gracioso que
viva aislado"*.

Una palabra de esa frase hay que corregirla antes de que se convierta en
diseno: **"guiar"**. La antena va DELANTE -- es la que sale al mundo sucio, la
que explora, la que se ensucia -- pero no lleva el volante:

```text
   quien pide           BMO-X. Siempre. La antena no manda nada que no se pidio
                        (`Conversacion`: una linea fuera de orden CIERRA)
   quien decide         BMO-X: que se pinta, que se guarda, que se niega
   quien mastica        la antena, y entrega DATOS en formatos simples
   si mandara ella      una app del movil tendria el volante de BMO-X; la
                        cuarentena existe para que eso no pase ni por descuido
```

** Un explorador que vuelve con mapas, no un guia que te lleva de la mano.

## El movil, con sus numeros: 6 GB, RAM TURBO y MagicOS

```text
   6 GB de RAM           un WebView con una pagina y ffmpeg a 720p caben; el
                         navegador y el codec no van a la vez, y no hace falta
   RAM TURBO             es memoria VIRTUAL sobre el almacenamiento flash. Da
                         CAPACIDAD, no velocidad: cien veces mas lenta que la
                         RAM. Vale para no matar apps de fondo, no para masticar
                         mas deprisa. No cuenta como RAM para este plan
   MagicOS               es Android, y Android es un kernel Linux. Termux corre
                         SIN root y la app de L0/S6 es una app normal: nada de
                         este plan pide root, y root no se va a pedir
```

** Es la V1 de la antena: sirve para PROBAR el modelo. El escritorio del
Ryzen ya lo probo (abajo); el movil tiene que repetirlo con menos CPU.

## Lo que BMO-X hace "en interior": el HISTORIAL

Aqui esta la mitad que el movil no tiene y que hace que el reparto valga:

```text
   la antena          OLVIDA: la pagina de hace un minuto ya no esta
   BMO-X              RECUERDA: cada lamina, cada video, cada dato que entro,
                      queda en ESTRATOS con QUIEN lo trajo (el nombre de la
                      antena) y CUANDO (copy-on-write: escribir ES commitear)
```

Eso convierte el navegar en algo que BMO-X no tenia manera de dar: una
biblioteca de lo que se vio, firmada y con fecha, que no depende de que el
movil siga vivo. Es PRESTA ALMACEN (PR2) mirado desde el otro lado: no es solo
que el movil guarde en BMO-X; es que lo que la antena TRAE tambien se guarda.
Una lamina de 90 KB cabe donde cabe un `.mpg` de 20 MiB (S1a).

** Y "vive aislado" es exacto, y es la gracia: **aislado no es incomunicado**.
Por la unica ventana entran DATOS, nunca codigo; el que mira por ella (el
lector de `lamina.rs`) rechaza con nombre todo lo que no es del formato. Un
sistema que no puede ejecutar lo que le llega no necesita antivirus: necesita
un lector estricto, y eso es lo que se escribio hoy.

## MEDIDO el 2026-09-16: la lamina de una pagina real

`toolchain/tools/antena/lamina.js` corrio en el Chromium del escritorio (no en
el movil todavia) sobre un articulo de Wikipedia en espanol a 640 px:

```text
   sin metrica         22.117 px de alto, 2.149 lineas, 91,7 KB, 72 ms, y el
                       texto se SALIA por la derecha: la fuente del navegador es
                       proporcional y mas estrecha que los 8 px de BMO-X
   con metricaBMO()    16.407 px de alto, 2.106 lineas, 89,9 KB, 48 ms, CERO
                       tiras al borde, CERO rechazos del lector; 510 letras
                       Latin-1 (acentos, enie) y 4 `?`
   por la LAN          89,9 KB a 10 Mbit = 72 ms + 16 de ping: ~0,1 s
   el tope             4.096 lineas: un articulo largo usa la mitad
```

** El hallazgo: **la lamina no se arregla en BMO-X, se maqueta con su
metrica**. `metricaBMO()` inyecta Courier New a 13,33 px (mide exactamente 8x16)
antes de recorrer, y el navegador parte las lineas donde BMO-X las va a pintar.
Los rotulos "solo para lectores de pantalla" (1x1 px recortados) salian como
texto al borde; ahora se saltan.

## Los escalones nuevos

- [x] **L1a -- el formato, puro y con banco.** HECHO el 2026-09-16 en
      `platform/shared/bmo-antena/src/lamina.rs`: `Cabecera`, los cinco
      elementos, `Lector` (exactamente las `n` anunciadas; una mala cierra),
      `Fuera` y `Color` como rechazos nuevos. **Como se sabe:** `cargo test -p
      bmo-antena`, 30 pruebas, incluidas 20.000 lineas mutadas de las que nada
      que pasa se sale de la lamina, y la lamina real de example.com
      (`toolchain/tools/antena/ejemplo.lamina`) que entra entera con el parrafo
      cayendo de 16 en 16 px.

- [x] **L0a -- el recorrido del DOM, en un navegador de escritorio.** HECHO el
      2026-09-16: `toolchain/tools/antena/lamina.js` (`metricaBMO()` +
      `lamina()`), y `cliente.py --lamina f` juzga un fichero con las mismas
      reglas. **Como se sabe:** los numeros de arriba, y `python
      toolchain/tools/antena/cliente.py --lamina toolchain/tools/antena/ejemplo.lamina`
      dice `7 lineas, 308 bytes`.

- [ ] **L0 -- el recorrido, en el MOVIL.** Primero SIN app: Chrome en el
      HONOR, depuracion USB y `chrome://inspect` desde el PC, con `lamina.js`
      pegado en la consola remota (paso a paso en
      `toolchain/tools/antena/GUIA_MOVIL.md`, seccion L0). La app Android que
      lo hace sola es AA0 de `docs/plan/PLAN_NAVEGAR.md`. **Como se sabe:** el
      HONOR X7a saca la lamina del mismo articulo con CERO rechazos de
      `cliente.py --lamina`, y se apunta cuantos ms tardo el recorrido (el
      Ryzen tardo 48).

- [ ] **L1 -- LAMINA en BMO-X.** `PAGINA <url>` en `Conversacion` devuelve una
      lamina (cabecera + `n` lineas, con cupo de bytes), `CLIC <id>` / `TECLA
      <id> <texto>` de vuelta, `IMAGEN <id>` trae QOI. Quien la pinta es la app
      NAVEGAR y quien habla es el ANTENISTA: los dos con su escalera en
      `docs/plan/PLAN_NAVEGAR.md` (N0..N5), que es la propuesta maestra de la
      cara. **Como se sabe:** una pagina real llega maquetada, se pinta en el
      Ryzen, el scroll va en local sin tocar la red, un clic en un enlace trae
      la lamina siguiente, y `historial` la ensena.

- [ ] **D1 -- la via directa trae un `.bex` FIRMADO.** Despues de G5: un GET de
      HTTP/1.0 (una pagina de RFC, en `bmo-pila`) contra un servidor de la LAN
      que sirve ficheros, y el `.bex` pasa por `bmo-firmar` antes de correr.
      **Como se sabe:** el `.bex` bajado corre en BMO-X; el mismo sin firma se
      niega por nombre, y la antena en cuarentena no impide la bajada.
