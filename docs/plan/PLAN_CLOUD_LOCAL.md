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

- [ ] **S3b.2 -- BULK en el xHCI.** `platform/drivers/usb/xhci/src/transferencia.rs`
      sabe control, interrupcion de entrada e isocrono de salida; faltan los
      endpoints BULK de entrada y salida (tipos 2 y 6 del contexto). **Como se
      sabe:** un pendrive contesta a un INQUIRY de almacenamiento, que es BULK y
      no necesita ningun movil.

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
