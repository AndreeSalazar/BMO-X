# GUIA DEL MOVIL -- convertir el Android en ANTENA

> Paso A1 de `docs/plan/PLAN_CLOUD_LOCAL.md` (seccion 9). Escrita el 2026-09-14
> para el HONOR X7a de Eddi, vale para cualquier Android con Termux.
>
> **La meta de esta guia es UNA:** que el movil sirva un video y que Windows lo
> reciba. BMO-X todavia no entra: le falta TCP (G5 de `docs/plan/PLAN_RED_TX.md`).
> Probar primero con Windows separa los problemas: si aqui funciona, la mitad
> del movil esta TERMINADA y lo que quede es solo de BMO-X.

[!] PRIVACIDAD: las IPs de esta guia se escriben en la terminal y en ningun sitio
mas. Ni en el repositorio, ni en un commit, ni en una captura que se comparta.

---

## Lo que ya esta hecho (verificado el 2026-09-14 por el cable)

```text
   [x] Termux instalado, con python y ffmpeg
   [x] termux-setup-storage hecho (ya ve las carpetas del movil)
   [x] la carpeta bmo-antena en el almacenamiento, con antena.py y LEEME
   [ ] lamina_juez.py al lado de antena.py   <- NUEVO el 16-09: sin el, la
                                               antena no arranca (lo importa)
   [ ] un video en Movies               <- falta: habia 0
   [ ] la antena arrancada y contestando <- falta
```

** Por el cable, igual que antena.py: copiar `toolchain/tools/antena/lamina_juez.py`
a la carpeta `bmo-antena` del movil. Es el juez de las paginas, y la antena lo
usa antes de servir una.

---

## Paso 1 -- que Android no la duerma

En Termux:

```text
   termux-wake-lock
```

Y en Ajustes > Bateria > (Termux) > "Sin restricciones". Sin esto Android corta
la antena a los pocos minutos con la pantalla apagada.

## Paso 2 -- un video en Movies

Por el cable, desde el Explorador de Windows: copiar un video PROPIO o de licencia
libre a la carpeta `Movies` del movil (mp4, mkv, webm, mov, mpg o avi).

** La antena sirve lo que YA esta en su carpeta y no descarga nada (seccion 1 del
plan).

## Paso 3 -- las dos IPs (solo para ti)

```text
   la de Windows   en PowerShell:  ipconfig   -> "Direccion IPv4" del adaptador Ethernet
   la del movil    Ajustes > WLAN > la red conectada > detalles -> "Direccion IP"
```

Los dos tienen que estar en la MISMA red del router (el movil por WiFi vale).

## Paso 4 -- arrancar la antena

En Termux, con la IP de Windows (luego sera la de BMO-X):

```text
   python ~/storage/shared/bmo-antena/antena.py --carpeta ~/storage/movies --permitir <IP de Windows>
```

Se queda esperando. Dejar Termux abierto.

## Paso 5 -- pedirle desde Windows

En PowerShell, en la carpeta de BMO:

```text
   py toolchain/tools/antena/cliente.py <IP del movil>
```

Tiene que salir la LISTA con tu video y su id. Despues, 10 segundos de ese video:

```text
   py toolchain/tools/antena/cliente.py <IP del movil> <id> -s 10
```

Deja `prueba.mpg` en la carpeta. Abrirlo con VLC: si se ve, **A1 esta CUMPLIDO**.

---

## Si algo falla

```text
   lo que pasa                             por que                          arreglo
   la conexion se cierra sin contestar     --permitir no es la IP de        repetir el paso 3
                                           quien pide (la antena es asi
                                           a proposito: no le dice nada
                                           a un extrano)
   se queda colgado y da timeout           no estan en la misma red, o el   misma WiFi, no la
                                           router aisla los aparatos WiFi   de invitados
                                           ("aislamiento de AP")
   "ffmpeg: not found"                     falta ffmpeg                     pkg install ffmpeg
   "Permission denied" en storage          falta el permiso                 termux-setup-storage
   LISTA 0                                 Movies vacia o extension rara    paso 2
   se corta a los minutos                  Android la durmio                paso 1
   prueba.mpg de 0 bytes                   ffmpeg no pudo con el fichero    probar otro video;
                                                                            Termux dice por que
```

---

## Lo que el movil necesita para ser ANTENA DE PAGINAS (2026-09-16)

Eddi: *"antes de Navegar fijate en el celular, que necesita ANTENA"*. Exacto:
una ventana en BMO-X que dice "hace falta una antena" sin antena detras es un
cartel. Esto es lo que el movil tiene HOY para servir paginas, y lo que no:

```text
   HOY, con Termux    antena.py sirve las laminas (`.lamina`) que haya en su
                      carpeta, como `p1`, `p2`..., por el mismo `PIDE` que los
                      videos, y las JUZGA antes (una que BMO-X rechazaria sale
                      como `NO la lamina no vale: linea N: Fuera ...`).
                      Probado el 16-09 en Windows contra Windows: la lamina
                      llega byte a byte, la rota se niega con su coordenada
   las laminas        las hace el propio movil con `lamina.js` en su Chrome
                      (seccion L0 de abajo, por chrome://inspect) y se dejan en
                      la carpeta. Es A MANO: una pagina, una lamina
   lo que NO hay      una antena que reciba `PIDE <url>` y saque la lamina
                      SOLA. Eso pide un WebView, o sea la app Android (AA0 de
                      docs/plan/PLAN_NAVEGAR.md). Termux no trae un motor de
                      navegador y no se va a fingir con un analizador de HTML
                      escrito en Python: seria escribir un navegador, que es
                      justo lo que la antena existe para no hacer
```

** Con esto el orden queda claro: la antena de paginas A MANO ya existe y basta
para probar NAVEGAR de punta a punta (N2 y N3); la antena que navega SOLA es
la app, y es lo primero del movil que pide escribir codigo Android.

### Arrancar la antena con paginas

Las laminas van en la MISMA carpeta que los videos (`--carpeta`): la antena
lista los `.mp4`... como `v1, v2...` y los `.lamina` como `p1, p2...`.

```text
   python ~/storage/shared/bmo-antena/antena.py --carpeta ~/storage/shared/bmo-antena/carpeta --permitir <IP de Windows>
```

Y desde Windows:

```text
   python toolchain/tools/antena/cliente.py <IP del movil>        lista: v.. y p..
   python toolchain/tools/antena/cliente.py <IP del movil> p1     guarda pagina.lamina y la juzga
```

---

## Lo que viene despues (y NO es del movil)

```text
   A1  el movil sirve y Windows recibe          esta guia
   B   BMO-X pide en vez de Windows             TCP en BMO-X, luego el reproductor
   C   la antena mastica mas (BUSCA, IMAGEN)     antena.py crece; el movil no cambia
   D   BMO-X presta, con castigo si se pasa      la ley ya esta: bmo-antena/cuarentena
   E   por cable USB directo, sin router         el kernel: RNDIS y la segunda tarjeta
```

Cuando A1 este cumplido, el movil ya no pide nada nuevo hasta C.

---

## L0 -- la LAMINA sale del movil SIN escribir una app (2026-09-16)

Es la segunda cosa que el movil puede hacer, y no pide Termux: pide el
navegador que ya tiene y un cable. `lamina.js` corre DENTRO de una pagina
cargada y escribe la pagina ya maquetada; en el PC ya lo hizo (un articulo de
Wikipedia a 640 px: 2.106 lineas, 89,9 KB, 48 ms). Aqui se repite en el HONOR
para saber CUANTO tarda el movil, que es el numero que decide si la V1 sirve.

### Lo que hace falta

```text
   en el movil    Chrome (Play Store). El navegador de HONOR no expone la
                  consola por USB; Chrome si
                  Opciones de desarrollador: Ajustes > Acerca del telefono >
                  tocar 7 veces "Numero de compilacion"
                  Depuracion USB: Ajustes > Sistema > Opciones de
                  desarrollador > Depuracion USB
   en el PC       Chrome, y el cable USB del movil
   en el repo     toolchain/tools/antena/lamina.js y cliente.py
```

[!] Al conectar, el movil pregunta "Permitir depuracion USB desde este
ordenador?": si, y solo a ESTE PC. La depuracion USB es la llave del movil
(igual que ADB en la seccion 7 del plan): se apaga cuando se acaba la prueba.

### Paso a paso

1. En el movil, en Chrome, abrir la pagina de la prueba (un articulo de
   Wikipedia en espanol vale: tiene acentos, enlaces, imagenes y un campo).
2. Conectar el cable. En el Chrome del PC ir a `chrome://inspect/#devices`.
   El HONOR aparece con sus pestanas; pulsar **inspect** en la de la pagina.
3. Se abre la consola REMOTA: lo que se escribe ahi corre en el movil.
   Pegar el contenido entero de `lamina.js`, Enter. Luego:

```text
   metricaBMO()
   var t0 = performance.now(); var r = lamina({ancho: 640});
   Math.round(performance.now() - t0) + " ms, " + r.lamina.length + " bytes"
   copy(r.lamina)
```

4. `copy(...)` deja la lamina en el portapapeles del PC. Pegarla en un fichero
   `honor.lamina` y juzgarla:

```text
   python toolchain/tools/antena/cliente.py --lamina honor.lamina
```

### Lo que tiene que salir

```text
   cliente: lamina 640x<alto>, <n> lineas, <bytes> bytes (...)
            <cajas> caja, <tiras> texto, <imagenes> imagen, ...
```

Cero RECHAZADA. Y los milisegundos de la consola del movil se apuntan en
`docs/plan/PLAN_CLOUD_LOCAL.md` (L0): el Ryzen tardo 48 ms; el movil va a
tardar mas, y ESE numero es el que se queria.

### Si algo falla

```text
   el movil no aparece en chrome://inspect     depuracion USB apagada, o el
                                               cable es solo de carga
   la consola dice "lamina is not defined"     se pego a medias: pegar el
                                               fichero ENTERO otra vez
   RECHAZADA con "Fuera"                       la pagina cambio de ancho entre
                                               metricaBMO() y lamina(): pedir
                                               las dos seguidas
   RECHAZADA con "NoAscii"                     un control en el texto: es un
                                               fallo de lamina.js, y se apunta
                                               con la pagina que lo dio
```

** Lo que esto NO es: todavia no hay antena de laminas. Esto saca UNA lamina a
mano para medir. La antena que contesta `PAGINA <url>` sola es la app Android
(AA0 de `docs/plan/PLAN_NAVEGAR.md`), y esta prueba es la que dice si merece
la pena escribirla para este movil.
