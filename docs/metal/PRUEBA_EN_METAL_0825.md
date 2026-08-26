# QUE TECLEAR EN EL RYZEN -- tanda del 2026-08-25

> La regla de esta carpeta, de la primera hoja y sin cambios:
>
> *"Cada prueba dice **que afirma** y **como se cae**. Una prueba que solo puede
> salir bien no prueba nada -- si no se sabe de antemano que aspecto tiene el
> fallo, cualquier cosa que aparezca en pantalla se lee como exito."*
>
> Y la de orden: **lo que no toca nada va primero, lo que no se deshace va al
> final.**

---

# 0. LO QUE HAY QUE SABER ANTES DE ARRANCAR

## 0.0 -- ★★ LO QUE SE ACUMULO SIN PISAR EL METAL, EN UNA LISTA

Esta tanda no es de un commit: son **once** que nadie ha ejecutado. Van aqui
juntas para que, si algo sale mal, se sepa de un vistazo que hay dentro.

| # | que entro | como se ve si esta bien | si falla |
|---|---|---|---|
| 1 | **W^X** (`PTE_NX`) | arranca | ⛔ pantalla negra |
| 2 | **SMAP** | arranca | ⛔ `#PF` en el primer syscall que copie |
| 3 | los cuatro bits de guardia | `ext` -> cuatro `Yes` | fila en `No` |
| 4 | **la topologia careada** | `cpu` -> `6 / 12 (2 por nucleo, MEDIDO)` | fila `[!] duda` |
| 5 | **`cabina`** en el escritorio | `cabina` pinta el anillo | orden desconocida |
| 6 | la reloc dentro de su seccion | DOOM se juega | ⛔ DOOM no carga |
| 7 | **el gate de AUTORIA** | nada cambia (todo `sig_algo=0`) | un `.bex` no admite |
| 8 | **la AUTORIDAD** (C4) | `sonda` niega 8 y 9 | ⛔ el escritorio no lanza |
| 9 | la MADT fuera del kernel | `smp all` -> `12 de 12` | faltan nucleos |
| 10 | `trim-paths` | nada visible | -- |
| 11 | `red rx` desde el escritorio | (fuera de esta hoja) | -- |
| 12 | **`cabina radar`** | pinta el barrido, y dice si algo se escapo | orden desconocida |
| 13 | **el audio, paso 0** | `cabina` trae los cuatro numeros del audifono | ningun aparato reproduce |
| 14 | **el TUBO de audio (A1)** | `audio` dice `TUBO ABIERTO` | dice que no esta abierto |
| 15 | **el silencio** | `audio silencio` -> `encoladas` SUBE, `tarde` = 0 | `tarde` sube |
| 16 | **el bufer prestado (A4)** | `audio` ensena `huecos` aparte de `tarde` | -- |
| 17 | **el censo de audio POR SLOTS** | `audio` encuentra el audifono | `slots mirados =0` |

⚠ **Las tres que pueden dejarte sin maquina son la 1, la 2 y la 8**, y las tres
fallan de forma distinta: las dos primeras no arrancan, la tercera arranca y no
te deja lanzar nada. Estan desarrolladas justo debajo.

★ **Y el orden de la tanda no es el de esta tabla.** Aqui estan por lo que son;
abajo estan por lo que cuestan si salen mal.



## 0.1 -- Esta tanda trae DOS cosas que pueden impedir el arranque

No es alarmismo: son las dos unicas del lote que tocan como se mapea la memoria
y como el cargador admite un programa. Van dichas aqui arriba para que, si la
pantalla se queda negra, **no haya que buscar**.

```text
   W^X    pone el bit NX (63) en toda pagina escribible. Si alguna pagina de
          CODIGO quedara marcada asi, el primer salto a ella da #PF y no
          arranca nada
   SMAP   Ring 0 deja de poder tocar memoria de Ring 3. Si quedara UN camino
          sin `stac`/`clac`, el primer syscall que copie algo da #PF
```

★ **Los dos fallan RUIDOSAMENTE y en el arranque**, que es la forma buena de
fallar. Lo que ninguno de los dos puede hacer es corromper el disco.

## 0.2 -- ⚠ Y UNA TERCERA, DE LA TARDE DEL 25-08: la AUTORIDAD

`EJECUTAR` y `REINICIAR` pasan a pedir **autoridad**, que solo se fija al nacer y
solo desde Ring 0. El escritorio la tiene --lo arranca el kernel-- y lo que el
lanza, no.

```text
   si esto esta mal    el escritorio NO PUEDE LANZAR NADA ni reiniciar
   como se ve          escribes `calc` y contesta un error de permiso
```

★ No impide arrancar y **no se lleva la maquina**: el escritorio sigue en pie y
lo dice. Pero es lo primero que hay que probar despues de que aparezca el
escritorio, porque si falla, la tanda entera se queda sin poder lanzar programas.

## 0.3 -- Y una que puede impedir que un programa CARGUE, no que arranque

El cargador comprueba desde el 25-08 que cada relocation quepa en la seccion que
dice parchear. Se midieron los 24 `.bex` del arbol contra la regla y **ninguno se
rechaza**, pero la mas ajustada de DOOM acaba **justo** en el borde:

```text
   .data de doom.bex     151.560 bytes = 0x25008
   la reloc #706         offset 0x25000, ocho bytes, acaba en 0x25008
   holgura               CERO
```

⚠ **Por eso DOOM es la prueba 3 y no un extra.** Si la regla estuviera un byte
mal escrita, el sintoma no seria *"relocation invalida"*: seria que el programa
mas grande del arbol deja de arrancar.

---

# 1. LO QUE NO TOCA NADA -- cuatro ordenes de lectura

Todas desde el **escritorio**. Ninguna escribe en ningun aparato.

## 1.1 -- `cpu` ★ LA ORDEN DE ESTA TANDA

**Que afirma**: que la topologia ya se MIDE en vez de suponerse, y que si algun
testigo discrepa **se dice aqui** en vez de morir en un log de Ring 0.

**Lo que tiene que salir**:

```text
   nucleos    6 fisicos / 12 hilos   (2 por nucleo, MEDIDO)
```

**Como se cae, y cada forma dice una cosa distinta**:

| lo que sale | que significa |
|---|---|
| `6 fisicos / 12 hilos (2 por nucleo, MEDIDO)` y **sin** fila `[!] duda` | ✅ los cuatro testigos coinciden. El `27/54` era de la fuente vieja |
| lo mismo **con** fila `[!] duda` | el numero ya es bueno pero **un testigo sigue fuera de la fila**, y la fila dice cual |
| **no** aparece `(N por nucleo, MEDIDO)` | la hoja 0x0B no contesto: se cayo al testigo heredado, y `fisicos` es una COPIA de `hilos`, no una division |
| vuelve un numero imposible | ahora la fila `[!] duda` dice **por que** |

[!] **La fila `[!] duda` solo sale si hay duda.** Que no salga es el resultado
bueno, no una prueba que no corrio.

★★ **Y si sale la fila `[!] duda`, la orden siguiente es `cabina fallos`**, que
dice **cual** de los cuatro testigos discrepo y **con que numero**. Ver 5.2.

## 1.2 -- `consumo`

**Que afirma**: que la misma duda viaja al otro panel. La fila `nucleos` lleva la
nota al lado.

**Como se cae**: si `cpu` dice una cosa y `consumo` otra, el fallo esta en los
paneles y no en el kernel -- los dos leen el mismo `INFO`.

## 1.3 -- `ext` ★ LOS CUATRO BITS DE GUARDIA

**Que afirma**: que NX, SMEP, SMAP y UMIP estan **los cuatro** encendidos. Hasta
el 25-08 la tabla decia que ninguno, y era falso en tres de cuatro.

**Lo que tiene que salir**: las cuatro filas en `Yes`, con su motivo.

**Como se cae**:
- alguna en `No` -> el bit no se puso; mirar `s1_cpu/cpu/mod.rs`
- **`Smap` en `Yes` y la maquina arrancando** es justo lo que hay que confirmar:
  significa que los dos caminos que tocaban Ring 3 se quitaron bien

## 1.5 -- ★ EL AUDIO: los cuatro numeros, PREDICHOS antes de arrancar

**Que afirma**: que el paso 0 de `PLAN_AUDIO` --parsear el descriptor
AudioStreaming del audifono-- funciona en el aparato de verdad. Lleva escrito y
cableado desde hace dias y **nadie lo ha ejecutado**.

**No hay orden que teclear**: corre solo al enumerar el USB. Sale en `cabina`.

**Lo que tiene que salir**, y el maestro lo predijo antes de mirar el aparato:

```text
   audio: interfaz AudioStreaming, alt      =1
   audio: canales                           =2
   audio: bits por muestra                  =16
   audio: bytes por trama (wMaxPacketSize)  =192
   audio: frecuencia elegida                =48000
   audio: el endpoint isocrono es el DCI    =...
```

★★ **Los 192 son el numero que hay que cuadrar.** Una trama de 48 kHz / 16 bits
/ 2 canales ocupa exactamente 192 bytes por milisegundo, y `bmo-sonido` lo
calcula igual desde el otro lado. **Si el aparato dice otra cosa, ahi esta la
respuesta antes de escribir el bucle que alimenta el tubo.**

**Como se cae**:

| lo que sale | que significa |
|---|---|
| los seis numeros | ✅ el paso 0 vale, y el 192 se puede cuadrar |
| `ninguna frecuencia suya cabe en su propio paquete` | el descriptor se contradice; **no hay codigo que lo arregle** |
| `puertos libres mirados, y ninguno reproduce` | o no esta enchufado, o el parser no lo reconoce |

[!] Y no va a sonar nada todavia: **falta `SET_INTERFACE`** (A1 de `PLAN_AUDIO`),
que es lo unico que separa esto de que salga un sonido.

## 1.6 -- ** EL TUBO, Y EL SILENCIO. La primera vez que este sistema emite algo

**Que afirma**: que A1 y A2 funcionan -- que el aparato acepto ponerse en su alt,
que el xHC configuro el endpoint isocrono, y que las tramas salen.

```text
   audio              -> tiene que decir TUBO ABIERTO, con frecuencia y
                         bytes por trama
   audio silencio     -> arma el empuje
   audio              -> otra vez, y AHORA es donde se mira
```

★ **Y los TRES contadores son distintos aunque los tres se oigan igual:**

```text
   encoladas   sube sola si el tubo se alimenta
   tarde       el xHC no llego a su cita     -> el problema es del BUS
   huecos      nadie escribio la trama       -> el problema es de la APP
```

Con el silencio armado y **sin bufer prestado**, `huecos` tiene que quedarse en
cero: no hay productor al que esperar. Si sube ahi, **el contador esta mal, no el
audio**.

**Lo que tiene que salir en la segunda vuelta**:

```text
   encoladas       sube sola, y rapido (250 latidos/s x 8 tramas)
   tramas TARDE    0
```

★★ **El silencio no puede sonar mal, y esa es toda la idea.** Es la misma jugada
que `net rx`: si el tubo aguanta ceros sin atascarse, esta vivo **sin haber
arriesgado un solo ruido raro en tus oidos**.

**Como se cae, y cada forma dice una cosa distinta**:

| lo que sale | que significa |
|---|---|
| `TUBO ABIERTO` y `tarde` = 0 con `encoladas` subiendo | ✅ **A1 y A2 valen. El camino esta vivo** |
| `el tubo NO esta abierto` | fallo A1: mirar `cabina` -- o el xHC no configuro, o el aparato no acepto el alt |
| `encoladas` quieto | el latido del bus no llega, o `armar` no prendio |
| **`tarde` sube** | el metronomo no llega. Es el numero que separa "suena bien" de "chasquea" |

[!] `audio calla` lo para. Y **no se queda armado entre arranques**: no hay nada
que persista, asi que reiniciar lo deja callado.

⚠ **Ponte el audifono ANTES de `audio silencio`, no despues.** Si algo esta mal y
sale ruido en vez de silencio, es mejor que este en la mesa que en un oido.

## 1.4 -- `placa`

**Que afirma**: que la NIC declara si ofrece **MSI** y a que generacion/carriles
va el enlace PCIe -- datos que solo se alcanzan por ECAM, o sea por el MCFG.

**Como se cae**: si no sale ninguna capability extendida, o el MCFG no se
localizo o el recorrido se corto en los 48 saltos del tope.

** No se programa nada: se lee y se cuenta. Encender MSI el mismo dia que se
descubre que existe seria cambiar dos cosas a la vez.

---

# 2. LANZAR UN PROGRAMA -- que el cargador siga admitiendo lo que admitia

## 2.1 -- ★★ `doom` -- LA REGRESION QUE MAS IMPORTA DE ESTA TANDA

**Que afirma**: que la comprobacion nueva de relocations **no rechaza lo que ya
funcionaba**. DOOM trae 1.285 relocations y una de ellas acaba con holgura CERO
(ver 0.2).

**Lo que tiene que salir**: lo mismo que el 14-08. DOOM se juega.

**Como se cae, y es inconfundible**:

```text
   FALLO proc: una relocation se sale de la seccion que dice parchear =NNNN
```

⚠ **Si sale ese mensaje, la regla esta mal y hay que revertir la comprobacion**,
no ajustar el `.bex`. El numero que acompana es el `offset` de la reloc culpable,
y con el se sabe en un minuto si el fallo es el borde (`<` en vez de `<=`) o
otra cosa.

## 2.2 -- ★★ `sonda` -- LOS DOS EMPUJONES NUEVOS, Y EL ULTIMO ES EL PELIGROSO

**Que afirma**: que C4 quedo cerrada -- que un `.bex` lanzado por el escritorio
**no puede lanzar otro ni reiniciar la maquina**.

`sonda.bex` gana los empujones **8** y **9**:

```text
   8. lanzar otro programa sin autoridad    [ok] negado (codigo 3)
   9. reiniciar sin autoridad               [ok] negado (codigo 3)
```

⚠⚠ **EL 9 ES EL EMPUJON MAS PELIGROSO DE TODA LA TANDA.** Si el kernel no se
defiende, **la maquina se reinicia en ese momento** -- y con ella se pierde todo
lo que la sonda iba a decir. Por eso va el ultimo dentro del propio programa.

**Como se cae, y las dos formas dicen cosas distintas**:

| lo que pasa | que significa |
|---|---|
| `[ok] negado` en los dos, y sale el recuento | ✅ C4 cerrada |
| `[FALLO] el kernel DEJO PASAR` en el 8 | la autoridad no se comprueba en `EJECUTAR` |
| **la maquina se reinicia sola** | la autoridad no se comprueba en `REINICIAR`. **Ese es el resultado**, aunque no salga el recuento |
| `sonda` no arranca | esto es del gate, no de C4: mirar `cabina` |

★ **Y la prueba de que el escritorio SI la tiene se hace sin querer**: si has
podido escribir `sonda` para llegar aqui, `EJECUTAR` con autoridad ya funciono.

## 2.3 -- `ray` y la calculadora

**Que afirma**: lo mismo con los programas pequenos. `ray.bex` trae UNA
relocation; si DOOM pasa y este no, el fallo es de las tablas pequenas.

---

# 3. LO QUE CAMBIA EL ESTADO DEL HARDWARE

★ A partir de aqui **ya no es leer**. Nada de esto rompe el disco, pero `smp all`
no se deshace sin reiniciar.

## 3.1 -- `smp` y luego `smp all`

**Que afirma**: que el careo de la topologia y el bring-up **cuentan lo mismo**.

**El orden importa y es este**:

```text
   smp          censa y NO despierta a nadie. Mira y no toques
   smp all      levanta los demas
   cpu          otra vez  <- y aqui esta la prueba
```

**Como se cae**: si despues de `smp all` el `en pie` no llega a `12 de 12`, o si
`cpu` empieza a ensenar la fila `[!] duda` **cuando antes no la ensenaba**,
entonces CPUID y la MADT no dicen lo mismo -- y eso es exactamente lo que esta
tanda existe para hacer visible.

## 3.2 -- `smp prueba`

**Que afirma**: el reparto sigue dando lo que dio el 24-08.

**Lo que tiene que salir**: `~11,59x` con los doce en pie.

[!] Y el aviso de la hoja anterior sigue vigente: **ese numero es cierto y no se
puede extrapolar.** La faena del banco esta ligada a LATENCIA; el motor de
inferencia esta ligado a THROUGHPUT y ahi el techo sigue siendo ~6x.

## 3.3 -- La red

★ **Fuera del alcance de esta hoja por decision del dueno** (*"no toques en
RED"*). El paso 1 ya se cerro en metal el 25-08: 16 tramas, 7.967 bytes, 0
perdidas, IPv4 16.

Lo unico sin fotografiar es `red rx` **desde el escritorio** (commit `e555684a`),
que hasta ese dia mandaba a Ring 0. Se anota aqui y **no se pide**: quien decida
probarlo tiene el plan en `docs/metal/PRUEBA_RED_PASO_1.md`.

---

# 4. LO QUE HAY QUE TRAER DE VUELTA

Con `guarda` queda en `A:\datos\SALIDA.TXT`, que es como se hizo la hoja del
24-08.

```text
   [ ] la salida de `cpu`       ENTERA, con la fila de duda o sin ella
   [ ] la de `ext`              las cuatro filas de guardia
   [ ] la de `placa`            las capabilities extendidas de la NIC
   [ ] `smp all` + `smp prueba` los dos numeros
   [ ] si DOOM arranco          si/no, y el mensaje exacto si no
   [ ] la salida de `sonda`     ENTERA, con su recuento
   [ ] la de `cabina fallos`    despues de todo lo demas
   [ ] la de `cabina radar`     *** LA MAS IMPORTANTE DE LA LISTA
   [ ] las seis filas de `audio` con el audifono ENCHUFADO
```

★ **`cabina fallos` va el ultimo de la lista y se pide siempre**, salga bien o
mal la tanda. Es el unico sitio donde estan juntos todos los avisos que el kernel
apunto durante el arranque -- incluidos los que no tumbaron nada y por eso no
salieron por pantalla.

### *** Y `cabina radar` ANTES QUE `cabina fallos`, aunque suene al reves

`cabina fallos` filtra **los 48 que sobrevivieron en el anillo**. Un FALLO del
arranque que ya se cayo **no sale**, y la respuesta es *"ni un aviso ni un
fallo"* -- indistinguible de estar bien.

`cabina radar` cuenta en el ORIGEN y **no pierde ninguno**, ni por giro ni por
reentrancia. Lo que hay que mirar es la ultima linea:

```text
   nada se ha escapado: todo lo que paso sigue en el anillo    <- entonces
                                                                 `fallos` dice
                                                                 la verdad
   [!] N clase(s) marcadas con ! : HUBO, y ya no se pueden leer <- entonces
                                                                 `fallos` MIENTE
                                                                 por omision
```

[!] Si sale la segunda, **traer el numero y la fila**: dice de que capa y de que
gravedad se perdieron sucesos, y eso ya acota donde mirar aunque el detalle no
exista. Y en ese caso `cabina` a secas ya no basta -- hay que volcar antes.

---

# 4.5 -- ★★ LO QUE EL RYZEN YA CONTESTO (primera vuelta, 25-08)

```text
   nucleos    6 fisicos
   hilos     12 logicos       y SIN fila de duda
   en pie     1 de 12
```

## Lo que esas tres lineas cierran de golpe

★★ **La fila de `nucleos` no lleva nota de duda, y `duda_nota()` solo calla si
los CUATRO bits estan a cero.** Asi que de un tiron queda demostrado:

| lo que se prueba | por que |
|---|---|
| el careo **corrio** | si no, el bit `SIN_MEDIR` daria texto |
| la hoja **0x0B contesto** | `hilos_por_nucleo` se MIDIO, no se supuso |
| las dos hojas de CPUID **coinciden** | el bit `CPUID` esta a cero |
| **la MADT coincide** con CPUID | el bit `MADT` esta a cero -- y eso confirma en metal el `leer_madt` que salio del kernel a `bmo-firmware` (C6) |

** Y que la maquina ARRANQUE descarta las dos primeras de la lista de 0.0: **W^X
y SMAP no la tumbaron.**

⚠ **Lo que sigue SIN probar de esa lista**: `EJECUTAR` con autoridad (C4).
`audio` y `save` son ordenes internas, no lanzamientos. **Basta con escribir
`calc` una vez.**

## Y el audio: el `=0` que resolvio el fallo

```text
   audio: puertos libres mirados, y ninguno reproduce  =0
```

★★ **Cero no era "mire y no habia": era "no llegue a mirar".** Esa distincion la
da esa linea a proposito, y fue la que lo resolvio -- el audifono ya estaba
enumerado, tenia slot, y el censo recorria PUERTOS LIBRES. Arreglado el mismo
dia: ahora recorre slots, como el camino del volumen.

---

# 5. ⚠ LO QUE ESTA TANDA **NO** PUEDE CONTESTAR, Y HAY QUE DECIRLO

## 5.1 -- Por que el 25-08 salio 27/54

**No se sabe, y esto no lo averigua.** El mismo codigo dio 12 el dia anterior. Lo
que esta tanda hace es **volverlo visible la proxima vez**: si vuelve a pasar, la
fila `[!] duda` dira cual de los cuatro testigos se salio de la fila.

★ Y el 12 del 24-08 sigue siendo lo que era: **un acierto que no se podia
demostrar.** Ahora se podria.

## 5.2 -- [X] El detalle del careo YA llega al escritorio (arreglado el 25-08)

Esta seccion decia que el careo apunta cuatro lineas en CABINA --que testigo, con
que valor-- y que **desde el escritorio no habia forma de leerlas**: no existia
orden `cabina`, y `autopsia` ensena el ultimo fallo de Ring 3, que es otra cosa.

**Se escribio antes de la tanda, que es justo lo que esta hoja recomendaba.** La
fontaneria estaba entera --`OP_CABINA_INFO`, `OP_CABINA_TEXTO`, los nueve campos
y las cinco severidades, con sus envoltorios en `userland`-- y faltaba la orden.

```text
   cabina          los ultimos 20, con severidad y color
   cabina todo     los 48 del anillo
   cabina fallos   solo WARNING y peores   <- la que se usa cuando algo fallo
```

★ **Y eso cambia lo que hay que teclear en la prueba 1.1.** Si `cpu` ensena la
fila `[!] duda`, la orden siguiente es `cabina fallos`, y ahi sale **cual** de
los cuatro testigos se salio de la fila y **con que numero**:

```text
   [!] cpu   CPUID se contradice: hoja 0x0B contra la heredada  =NN
   [!] cpu   la MADT declara otros hilos que CPUID              =NN
   [X] cpu   el silicio NO dice lo que este perfil sabe que es  =NN
```

[!] Y hay que leer la cabecera del volcado: si dice que **se cayeron** eventos
del anillo, lo que se esta viendo no es el principio del arranque.

## 5.3 -- La cara que viaja no se ve todavia

El formato y el emisor entraron el 25-08 con 19 pruebas, pero **el lector del
escritorio (escalon 3) no esta escrito**. Nada que teclear: la cara de la
calculadora existe como bytes y todavia no la pinta nadie.
