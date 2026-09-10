# LO QUE DIJO EL RYZEN -- 2026-08-24

> La foto de verdad, con la salida cruda en `A:\datos\SALIDA.TXT`. Se anota
> entera, incluidas las dos cosas que salieron mal, porque **una hoja que solo
> apunta lo que funciono no es una medida: es un anuncio.**

---

# 1. *** EL RESULTADO GRANDE: EL REPARTO DA 11,59x

```text
   smp prueba   ->  aceleracion: 0.99x     <- con los nucleos dormidos
   smp all      ->  nucleos en pie: 12 de 12
   smp prueba   ->  aceleracion: 11.59x
```

## Lo que esto cierra

El `0.00x` del **2026-08-08** llevaba mas de dos semanas sin explicacion, y la
explicacion resulto ser la mas simple: **los nucleos no estaban en pie.** `smp
prueba` a secas mide con lo que hay, y lo que habia era el BSP solo -- por eso
`0,99x`, que es exactamente "un nucleo hace el trabajo de un nucleo".

** Y eso convierte un plan entero de fe en un plan sobre un numero: la puerta a
los nucleos desde Ring 3 ya no se disena sobre un reparto que no se sabia si
funcionaba.

## [!] PERO EL NUMERO CONTRADICE LA PREDICCION, Y HAY QUE DECIRLO

`AXION_MAESTRO` y la memoria del proyecto decian:

> *"~6x es el techo honesto, no 12x: dos hilos SMT comparten L1/L2 y unidades de
> ejecucion. Si `smp prueba` da 6,x, eso ES el maximo."*

Salio **11,59x sobre 12 hilos: el 96,6%**. La prediccion estaba equivocada, o la
faena no es la que la prediccion suponia. Y la segunda es la respuesta:

```text
   faena LIGADA A LATENCIA    una cadena de dependencias, poco ILP
                              -> los dos hilos SMT se rellenan los huecos
                              -> casi 2x por nucleo. **Es lo que se midio**

   faena LIGADA A THROUGHPUT  matmul con AVX2, las unidades saturadas
                              -> el segundo hilo no encuentra hueco
                              -> ~6x, que es lo que decia la prediccion
```

*** **Asi que 11,59x es cierto Y no se puede extrapolar.** El motor de
inferencia es lo segundo, y ahi el techo sigue siendo ~6x. Medir el reparto con
la faena de verdad --un producto de matrices-- es lo que dira el numero que
importa, y **es una faena distinta de la que este banco corre hoy**.

---

# 2. LA RED: la MAC acerto, y el enlace NO es el que se predijo

```text
   tarjeta     0x10EC8168   (Realtek RTL8168)
   en el bus   37:0.0
   MAC         2C-F0-5D-D9-3C-E3        <- PREDICHA. Coincide
   enlace      ARRIBA, 10 Mbit          <- se predijo 100
   PHYstatus   0x87
```

## El byte crudo es la prueba, y aqui se ve por que se guarda

`PHYstatus = 0x87 = 0b1000_0111`. Con el mapa de la familia:

```text
   0x01  FullDup     puesto
   0x02  LinkStatus  puesto   <- hay enlace
   0x04  10 Mbps     puesto   <- ***
   0x08  100 Mbps    NO
   0x10  1000 Mbps   NO
   0x80  ?           puesto   <- ver abajo
```

** **El decodificador acerto**: leyo `10 Mbit` de un byte que dice 10 Mbit. Lo
que cambio es el enlace fisico, no el codigo -- el 2026-08-11 la misma maquina
dio `0b1011` (100 Mbps). Cable, puerto del switch o negociacion: es un hecho del
mundo, no del driver.

## [!] Y UN BIT QUE NO SE DECODIFICA: el 0x80

Esta puesto y **este driver lo ignora**. En la familia 8169 ese bit es
`TBI_Enable` --interfaz de fibra-- y en un 8168 de cobre no deberia estarlo.

*** No se afirma que sea un fallo: se anota que **hay un bit encendido que
nuestro mapa no explica**, y que el byte entero esta guardado justo para poder
mirarlo cuando alguien tenga la hoja del chip delante. Es la regla escrita en
`Identidad::phy`: *el byte entero es la prueba y las funciones son la opinion*.

## Y el paso 1 NO se hizo

```text
   receptor    apagado   (net rx en Ring 0)
   tramas      0
```

`net rx` **desde el escritorio no arma nada**, y no es un fallo: es una decision
escrita en `commands/mod.rs` -- *"ninguna transmite ni un byte: son campos de
INFORME"*. El panel te manda a Ring 0.

*** **Y ahi esta el problema de verdad, que no es de la red.** La memoria del
proyecto lo tiene con nombre desde hace tiempo: **el dueno vive en el escritorio
y al shell de Ring 0 no vuelve.** Un camino que solo existe en Ring 0 es un
camino que el dueno de la maquina no puede tomar.

Lo mismo le paso a `placa`:

```text
   placa
     no es un comando ni una ruta. escribe 'help'.
```

Lo cablee en el shell de Ring 0 y **el escritorio no lo tiene**. Es mi error, y
es el patron documentado.

### Lo que hay que hacer, y la forma ya existe

No es *"que el escritorio toque la NIC"*. Es una **operacion sobre la tarea**,
igual que `TASK_OP_DISCO`: un programa de Ring 3 le pide al kernel que arme el
receptor, y el kernel decide. Con la misma regla que el disco -- **se apunta en
CABINA antes y despues**, porque la primera operacion que cambia el estado de un
aparato no puede ser silenciosa ni cuando funciona.

---

# 3. EL CONSUMO, con los doce en pie

```text
   nucleos       6 fisicos / 12 logicos, 12 en pie
   reloj ahora   4490 MHz   (base 3700)   -- boost, medido por MPERF/APERF
   gasta paquete 57.7 W
   gasta nucleo   9.9 W
   memoria       17 MiB usados de 15.178
```

** **Confirma lo que `AXION_MAESTRO` predijo y nadie habia medido**: con los doce
en pie, once giran en vacio y *"la maquina consume como si estuviera
trabajando"*. 57,7 W en reposo es ese precio, con su numero.

*** Y es el argumento de MWAIT convertido en watios. Hasta hoy la frase era una
prediccion; ahora es una medida, y la mejora se podra comparar contra ella.

[!] Y BMO-X ocupa **17 MiB de 15.178**. La fila que el propio informe llama *"la
que tiene que VOLVER"* volvio.

---

# 4. LAS CASILLAS, DESPUES DE ESTA FOTO

| | antes | ahora |
|---|---|---|
| el reparto entre nucleos | 🟡 `0.00x`, sin explicar | 🟢 **11,59x en metal** |
| la MAC y el enlace | 🟢 | 🟢 (y el enlace bajo a 10 Mbit) |
| el anillo RX | 🟡 escrito | 🟡 **sigue sin foto** -- no hay camino desde Ring 3 |
| el censo de la placa | 🟡 escrito | 🟡 **no llega al escritorio** |
| el consumo de los 12 | ⚪ predicho | 🟢 **57,7 W medidos** |
