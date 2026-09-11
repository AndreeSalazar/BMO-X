# PLAN -- EL SEMAFORO COMPLETO: donde el arbol todavia no dice de que color es

> Escrito el **2026-09-10**, midiendo el arbol entero fichero a fichero. Ningun
> numero de aqui es una impresion: todos salen de contar `[carril]` contra el
> total de `.rs` de cada zona, saltando los bancos de pruebas.
>
> La ley es **L6g**, el nivel 3 (`FUERO/META-KERNEL_HARD.md`). Y su eje, que es
> lo que hay que tener delante para repartir bien:
>
> > **Voy a tocar esto. Que arrastro?**
>
> No *"que hace"* -- eso ya lo dice el nombre del fichero.

---

# 1. EL MAPA, MEDIDO

```text
   zona                              ficheros  con carril    %
   --------------------------------  --------  ----------  ----
   Ultra_kernel_x86-64/.../ring0          177         177   100
   toolchain/lang/c/src (BMO C)            39          39   100
   platform/abi                           100          23    23
   Ultra_userspace/userland/src            20           4    20
   *** platform/drivers                    50          12    24   <- y son RING 0
   toolchain/tools                         36           3     8
   Ultra_userspace/services                72           6     8
   platform/shared                         48           2     4
   toolchain/lang/{inti,cobol,cpp,ada}     93           0     0
```

## [!] Y el que importa no es el porcentaje mas bajo

`toolchain/lang/inti` esta al 0% y **no pasa nada**: es un compilador que corre
en el anfitrion, y si se equivoca lo caza su banco o el programa no compila.

`platform/drivers` estaba al 1% hasta hoy, y eso **si** era un agujero:

```text
   son 14 crates que el KERNEL ENLAZA          bmo-ahci, bmo-xhci, bmo-net...
   ejecutan con el privilegio de Ring 0
   lo unico que no es de Ring 0 es su CARPETA
```

*** Y ahi dentro esta **todo el DMA**. Los catorce sitios que
[`NEUTRO/DMA/EMBUDO.txt`](../../NEUTRO/DMA/EMBUDO.txt) censa --los que le dan una
direccion fisica a un aparato-- viven en `platform/drivers`, sin excepcion. O
sea que el codigo que le habla a los maestros del bus era **el menos senalizado
del arbol**, con Ring 0 al 100%.

  > Un semaforo cuyo alcance es una CARPETA y no un PRIVILEGIO deja fuera justo
  > lo que se mudo de carpeta.

** Es la TERCERA vez que aparece esta frase. `contrato_ley.py` la lleva escrita
dos veces --R11 para REX, R17 para `fundamentals/`-- con estas palabras: *"no lo
cubria ninguna regla, porque L6g dice todo `.rs` de Ring 0 y esto no es Ring
0"*. En los drivers esa frase es **falsa**, y por eso este caso es distinto: no
es codigo de al lado, es Ring 0 con otra direccion.

---

# 2. LO QUE YA SE HIZO EL 2026-09-10

- [x] **S0 -- EL CAMINO DEL DMA, SENALIZADO.** Los once ficheros que le dan una
      direccion fisica a un aparato, mas la fachada de AHCI. 12 de 50.

  ```text
     ROJO   ahci/{comando,arranque,controller}.rs      la PRDT y las tres tablas
     ROJO   xhci/{transferencia,lib,enumerar}.rs       TRBs, DCBAA, contextos
     ROJO   net/{anillo,lib}.rs                        el corral de la tarjeta
     VERDE  ahci/{lib,storage_hal}.rs                  fachada y `trait`
     VERDE  xhci/avisos.rs                             el buzon, y se PRUEBA
  ```

  ** Y uno de ellos estaba **a medias**: `net/src/lib.rs` declaraba `[cuesta]
  APARATO` y `[riesgo] SILENCIO` desde agosto **y no declaraba `[carril]`**. Las
  dos etiquetas dificiles puestas y la facil sin poner, porque ninguna regla la
  pedia ahi.

- [x] **S1 -- R20, la regla, CON TRINQUETE.** `toolchain/tools/contrato/` y su
      suelo en `CARRILES_DRIVERS.txt`.

  [!] R10, R11 y R17 dicen *"sin trinquete: se empieza cubriendolos todos"*, y
  podian decirlo porque no habia nada que tolerar. Aqui hay 19.016 lineas ya
  escritas: un muro dejaria el build ROJO hasta terminarlas, y **un build rojo
  que hoy no se puede poner verde se acaba desactivando**.

- [x] **S2 -- EL DENOMINADOR.** Los dos guardianes del semaforo decian `N` y
      ahora dicen `N de M`.

  *** Un numero suelto se lee como cobertura completa. `12 ficheros declaran su
  carril` suena a terminado; `12 de los 50` es lo mismo y suena a lo que es.

- [x] **S3 -- `fases.py` DEJA DE SALTARSE EN SILENCIO.** Un fichero de BMO C sin
      `[fase]` ni `[aparece]` se saltaba con un `continue`, y sumado al
      trinquete eso queria decir que **un fichero NUEVO sin etiqueta pasaba**:
      `marcados` no bajaba porque el nuevo nunca conto.

  ** Ahora se exige a todos y sin trinquete, por la misma razon que R10 lo hace
  con Ring 0: la cobertura ya era 39 de 39. Comprobado creando un fichero vacio
  -- lo caza y dice por que.

    > Un contador que solo mira lo declarado no cuenta de menos: deja de ser un
    > guardian. Misma forma que `EMBUDO.txt` (10-09) y `planes.py` (09-09).

---

# 3. LO QUE FALTA, POR ORDEN DE PELIGRO

El criterio no es el tamano ni el porcentaje: es **como se anuncia el fallo**.
Una zona donde equivocarse REVIENTA no necesita semaforo con urgencia -- el
fallo ya avisa. La urgencia esta donde equivocarse **no avisa**.

## [ ] S4 -- los 38 drivers que quedan (`platform/drivers`)

Los que mas pesan, y lo que hay que mirar en cada uno:

```text
   usb/uhid/       891+648+396   interpreta informes de un aparato AJENO
   storage/fat32/  971+537       escribe en el disco: aqui se pierde trabajo
   storage/estratos/ 679+572+541 el FS propio, y escribir ES commitear
   usb/uaudio/     626+509       pide bufers que el xHC va a leer
   usb/input/      633           el foco: si se equivoca, las teclas van a otro
   gpu/rdna4/      557           ya tiene UNO con carril (psp.rs, AMARILLO)
   storage/{block,identify,particiones,trim}
   audio/, rtc/
```

Se verifica: `CARRILES_DRIVERS.txt` sube hasta 50, y el renglon del build dice
`50 de los 50`.

## [ ] S5 -- `platform/shared`, 2 de 48

Aqui viven **los jueces**: `bmo-dma-juicio`, `bmo-dma-forma`, `bmo-carga-juicio`,
`bmo-fisica-juicio`, `bmo-mmio-juicio`, `bmo-disco-juicio`, `bmo-juicio`.

** La mayoria va a salir VERDE, y eso NO hace inutil el trabajo: *"saber que
algo es verde tambien es saber"* (L6g). Un juez puro, sin estado y con su banco
de filas, es exactamente lo que se puede tocar deprisa un dia que la maquina
esta rota -- y hoy nada lo dice.

[!] Ojo a las excepciones, que son las que valen: `bmo-hash` lo usan los CINCO
frontends y ESTRATOS a la vez; tocarlo cambia las firmas de todo lo ya escrito.

Se verifica: `platform/shared` en la tabla de la seccion 1, al 100%.

## [ ] S6 -- `platform/abi`, 23 de 100

`fundamentals/` ya esta cubierto por R17 (23 de 23). Lo que falta es `bef/` --el
formato-- y `syscalls/`.

** Y `bef/writer.rs` es el caso de libro: escribe el `.bex` que el kernel va a
ejecutar. Si se equivoca, el fallo aparece en el METAL y en otro programa.

Se verifica: una regla como R17 pero sobre `bef/`, y su renglon en el build.

## [ ] S7 -- `Ultra_userspace/services`, 6 de 72

El escritorio y el compositor. Aqui el fallo **se ve**, que es el mejor de los
casos: por eso va detras de los drivers y no delante.

---

# 4. *** LO QUE NO HAY QUE HACER, Y ES LA MITAD DEL PLAN

## 4.1 No partir un fichero porque sea grande

L6g parte cuando hay **DOS MASAS**, no cuando hay muchas lineas. Partir por
tamano produce tres ficheros donde solo hay dos lineas de verdad, y eso es la
aguja mejor escondida.

- [ ] **S8 -- los TRES candidatos con evidencia**, y solo esos. Cada uno trae la
      costura ESCRITA EN SU PROPIO FICHERO, que es lo que los separa de una
      corazonada:

  ```text
     net/src/lib.rs        826   "Preguntale al chip quien es. No escribe NADA"
                                 -> leer (VERDE) y programar el filtro (ROJO)

     usb/xhci/src/lib.rs   722   seis secciones, y dos de ellas son PLANOS:
                                 `TRB`, `Registers`, `USB Descriptor types`
                                 -> las formas (VERDE) y el `Controller` (ROJO)

     usb/uhid/src/lib.rs   891   ya tiene dentro "EL PORTERO: LOS PAPELES Y EL
                                 VEREDICTO" con su propio letrero de seccion
  ```

  [!] **Y no antes de un arranque verde.** Partir un driver de disco o de USB
  sin poder arrancar es cambiar codigo que nadie va a ejecutar en semanas.

## 4.2 No poner carril donde no hay privilegio ni sorpresa

`toolchain/lang/{inti,cobol,cpp,ada}` estan al 0% y **se quedan asi**. Son
compiladores del anfitrion: si se equivocan, lo dice su banco o no compila. Un
semaforo ahi seria papeleo.

*** BMO C es la excepcion, y tiene su propia regla (`[fase]` + `[aparece]`) en
vez de esta. El motivo es exacto: **su fallo APARECE LEJOS**. Quince de sus 39
ficheros declaran `[aparece] DENTRO`, o sea que el banco no protege ahi y el
sintoma sale en otro programa, tres capas mas arriba. Eso es lo que pide
semaforo, no ser un compilador.

  > El carril no se pone por importancia. Se pone donde el fallo no avisa.
