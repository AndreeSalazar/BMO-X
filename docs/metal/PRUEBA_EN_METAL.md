# PRUEBA EN METAL -- el arranque del 2026-08-12

Guia para el Ryzen. **No es una lista de deseos: es lo que hay que traer de
vuelta** para que el siguiente paso se decida con datos y no con teorias.

Han entrado **nueve commits y ninguno ha visto un CPU**. Dos de ellos tocan lo
que se nota en el primer segundo: **el camino de la entrada** y **el del
pintado**. Por eso el orden de abajo es *si esto falla, para*.

> La guia de la tanda anterior queda en `PRUEBA_EN_METAL_0810.md`.

---

# PARTE 0 -- El comando, y lo que se puede saber sin arrancar

```powershell
Ultra_kernel_x86-64\build.ps1 -Flash -Drive A -Data A
```

Si el build **para**, mirar en este orden: el guardian de ASCII (hay comentarios
nuevos en cinco ficheros), el guardian de contrato de syscalls, y el enlazado.
Ninguno de los tres deberia saltar -- `cargo check` esta limpio en kernel,
userspace y toolchain.

Y **antes de ir al Ryzen**, esto se corre desde Windows:

```powershell
cargo test -p bmo-verify --test ram_del_disco -- --nocapture
```

Imprime la tabla de transporte de todos los `.bex` recien desplegados. Si
`doom.bex` no sale, el despliegue no lo copio y no hace falta reiniciar nada.

---

# PARTE 1 -- LO QUE SE MIRA PRIMERO, porque para todo lo demas

## 1.1 -- Arranca?

Lo de siempre: escritorio pintado. Si **no** arranca, el sospechoso numero uno es
el corte de `dev/usb.rs` en cuatro modulos, o el hilo del bus:

```
   git revert 61a8fa2f     el corte en modulos
   git revert 58888f46     el hilo del bus
```

## 1.2 -- El hilo del bus late

En **F11**, fila `usb`, campo NUEVO:

```text
   bus=turns:overlaps
```

**`turns` tiene que SUBIR, y sobre todo mientras un programa de Ring 3 tiene la
entrada.** Es el numero que dice que el teclado ya no depende de que alguien
pregunte.

Y en el arranque, una linea nueva:

```text
   usb: el bus tiene hilo propio, tid =3
```

Si sale `NO hubo ranura para el hilo del bus` o `sin aparatos`, **el hilo no
esta** y el sistema se comporta como antes -- o sea, con el fallo de
congelacion.

[!] Si mas tarde aparece `FALLO usb: el hilo del bus DEJO DE LATIR`, eso es el
vigilante nuevo y significa exactamente lo que dice.

## 1.3 -- El parpadeo

**Mover el raton por el escritorio.** No tiene que parpadear.

Y el numero, con la orden `perf`:

```text
   fotogramas  ...
   media       ...
   peor        ...
   cajas       <- FILA NUEVA
```

- `cajas 2` o `3` con un `peor` pequeno -> el troceado trabaja.
- `cajas 1` con un `peor` de ~8 MB -> degenero, y el sospechoso es
  `COSTE_DE_UNA_CAJA` en `sin_gpu/sucio.rs`, no el volcado.

Vuelta atras: `git revert 758ab20f`.

---

# PARTE 2 -- EL TECLADO, que es el fallo que se sufrio

## 2.1 -- Desenchufar

Desenchufa el teclado. En F11 tienen que salir **DOS** lineas, no una:

```text
   AVISO usb: puerto: algo se DESENCHUFO =N
   AVISO usb:   ...y ERA UN APARATO MIO: lo suelto =N
```

★ **Sin la segunda, el olvido no ocurrio** y lo de abajo va a fallar.

## 2.2 -- Volver a enchufar

**Tiene que escribir.** Y en F11:

```text
   INFO usb: puerto: ENCHUFADO y adoptado =N
```

Si en vez de eso sale `puerto: ENCHUFADO, ya creo tenerlo todo` seguido de
`...creo tener teclado:raton =0b1_0000_0001`, el olvido fallo: el adoptador
cree que todavia tiene el teclado.

> Esa linea se llamaba `nada que adoptar` hasta el 2026-08-15, y juntaba TRES
> causas distintas bajo la misma frase. Desde entonces son tres mensajes: `ya
> creo tenerlo todo` (el fantasma), `ENCHUFADO pero CERRADO por intentos` (la
> puerta agotada) y `enumere y no era mio` (el unico normal). Ver la SEXTA
> VUELTA al final del documento.

Vuelta atras: `git revert 11d97e99`.

## 2.3 -- El rescate desde la puerta cruda

**Lanzar DOOM y volver con `Ctrl+Alt+Esc`.** Antes, desde un programa que lee
teclas crudas, no volvia.

```text
   AVISO input: entrada RESCATADA por el teclado =PID
```

★ Esto es lo que de verdad cierra el commit del hilo del bus: **funciona aunque
el dueno de la entrada este colgado**.

---

# PARTE 3 -- LO QUE SE COBRA DE UNA VEZ

## 3.1 -- Las unidades de CABINA

Ya no hace falta convertir a mano. En F11:

```text
   red:  MAC                             =2C:F0:5D:D9:3C:E3
   red:  PHYstatus crudo                 =0b1011
   red:  enlace ARRIBA, megabits         =100        <- antes salia 64
   arch: archivo REFLEJADO para leer     =4.0 MiB (4196020)
   usb:  el bus tiene hilo propio, tid   =3
```

Si alguna sale en hexadecimal pelado, esa llamada no se migro -- no es un fallo,
es una que falta.

## 3.2 -- `smp`

```text
   smp stop      -> "obreros parados" + "[!] seguiran contando como en pie"
   smp           -> "12 de 12" + "[!] pero estan PARADOS"
   smp all       -> despierta
   smp test      -> TIENE QUE VOLVER A ACELERAR
```

★ Lo ultimo es lo que importa: antes de `11d97e99`, un `smp all` tras un `stop`
habria dado 12 en pie y **cero obreros**, sin decir por que.

## 3.3 -- La red RECIBE

```text
   net rx        -> "receptor ARMADO, anillo en la fisica =0x..."
   (esperar unos segundos)
   net rx        -> "red: trama de 2CF0..." con tipo 0806 (ARP) u 0800 (IPv4)
```

[!] **Cero en la primera vuelta es lo esperado**, no un fallo: el anillo se acaba
de armar y el broadcast llega cada pocos segundos.

Si NUNCA sube: `la NIC no termina su reset` (el BAR o el aparato) o `sin marco
para el anillo` (memoria). Si sale `trama demasiado corta`, llegan bytes y el
sospechoso es el descuento del FCS.

Vuelta atras: `git revert abd9cf1c`.

## 3.4 -- El audio dice como quiere las muestras

Con el audifono **enchufado antes de arrancar**:

```text
   audio
```

Y en F11, los numeros del paso 0:

```text
   audio: interfaz AudioStreaming, alt        =1
   audio: canales                             =2
   audio: bits por muestra                    =16
   audio: bytes por trama (wMaxPacketSize)    =192 B
   audio: frecuencia que acepta               =48000
   audio: frecuencia elegida                  =48000
   audio: y una trama suya ocupa              =192 B
   audio: el endpoint isocrono es el DCI      =2
```

★★ **Las dos ultimas deciden si el plan de audio es posible**: la trama tiene que
CABER en el paquete. Si sale `ninguna frecuencia suya cabe en su propio paquete`,
no hay codigo correcto que lo arregle.

Si no aparece nada: `puertos libres mirados, y ninguno reproduce =N`. Con el
audifono enchufado y `N > 0`, el aparato esta y **no es UAC1 como se creia** --
lo cual tambien es una respuesta, y cambia el plan.

---

# PARTE 4 -- DOOM

`run apps/doom.bex`. Lo ultimo que se supo es que **pasa de `M_LoadDefaults` y
muere despues**. Lo que hace falta es **donde**:

| Sintoma | Sospechoso |
|---|---|
| no sale nada | el reflejo de ficheros -- `git revert cf878698` |
| se para y no sale `W_Init` | el WAD otra vez; mirar `arch` en CABINA |
| arranca y muere sin pintar | el monton: 12 MiB CONTIGUOS. CABINA dice si el kernel los nego |
| pinta y no responde | mirar si `bus=turns` sigue subiendo |
| anda solo y no para | la cola cruda: se perdio un `soltar` |

---

# QUE TRAER DE VUELTA, en orden de utilidad

1. **`A:\datos\salida.txt`** -- se llena solo con lo que se lanza desde
   `Ejecutar`, y `guarda` vuelca el historial entero. **Vale mas que cualquier
   foto**: se puede leer, buscar y comparar.
2. **Foto de F11 (CABINA)**, con el filtro `A` para la ultima accion o sin
   filtro para el historial.
3. **Foto de la fila `usb`** completa: ahi van `bus=`, `apk=` y `kev=`.
4. **La salida de `perf`**, por la fila `cajas`.
5. **La salida de `audio`**, que es la unica que no tiene precedente.

Y si algo se cuelga antes de poder escribir: **la foto de lo ultimo que quedo en
pantalla sirve igual**. CABINA se pinta desde el bucle del shell, asi que lo que
se ve es lo ultimo que el sistema alcanzo a contar.

---

# LOS NUEVE COMMITS, y su vuelta atras

| commit | que toca | si algo falla |
|---|---|---|
| `58888f46` | **el camino de entrada de todo** | teclado mudo |
| `abd9cf1c` | la NIC (solo con `net rx`) | nada, es opt-in |
| `adfbcd20` | como se pintan los numeros de CABINA | lineas raras |
| `11d97e99` | teclado replug + smp | el teclado no vuelve |
| `61a8fa2f` | **corte de `usb.rs`** (cero logica) | no arranca |
| `758ab20f` | **el pintado del compositor** | parpadeo o basura |
| `34ddeb4a` | solo mover un fichero | nada |
| `8c1f5ab4` | la orden `audio` | nada, es opt-in |
| `af285731` | solo toolchain | nada en metal |

★ Los dos en negrita son los unicos que pueden dejar la maquina inservible. Los
demas o son opt-in o solo cambian texto.

---

# SEGUNDA VUELTA -- 2026-08-12, tarde

El arranque del mediodia ya contesto varias cosas y **esas no se repiten**. Lo
que sigue son SOLO las preguntas abiertas, mas las cuatro filas nuevas.

## Ya contestado, y no hay que volver a mirarlo

| | |
|---|---|
| Arranca, escritorio pintado | OK |
| El corte de `usb.rs` en cuatro modulos | OK, arranco |
| Desenchufar suelta el aparato | OK -- salio `...y ERA UN APARATO MIO: lo suelto` |
| Volver a enchufar lo re-adopta | OK -- salio `puerto: ENCHUFADO y adoptado` |
| El raton sobrevive a todo | OK |
| La escritura a disco | OK -- `archivo guardado` |
| Cerrojos con once nucleos | **ni un choque** |

## 1. EL TECLADO -- una sola linea la contesta entera

Desenchufar y volver a enchufar. Lo que hace falta es la linea NUEVA que sale
pegada a `ENCHUFADO y adoptado`:

```text
   usb:   ...y su bomba encolada k:r     =0b1_0000_0001
```

- El **primer** bit (el alto) es el teclado. `1` = tiene transferencia encolada.
- Si sale `0`, ademas saldra en ambar:
  `...pero el TECLADO quedo MUDO: sin transferencia encolada`

★ **Con eso se sabe si el teclado esta enumerado-y-mudo o si el problema esta mas
abajo.** Son dos sitios distintos del driver y hasta ahora se veian igual.

## 2. LAS CUATRO FILAS NUEVAS DE `info`

```text
   info
```

```text
   mide    frecuencia real + consumo   (lo declara el perfil)
   tsc     3.70 GHz   (medido)
   ahora   4.61 GHz   (boost)
   gasta   88.4 W paquete / 61.2 W nucleos   (el resto: fabric + memoria + L3)
```

Y **`info` dos veces seguidas**: `ahora` y `gasta` tienen que salir DISTINTOS.
Son medidas por diferencia -- si salen identicas, no se estan recalculando.

[!] Si `mide` dice `nada: este perfil no declara sensores`, para ahi: el resto de
las filas no significa nada y el motivo esta en CABINA del arranque.

## 3. LA SECUENCIA QUE CIERRA AXION -- tres lecturas

```text
   info        <- apunta los vatios
   smp all
   info        <- TIENEN QUE SUBIR (once nucleos girando en vacio)
   smp stop
   info        <- y BAJAR
```

★★ Esto convierte la seccion 5 de este documento --*"un obrero que espera GIRA y
consume como si trabajara"*-- de afirmacion en medida. **Es el numero que decide
si MWAIT vale la pena o no.**

## 4. `smp test` SIN `stop` delante

La vez anterior el `stop` estaba puesto, y por eso salio `11 entraron, 0 vieron
la ronda`. La prueba limpia:

```text
   smp all
   smp test     <- tiene que ACELERAR
```

Si con esto tampoco acelera, entonces si hay un bug y los tres testigos dicen en
que tramo.

## 5. `audio`, y la linea que falta

```text
   audio
```

Lo que hace falta es la ultima linea:

```text
   audio: puertos libres mirados, y ninguno reproduce =N
```

- **N = 0** -> el fallo es MIO: solo se miran puertos que `uhid` no tomo, y el
  audifono quedo marcado.
- **N > 0** -> se miro y el audifono no es UAC1 como se creia. Tambien es una
  respuesta, y cambia el plan del paso 0.

## 6. DOOM -- una foto vale mas que cualquier texto

`run apps/doom.bex`, y **la foto de como se ve la pantalla rota**. Con eso se
distingue a ojo si es la transicion al devolver la pantalla o si es de DOOM: el
troceado por regiones **no toca a DOOM**, que pinta con su propio blit.

---

## Resumen: seis cosas, y dos son una foto

1. `bomba encolada k:r` tras reenchufar el teclado
2. `info` dos veces
3. `info` / `smp all` / `info` / `smp stop` / `info`
4. `smp all` + `smp test` sin stop
5. la ultima linea de `audio`
6. la foto de DOOM

Y como siempre: **`A:\datos\salida.txt` vale mas que las fotos** para todo lo
que sea texto.

---

# SEXTA VUELTA -- 2026-08-15: LAS PUERTAS SIEMPRE ABIERTAS

El dueno lo reporto asi: *"el teclado y mouse cuando entre la BIOS o algo causo
un pequeno bug... es como que al teclado se le olvido, o otras veces mouse y
teclado se olvido"*. Con la foto de CABINA delante:

```text
   info usb  puerto: ENCHUFADO, nada que adoptar     =4
   info usb    ...y creo tener teclado:raton         =257
```

`257` es `0x101`: **"tengo los dos"**, con el dueno mirando un teclado que no
escribe. No era un bug: eran tres puertas que se cierran solas.

| | lo que pasaba | reparado por |
|---|---|---|
| 1 | El aviso de puerto era un buzon de UNA plaza. Un desenchufe y un enchufe seguidos se fundian en "conectado" y la desconexion no ocurria jamas | `bmo_xhci::avisos`, una cola FIFO |
| 2 | El kernel recogia UN aviso por bombeo (`if let`) | `while let`, hasta 4 por vuelta |
| 3 | Nada comparaba lo que el driver cree con lo que dicen los puertos: un aviso perdido = puerta cerrada hasta reiniciar | `bmo_uhid::barrido`, cada 500 ms |
| 4 | `MAX_PUERTOS = 16` cerraba el hot-plug de los puertos altos en silencio | 32 |

## 1. Lo primero: la fila `puertas` de CABINA (F11)

```text
   usb ... bus=T:O puertas=esperando:PERDIDOS:barridos:reparados
```

- `PERDIDOS` tiene que ser **0**. Si sube, la cola de avisos se lleno.
- `barridos` tiene que **subir siempre** (dos por segundo). Si esta pegado, el
  TSC no esta medido y la red no existe.
- `reparados` deberia ser **0** en una sesion sana. Si sube, el sistema se esta
  arreglando solo -- y cada uno de esos es medio segundo en que el teclado no
  respondia. **Es el numero que contesta si el fallo del dueno sigue vivo.**

## 2. La prueba que reproduce el fallo reportado

Desenchufar el teclado **y volver a enchufarlo rapido, sin esperar**. Antes eso
era justo lo que fundia los dos avisos en uno. Ahora tienen que salir los dos:

```text
   AVISO usb: puerto: algo se DESENCHUFO =N
   AVISO usb:   ...y ERA UN APARATO MIO: lo suelto =N
   info  usb: puerto: ENCHUFADO y adoptado =N
```

★ **Si sale el par completo, la cola funciono.** Si falta el DESENCHUFO pero el
teclado vuelve a escribir medio segundo despues, funciono la RED en vez de la
cola -- y entonces `puertas` marcara `reparados=1`, que es la senal de que
todavia se pierde algun aviso.

## 3. Y la que prueba la red de verdad

Entrar en la BIOS, salir, y arrancar BMO. Si algun aparato no enumera en el
arranque, el barrido tiene que recogerlo **solo, sin tocar nada**, dentro del
primer segundo:

```text
   info usb: BARRIDO: adopte lo que un aviso perdido dejo fuera =1
```

O, si lo que quedo fue un fantasma:

```text
   AVISO usb: BARRIDO: habia un FANTASMA, lo solte =1
```

★ **Esto es lo que se pidio**: que no haga falta reiniciar para recuperar un
teclado. Si sigue haciendo falta, el numero que lo delata es `reparados`.

## 4. Lo que NO puede pasar, y hay que mirarlo aposta

El barrido corre solo, dos veces por segundo, y lo que toca es el bus. Las dos
reglas que lo sujetan estan en pruebas (`barrido::tests`), pero el metal manda:

- **El teclado no puede morirse mientras escribes.** Un barrido que resetee el
  puerto de un aparato vivo es el bug del 2026-07-31 repetido 250 veces por
  segundo. Escribir un rato largo con el escritorio abierto.
- **No puede tironear.** Adopta como mucho UNA vez por barrido. Si el arranque
  da tirones de un segundo, es esto y hay que subir `BARRIDO_PERIODO_MS`.

---
---

# TANDA DEL 2026-08-17 (tarde) -- EL DISCO YA SABE DEVOLVER

Compila y **ningun CPU lo ha ejecutado**. Es la primera vez que BMO-X manda un
comando de disco que no es leer, escribir, IDENTIFY o FLUSH -- y el unico
destructivo de los cinco.

> El porque y los cuatro guardianes: `docs/componente/EL_DISCO_EXIGE.md`,
> seccion 12.1.

## 1. Antes de tocar nada: `disco`

En el terminal del escritorio (Ctrl+Alt), la palabra sola:

```text
   disco
```

Tiene que contestar tres bloques y ninguno inventado:

```text
   medio     ESTADO SOLIDO           lo de la tanda de esta manana
   trim      si                      la palabra 169
   volumen   montado   generacion N
   libre     4xx.x GiB
   devuelto  nada todavia en esta sesion
```

★ Si `trim` dice **no**, para: el resto de esta parte no aplica a este aparato y
la propuesta se va a negar sola (y eso tambien es un resultado correcto).

## 2. La propuesta, que NO manda nada

```text
   disco trim
```

Tiene que salir la cola libre en GiB, **desde que bloque y desde que LBA**
empieza, cuantos sectores son y cuantas ordenes va a costar:

```text
   cola libre    413.9 GiB   desde el bloque 27
   sectores      867543048 de 512 B   desde el LBA 2265088
   ordenes       207   (el disco admite 1 bloque(s) por orden)
```

★ Esos numeros **no son una cuenta parecida**: los sirve la misma funcion del
kernel que va a ejecutar el recorte (`INFO_DISCO_COLA_LBA` / `..._SECTORES`), y
la cifra de ordenes sale de la palabra 105 del disco, no de una suposicion.

[!] **Si aqui aparece un numero absurdo** --mas GiB que el volumen, un bloque de
inicio mayor que el total, o un LBA por debajo del principio de la particion--
**para y no escribas `ya`**. Ese numero es literalmente el que va a usar la orden.

## 3. La orden

```text
   disco trim ya
```

Lo que tiene que pasar:

- Sale `mandando el recorte (esto tarda)...` **antes** de la espera. Si el
  escritorio se congela sin ese renglon, el aviso llego tarde.
- Termina con `DEVUELTO: <tamano>` y el numero de ordenes.
- **F11 (CABINA)** lleva el relato: `recorte de la cola libre pedido por un
  proceso de Ring 3`, `sectores devueltos al disco` y `ordenes DATA SET
  MANAGEMENT`.

Y despues, `disco` otra vez: la fila `devuelto` ya no dice *nada todavia*.

## 4. LO QUE DE VERDAD SE ESTA PROBANDO -- reiniciar y leer

Recortar la cola libre **no debe cambiar ni un dato**. La prueba es esta y no la
de arriba:

```text
   reboot
   disco            generacion la MISMA, libre lo MISMO, montado
   ls               los ficheros de siempre, y `cat` de uno cualquiera
```

★★ Si despues de un recorte el volumen no monta, o un fichero sale a ceros, **el
rango estaba mal** y lo que hay que traer de vuelta es el bloque de inicio que
dijo la propuesta. Ese es el unico fallo que este camino puede tener y por eso
la propuesta lo imprime antes.

## 5. Los tres NO, que valen tanto como el si

Cada uno prueba una puerta distinta, y ninguno debe colgar nada:

```text
   sin volumen montado  ->  "no hay volumen ESTRATOS montado, o su cola esta vacia"
   disco no armado      ->  "sin permiso: gate de identidad o ventana"
   disco sin TRIM       ->  "este disco NO declara TRIM (palabra 169)"
```

## 6. La barrera, de paso

```text
   disco barrera
```

Verde = el disco confirmo el `FLUSH CACHE`. Es la orden mas barata de comprobar
y la que sostiene el sellado de ESTRATOS entero.

## 7. LO QUE HAY QUE TRAER DE VUELTA -- cinco cosas y ninguna es una impresion

Esta guia no pide "a ver si va". Pide **numeros que decidan lo siguiente**:

```text
   1  `disco` ANTES        la foto entera: medio, trim, cola libre, devuelto=0
   2  `disco trim`         los tres numeros de la propuesta (GiB, LBA, ordenes)
   3  `disco trim ya`      lo que contesto, palabra por palabra
   4  `disco` DESPUES      la fila `devuelto`: sectores y ordenes
   5  DESPUES DE REINICIAR generacion, libre, y un `cat` de un fichero
```

**La forma barata de traerlas todas**: `save` despues de cada una. Vuelca la
salida a `datos\salida.txt`, que esta en la FAT32 -- se enchufa el disco a
Windows y se abre con el bloc de notas. Una foto de la pantalla vale para el
punto 3 si algo sale en rojo, pero el texto se puede diferenciar contra el del
dia siguiente y una foto no.

★ **Y si algo falla, lo que decide es F11 (CABINA)**, no la linea roja del
terminal: ahi va el motivo del aparato, el LBA donde se quedo y las dos cuentas
del recorte. `save` tambien lo guarda.

[!] Lo que **no** hace falta traer: nada del rendimiento. Cuanto tarda el
recorte no importa hoy -- se pide una vez, a mano, y por una persona.
