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
   [ ] un video en Movies               <- falta: habia 0
   [ ] la antena arrancada y contestando <- falta
```

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

## Lo que viene despues (y NO es del movil)

```text
   A1  el movil sirve y Windows recibe          esta guia
   B   BMO-X pide en vez de Windows             TCP en BMO-X, luego el reproductor
   C   la antena mastica mas (BUSCA, IMAGEN)     antena.py crece; el movil no cambia
   D   BMO-X presta, con castigo si se pasa      la ley ya esta: bmo-antena/cuarentena
   E   por cable USB directo, sin router         el kernel: RNDIS y la segunda tarjeta
```

Cuando A1 este cumplido, el movil ya no pide nada nuevo hasta C.
