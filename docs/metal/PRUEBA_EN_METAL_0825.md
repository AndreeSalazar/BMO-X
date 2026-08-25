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

## 0.2 -- Y una que puede impedir que un programa CARGUE, no que arranque

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

## 2.2 -- `ray` y la calculadora

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
```

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
