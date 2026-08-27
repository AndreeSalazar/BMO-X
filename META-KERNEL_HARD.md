# META-KERNEL HARD

> **La ley de la maquina.** No la firma el dueno del proyecto: la firman los
> componentes que hay dentro de la caja.
>
> Escrito el **2026-08-16**, despues de que el eje de CICLOS consiguiera por fin
> las cinco piezas que hacen que una regla muerda (numero declarado, motivo al
> lado, metro real, juez que se niega a opinar, guardian que ya rechazo algo).
> Este documento generaliza esa forma al resto de la maquina.

---

## 0. Por que existe este documento

Una regla que dice *"aqui optimizamos ciclos"* es una preferencia. Se discute,
se olvida y se incumple sin que nada grite.

Una regla que dice *"la linea de cache de este chip mide 64 bytes, asi que dos
nucleos que escriben datos a menos de 64 bytes de distancia se roban la linea el
uno al otro"* **no se discute**: o se cumple o la maquina va lenta, y el numero
64 no lo eligio nadie de este proyecto.

**La diferencia entre las dos frases es todo el documento.** Aqui una regla solo
puede existir si al lado tiene el componente que la exige y el numero con el que
la exige. Lo demas es estilo y va a otro fichero.

### Que NO es

- No es una lista de buenas practicas. No hay ninguna regla aqui cuyo motivo sea
  *"queda mas limpio"*.
- No es un manual de hardware. Los manuales estan en AMD, en la spec de AHCI y
  en la de xHCI; aqui solo esta **lo que esta maquina exige y lo que BMO-X hace
  al respecto**.
- No sustituye a `docs/`. `LA_RAM.md` explica una identidad, `AXION_MAESTRO.md`
  un plan. Este fichero dice **que esta prohibido y por que no lo permite el
  silicio**.

### Como se lee una regla

Cada componente trae cuatro cosas y siempre en el mismo orden:

```
   PARA QUE EXISTE   por que hay una pieza asi en un ordenador
   QUE EXIGE         los numeros del componente, con su origen
   LAS REGLAS        R-<COMPONENTE><n>, citables desde la cabecera de un fichero
   EL PRECIO         lo que ya se pago en esta maquina por incumplirla
```

El apartado **EL PRECIO** no es decoracion. Una regla sin muerto detras suele
ser una regla inventada; cuando la hay, se dice de donde salio.

---

## 1. La ley de la casa (las meta-reglas)

Estas seis mandan sobre todas las demas. Si una regla de componente choca con
una de estas, gana esta.

### L0. Un eje sin juez es prosa

Un eje de optimizacion solo se declara cuando tiene **metro, fila y guardian**.
Los demas van a *DESCARTADO con motivo*, que es discutible; un eje escrito y sin
juez es un agujero con pinta de norma.

> Precio: `bmo_input::foco` tenia 12 tests, estaba cableado para pintar la
> ventanita de Alt+Tab, y `es_para` no se llamaba ni una vez. Todo verde y las
> teclas cayendo donde no era. **Una politica escrita, probada y que nadie
> consulta.**

### L1. Toda regla nombra a su componente y su numero

*"Esto es lento"* no es una regla. *"Esto son 872 ciclos por cruce de anillo y
el presupuesto dice 960"* si.

### L2. Tres origenes, y NUNCA se mezclan

```
   [MEDIDO]      medido en ESTA maquina, con el metro y la ventana declarada
   [SILICIO]     declarado por el chip (CPUID, spec del fabricante)
   [LITERATURA]  numero de fuera, todavia sin medir aqui  <- se marca SIEMPRE
```

Un numero de literatura escrito sin marcar se convierte en un hecho de la casa a
la tercera vez que alguien lo copia. Todo `[LITERATURA]` de este documento lleva
al lado **la sonda que lo convertiria en `[MEDIDO]`**.

### L3. Toda regla trae su sacrificio

Ninguna optimizacion es gratis. Una regla que no dice que empeora esta a medio
escribir. Ver seccion 2.

### L4. Una regla se prueba diciendo que NO

Un guardian que nunca ha rechazado nada no esta probado: se rompe el arbol a
proposito, se comprueba que grita **con el nombre del campo**, y se restaura.

> Precio: cuatro guardianes en `build.ps1` y los cuatro se probaron asi. El del
> formato del handle rechazo `HANDLE_GEN_SHIFT` puesto a 41 y lo dijo por su
> nombre.

### L5. Hardcodea CONTRATOS, pregunta HECHOS

El ABI, `USER_IMAGE_BASE`, los layouts congelados: constantes.
La RAM, los nucleos, las MMIO, el tamano del framebuffer, el area de XSAVE: se
le preguntan al silicio. **Nunca al reves.**

### L6. ** MODULAR -- y no es higiene: es INSTRUMENTACION

La regla favorita de la casa, y la que mas veces se ha pagado sola. Enunciada
por el dueno el 2026-08-13:

> *"es curioso que en el monolito hay partes que se convierten en agujas
> pequenas, pero al dividir se hacen agujas GRANDES, faciles de detectar"*

**Por que funciona, y no es estetica:** el tamano del fichero es el
**denominador de la busqueda**. Una omision no cambia de tamano al partir el
fichero; lo que cambia es **contra que se compara**. En 4.000 lineas, una
funcion que falta es ruido; en 134 con un patron declarado, es un hueco en una
simetria.

El caso con los numeros: `expr_is_float` y `expr_is_unsigned` son gemelas y la
segunda faltaba. Dentro de un `impl` de **4.161 lineas con 92 metodos**, la que
existia estaba a **900 lineas** de donde hacia falta la otra -- y el comentario
del sitio afectado llego a afirmar por escrito *"el codegen no arrastra esa
distincion hasta aqui"*, que era falso. Despues del reparto, `types.rs` son
**134 lineas con exactamente dos funciones hermanas**: que falte una tercera es
imposible de no ver.

**Las cuatro obligaciones:**

- **L6a.** Un modulo que pase de ~1.000 lineas **DE CODIGO** se parte. No es una
  sugerencia.

  **** **Y "de codigo" entro el 2026-08-24, porque la regla mordio a quien la
  cumplia.** Se le anadio a `syscall/mod.rs` una cabecera explicando POR QUE se
  habia repartido, el fichero cruzo las mil, y salto el guardian. Al medirlo:

  ```text
      977 lineas totales  ->  530 de CODIGO,  423 de DOCUMENTACION (43%)
  ```

  Su codigo era **la mitad del limite**. Lo que lo empujo fue la explicacion.

  ** Y eso hacia que el metro empujara contra lo que esta casa mas valora: la
  regla del dueno es *"todo tiene su por que; lo que no lo tiene, se quita"*, y
  este arbol es **36% documentacion medida**. Un guardian que cuenta el por que
  como si fuera riesgo **le pone precio a escribirlo** -- y el dia que alguien
  tenga prisa, lo barato sera borrar el comentario.

  Lo que L6a maneja es el riesgo del CODIGO: el estado que las funciones
  comparten y las interacciones que esconde. **Un comentario no comparte estado
  con nadie.**

  [!] Y el cambio **no es un blanqueo**, que es lo primero que hay que comprobar
  cuando una regla se relaja. Salieron cuatro ficheros y se quedaron los cinco
  mas grandes: `cobol/codegen` 2.948->1.869, `cobol/parser` 2.011->1.565,
  `c/codegen` 2.321->1.398, `cpp/parser` 1.621->1.177, `validator` 1.435->1.179.
  **Si contar codigo hubiera vaciado la lista, el cambio seria sospechoso.**

  El censo ensena **las dos columnas** --codigo y total-- para que nadie tenga
  que creerse la cuenta.
- **L6a-bis (2026-08-24).** Y **el limite es el mismo, lo que cambia es la
  salida de emergencia**: en `util` y en Ring 3 vale el trinquete --se puede
  sellar un techo y desde ese dia solo puede encoger--, y en **Ring 0 no se
  sella nada nuevo**. Un fichero del kernel que cruce las mil lineas para el
  build, y no hay `--motivo` que valga.

  *Regla de Eddi: "el guardian limita ESTRICTAMENTE hasta mil, y por que? porque
  hablamos de Bare Metal Orquestal. Pero si es para Ring 3 como library OS,
  okey."* El motivo esta dentro: **lo que cuesta un fallo depende del anillo.**
  En Ring 3 un fallo mata la tarea y el kernel recupera la pantalla --verificado
  en metal--; en Ring 0 se lleva la maquina, y ahi conviven 236 `static mut`
  que en un fichero grande puede tocar cualquier funcion.

  [!] **Y el anillo NO se decide por la carpeta.** Se midio el mismo dia:
  `platform/drivers/storage/fat32/src/lib.rs` (2.537) y
  `platform/drivers/usb/xhci/src/lib.rs` (1.584) viven bajo `platform/`,
  **parecen Ring 3 y son Ring 0** -- son crates que enlaza el kernel. Una regla
  por carpetas habria relajado el limite justo sobre los dos ficheros mas
  grandes que corren en Ring 0. Asi que el anillo sale del **grafo de
  dependencias del kernel**, que es un hecho: el dia que un driver se mude a
  Ring 3 de verdad, su `Cargo.toml` sale de ese grafo y el censo se entera solo.
- **L6b.** El corte se elige por **la pregunta que responde el fichero**, no por
  tamano ni por capas. Un fichero tiene que poder contestar *"por que soy un
  fichero y no un trozo del de al lado"*.
- **L6c.** Un fichero que declara una **simetria** (dos hermanas, tres ejes, N
  casos) hace visible el hueco. La cabecera lo dice en voz alta: *"el tercer eje
  que aparezca se escribe igual y al lado"*.
- **L6d.** ** **La prueba de que un reparto no cambio nada NO es que los tests
  pasen** --pasaban antes--: es que **el compilador emita los mismos bytes**. 33
  `.bex` hasheados antes y despues, identicos.
- **L6e. MODULAR PRECISA (2026-08-26).** El corte se elige **tambien** por lo que
  cuesta que esa pieza se equivoque, y **la cabecera lo declara**:

  ```text
     //! [cuesta]  MAQUINA -- calcula direcciones para el physmap
  ```

  Peticion del dueno, con sus palabras: *"Modular MAS precisas, la siguiente
  evolucion de Modulo (...) saber como se declara entre ellas, cual es el
  potencial que falle. Eso, porque si ese es el potencial del fallo, es la razon
  de MODULAR precisas."*

  **El vocabulario es CERRADO**, ordenado de barato a peor, y cada clase es algo
  que a este proyecto ya le ha pasado:

  | clase | que cuesta que se equivoque | donde ya paso |
  |---|---|---|
  | `NADA` | un numero mal en un panel. Se ve y se arregla | el `=1100` de la bitacora |
  | `TAREA` | muere una tarea de Ring 3. BMO sigue | `CPL3: tarea eliminada` |
  | `APARATO` | un aparato queda inservible hasta reiniciar | el teclado secuestrado |
  | `DATO` | se pierde o se corrompe trabajo de alguien | la ventana de escritura del disco |
  | `MAQUINA` | pantalla azul | el `#GP` de `destroy_address_space` |
  | `PUERTA` | rompe binarios que YA existen | `KIND_TAREA = 0x80` |

  ★★ **Y la regla que hace que esto CORTE en vez de solo documentar:**

  > **Un fichero cuya cabecera necesita declarar DOS clases esta mal cortado. El
  > corte va justo por donde cambia el coste.**

  Es el mismo truco que L6c: alli una simetria declarada hace visible el hueco;
  aqui **un coste declarado hace visible la costura**. Un fichero que sabe decir
  *"esta mitad mata una tarea y esta otra mata la maquina"* acaba de decir donde
  se parte.

  ★ **Y la segunda regla, que es la que ordena las dependencias:**

  > **Instrumentar no contagia el coste. DECIDIR si.**

  `vmm` (MAQUINA) llama a `cabina` (NADA) para apuntar, y `cabina` sigue
  costando nada: si se equivoca, sale un numero feo. Pero `vmm` llamando a
  `bmo-mmio-juicio` para **decidir** si un rango se cede hace que ese juez
  **cueste MAQUINA**, y por eso tiene 23 pruebas y cero `unsafe`. La diferencia
  no es quien llama a quien: es si la respuesta cambia lo que pasa.

  ** Esto no es una idea nueva del 26-08: **es la que el arbol ya usaba sin
  nombre.** `bmo-mmio-juicio` salio del kernel porque *"ceder una pagina de mas
  no da un fault: da una ventana"*; `bmo-disco-juicio` porque *"equivocarse aqui
  no da un fault en pantalla, se lleva el trabajo de alguien"*; y **L6a-bis
  existe por exactamente este motivo** -- *"lo que cuesta un fallo no es igual en
  los dos anillos"*. Tres cortes decididos por el coste, y ninguna ley que lo
  dijera.

  [!] **Ratchet y no muro**, igual que L6a: no se exige a los 150 ficheros de
  `ring0`. Lo que se exige es que **el que lo declare use el vocabulario**, y que
  **el numero de los que lo declaran no baje**. Lo vigila
  `toolchain/tools/contrato/contrato.py`.

****** Y desde el 2026-08-18, L6a tiene las cinco piezas.** Le faltaban las tres
ultimas y por eso se incumplia sin ruido: `gui/main.rs` crecio 1.244 lineas
entre el 08-04 y el 08-12 **teniendo ya un plan escrito para partirlo**.

```
   numero declarado    1.000 lineas, aqui arriba
   motivo al lado      el denominador de la busqueda
   metro real          `toolchain/tools/censo-modular/censo_modular.py`
   juez                LINEA_BASE.txt: un fichero de la lista solo puede ENCOGER
   guardian            --check sale con 1, y ya rechazo dos sondas:
                       una NUEVA de 1.101 lineas y una que CRECIO +5
```

** El juez es un **trinquete y no un muro**, y esa es la decision: dieciocho
ficheros incumplen L6a hoy, y un guardian que fallara con los dieciocho se
apagaria el primer dia -- *"uno que grita sin motivo se desconecta, y entonces
no protege nada"*, que ya estaba escrito en el guardian de los enlaces. Asi que
no juzga el pasado: juzga **el delta**. Lo que hay se arregla cortando; lo que
se prohibe es que aparezca otro.

### L7. ** LA HERENCIA: abuelo, padre, hijo y nieto

L6 dice **cuando** partir. Esta dice **como se ordenan los trozos**, y es la que
convierte una medida en algo que se puede refutar.

```
   abuelo   el HECHO en crudo        no sabe para que se usa
   padre    lo NOMBRA y lo compone   no sabe que tiene hermanos
   hijo     RELACIONA dos del padre  no sabe que significa la relacion
   nieto    el SIGNIFICADO / veredicto   vive fuera y se puede probar
```

El reparto que le dio nombre, del metro de ciclos:

```
   abuelo   `puertas`      N cruces y nada mas. No sabe que mide
   padre    `Fila`         nombre + capability + operacion. No sabe que hay otras filas
   hijo     `contra`       la diferencia entre dos filas. No sabe que significa
   nieto    `bmo-juicio`   el veredicto. Fuera del binario, probado en el anfitrion
```

**La ley, en una frase: EL CONOCIMIENTO SOLO BAJA.** Ninguna generacion sabe
quien la consume. El abuelo no puede nombrar al padre, el padre no puede
preguntar por sus hermanos, y el nieto es el unico que tiene opinion.

**Y ahora la parte que la hace obligatoria y no bonita -- las tres cosas que
compra:**

1. **Permite disenar el experimento.** Es lo que dejo elegir las filas de la
   sonda para que **entre dos consecutivas cambie UNA SOLA COSA**. Sin esa
   separacion, la resta mezcla dos variables y cuatro tandas seguidas dan el
   mismo numero y la misma duda -- que es exactamente lo que paso.
2. **** **Hace FALSABLE una medida.** La frase *"los 246 ciclos no pueden estar en
   el stub porque **el stub no sabe que operacion se pidio**"* no es una
   intuicion: **es L7**. El abuelo ignora al padre por construccion, asi que un
   coste que dependa de la operacion no puede aparecer ahi. Sin la jerarquia esa
   anomalia no seria una anomalia, seria un numero raro.
3. **Permite probar el significado sin la maquina.** El nieto no toca hardware,
   asi que vive en `platform/shared/` y corre en `cargo test`. Una regla sobre
   numeros se prueba en tres segundos, no en una tanda de flasheo.

**L7a.** Si una generacion necesita saber algo de la de abajo, **el corte esta
mal**: o el dato sube como parametro, o las dos son la misma generacion.

**L7b.** El nieto **siempre** fuera del binario que mide, aunque salga mas caro.
*"Alli se puede PROBAR; este binario es `no_main` para un target sin sistema
operativo y no corre un test."*

****** L7c. La generacion se comprueba entre CRATES, nunca entre ficheros.**
Anadida el 2026-08-18, al ponerle metro a L7 y descubrir por que el metro obvio
--leer los `use`-- habria condenado codigo correcto en su primera vuelta:

```
   una TUBERIA de datos   el consumidor importa al productor
                          -> conocimiento y dependencia apuntan IGUAL
   una CADENA de llamadas el que llama importa al llamado
                          -> el que MENOS sabe importa al que MAS sabe
```

`syscall/entry.rs` esta etiquetado **abuelo** en `docs/CENSO_DE_EJES.md`, y su
linea 41 dice `use super::dispatch;`: **el abuelo nombra al padre.** No es un
fallo del kernel. Es que ahi la etiqueta dice *cuanto sabe cada pieza* --que es
lo que hace falsable la medida-- y no *quien importa a quien*.

Donde las dos coinciden es donde la generacion **es un crate**, y entonces la
relacion no se deduce: esta declarada en el `[dependencies]` de un
`Cargo.toml`, un `pub use` no la puede esconder y no hay heuristica. Por eso el
guardian (`toolchain/tools/censo-modular/herencia.py`) solo juzga crates, y por
eso un crate que lleve varias generaciones dentro **lo declara** (`generacion:
varias`) en vez de callarse.

*El precio*: MAQUETA sale limpia --seis aristas y todas bajan-- y la puerta no
se puede juzgar asi. Saberlo costo un `grep`; construir el metro equivocado
habria costado el guardian entero, porque uno que grita sin motivo se apaga.

---

## 2. Los cinco ejes, y cual manda en BMO-X

Los cinco ejes no son cinco maneras de escribir codigo. Son **cinco recursos
escasos distintos**, y cual se agota lo decide la CARGA, no el fichero. Por eso
un eje se declara donde se conoce quien llama y cuantas veces.

| eje | el recurso escaso | unidad | quien lo paga | sacrificio tipico | juez hoy |
|---|---|---|---|---|---|
| **LATENCIA** | tiempo hasta la respuesta de UNA operacion | ciclos/op (minimo) | el que espera bloqueado | tamano y cache | **SI** |
| **TAMANO** | el sitio donde tiene que caber | bytes contra un techo fisico | el cargador y la pila | ciclos (mas saltos) | parcial |
| **THROUGHPUT** | la tasa agregada | bytes/s, frames/s | nadie en concreto | latencia individual, memoria | no |
| **CACHE** | espacio con jerarquia de velocidad | fallos por operacion | el eje de ciclos, sin verlo | ciclos (recalcular) | **NO** |
| **ENERGIA** | julios y grados | milivatios | la bateria y el ventilador | latencia de despertar | metro si, dueno uno |

### Las dos confusiones que hay que matar antes de usar la tabla

**1. Latencia y throughput son el mismo trabajo con otro denominador, y se
optimizan en direcciones OPUESTAS.** Lotes, colas profundas y buferes suben el
caudal y suben la latencia individual. Quien pida las dos cosas en el mismo
camino esta pidiendo que alguien elija a escondidas.

**2. Cache y memoria son dos ejes con un nombre.** *Huella* es cuanta RAM ocupo
--y hoy sobra: 14,8 GiB libres contra 5,4 MiB usados--. *Localidad* es cuantas
lineas de 64 B toco y en que orden, y no tiene nada que ver con el tamano total.
Y hay un dato que lo cambia todo: **un fallo a DRAM cuesta del orden de una
puerta entera** (ver R-CACHE1). O sea que el eje CACHE no es paralelo al de
ciclos: **es el sumando que no estas viendo**.

### ** "Ordenar por DONDE SE USA MAS" no es un eje: es el MULTIPLICADOR

Es la pregunta que hay que contestar antes de usar la tabla, porque parece un
eje y no lo es. *"Veces por segundo"* es el **segundo factor** de la aritmetica
del censo, y multiplica a unos ejes y a otros no:

| eje | la unidad | la multiplica el uso? |
|---|---|---|
| CICLOS | ciclos por vez | **SI** -> ciclos/s |
| CACHE | fallos por vez | **SI** -> fallos/s |
| ENERGIA | julios por vez | **SI** -> vatios |
| THROUGHPUT | ya es por segundo | **ya viene multiplicado** |
| **TAMANO** | bytes, una vez | ** **NO. Y es el unico** |

**Un binario ocupa lo mismo si se ejecuta una vez o un millon.** El tamano no se
paga por uso: se paga por **tener que caber**. Por eso `MAX_BEX` y el marco de
pila no se ordenan por frecuencia, se ordenan por **distancia a su techo** -- y
por eso un camino que se recorre una sola vez (la carga de un `.bex`, el
arranque) puede estar tachado en ciclos y **vivo en tamano al mismo tiempo**.

[!] **Y el puente, que es donde se cruzan:** el tamano **dentro de un camino
caliente deja de ser tamano y se convierte en CACHE**. Un bucle que no cabe en
los 32 KB de L1i paga fallos de instruccion en cada vuelta. O sea que la unica
excepcion a *"el tamano no se multiplica por el uso"* la cobra otro eje, no el
suyo.

**Como se ordena entonces, en la practica:**

```
   para CICLOS / CACHE / ENERGIA   ordenar por VECES POR SEGUNDO
   para TAMANO                     ordenar por % DE SU TECHO   (>90% = roto)
   para THROUGHPUT                 no se ordena: se compara contra el ancho de banda
```

### El orden de precedencia en esta casa

```
   CORRECCION                                   siempre, y no es un eje
   > LATENCIA      en la superficie del sistema (la puerta, los traps)
   > TAMANO        en lo que se carga y en lo que vive en la pila
   > THROUGHPUT    en lo que mueve datos (blit, disco, red)
   > CACHE         sin juez: hoy no se puede invocar para ganar una discusion
   > ENERGIA       un solo dueno declarado (el ocio y AXION)
```

** **CUMPLIR EL TECHO Y NO LA META NO ES ESTAR BIEN: ES ESTAR EN PLAZO.** Es la
frase de `presupuesto.rs` y vale para los cinco ejes.

### El estado real de los ejes, sin adornos

| eje | lo que hay hoy | lo que falta para que muerda |
|---|---|---|
| LATENCIA | doble testigo (`sys/precio.bex` y `c/coste.bex`, coinciden en 1 ciclo), juez `bmo-juicio` con 16 pruebas fuera del metal, 3 filas con techo/meta/porque, margen de ruido 5% | nada. Es el modelo |
| TAMANO | el build IMPRIME los tamanos; `MAX_BEX` = 4 MiB; pila de Ring 3 = 65.536 B | trinquete y marco maximo por funcion con `llvm-objdump`. **Ojo: con LTO el tamano SALTA, no crece suave -- el margen del 5% del ruido no sirve aqui** |
| THROUGHPUT | numeros sueltos medidos a mano (blit ~300 MB/s, fps de DOOM) | ventana declarada limpia, fila y juez |
| CACHE | **nada**: no hay PMC en el arbol (`rdpmc`/`PERFEVTSEL` no aparecen) | leer los contadores de rendimiento. Proyecto aparte |
| ENERGIA | RAPL leido de verdad: milivatios de paquete y de nucleo, con la unidad preguntada al chip | un dueno y un antes/despues (`smp stop`) |

---

## 3. Los componentes

### C1 -- CPU: el nucleo de ejecucion

**PARA QUE EXISTE.** Es la unica pieza que no puede esperar a nadie sin que todo
lo demas espere. Todo lo que hay en la caja existe para alimentarlo o para
recoger lo que produce.

**QUE EXIGE.**

```
   [SILICIO]   Ryzen 5 5600X (Zen 3, Vermeer), 6 nucleos / 12 hilos, un CCD
   [MEDIDO]    rdtsc                             69 -- 107 ticks (ver R-TIME6)
   [MEDIDO]    puerta pelada (INVOKE, min)       884 ticks = 240 ns
               los dos testigos: 889-4 = 885 y 926-43 = 883. Dos ticks.
   [MEDIDO]    dispatch (la mitad en Rust)       87 (C) / 104 (Rust)
   [MEDIDO]    el stub (por resta)               785 -- 839, o sea el 89-91%
   [MEDIDO]    resolver un handle                +166
   [MEDIDO]    una operacion mas gorda           +68
   [MEDIDO]    una llamada a funcion normal      19 ticks  <- la referencia
   [ANALISIS]  cruce syscall+sysretq             ~150 ciclos, IRREDUCIBLE
```

** Los ~150 del cruce no son un objetivo: **son el suelo**. Salen de que un
`rdtsc` mide 69 y `syscall`/`sysret` son de la misma familia microcodificada
pero hacen mas, y coinciden con lo que Liedtke consiguio con L4 en un 486 en los
noventa. **El coste de cruzar un anillo de privilegio es lo unico de esta cuenta
que no ha bajado en treinta anos.**

Y exige tres cosas mas que no son de rendimiento sino de verdad:

- **La pareja guardar/restaurar no es simetrica.** `XSAVE` hace *merge* de
  `XSTATE_BV` (no *store*) y deja intactos los 48 bytes reservados de la
  cabecera; `XRSTOR` da `#GP(0)` si no estan a cero. Ponerlos a cero es deber
  del software, una vez, al crear el area.
- **`setcc` escribe 8 bits.** `sete al` sobre un `rax` negativo deja los 56 de
  arriba puestos.
- **El CPU cachea el descriptor de CS.** Un `lgdt` sin far-jump deja al nucleo
  ejecutando con el descriptor del firmware.

**LAS REGLAS.**

- **R-CPU1.** El cruce de anillo se paga **una vez por operacion util**. Nada de
  un syscall por pixel, por byte o por caracter.
- **R-CPU2.** Nada entra en el camino del syscall sin **fila en el presupuesto**
  (techo, meta y porque). Un numero sin contrato es una anecdota.
- **R-CPU3.** Toda instruccion con pareja se lee en el manual **campo a campo y
  preguntando si hace merge o store**. No se asume que la de guardar deja el
  bufer completo.
- **R-CPU4.** Un valor que tiene que sobrevivir a una llamada ajena va a una
  **ranura de pila**, no a un registro. `r10`/`r11` son el arcen de todo el
  mundo.
- **R-CPU5.** Un resultado de rendimiento sin **doble testigo** no es un numero,
  es una opinion de un programa.
- **R-CPU6.** ** **UN NUMERO SIN SU UNIDAD NO ES UNA MEDIDA.** `rdtsc` cuenta
  TICKS de un reloj invariante, no ciclos de nucleo: en esta maquina son 1,22
  ciclos por tick, o sea un 22% de diferencia entre lo que se imprime y lo que
  paga el CPU. **Se dan las dos**, y el presupuesto se juzga en la unidad en que
  se midio -- convertir antes de comparar contra un techo lo moveria cada vez
  que el CPU cambia de frecuencia, que es lo unico que un trinquete no puede
  hacer. Ver R-CENSO0 y `bmo-juicio::Reloj`.
- **R-CPU7.** **** **ANTES DE OPTIMIZAR UN CAMINO, CONTAR SUS INSTRUCCIONES.** La
  via rapida del stub son **58 instrucciones** y una puerta cuesta **969
  ciclos**: aun a un IPC de 1, el 94% del coste **no puede estar** en el numero
  de instrucciones. Esa resta tacha de golpe el trabajo de limar el ensamblador
  y deja la lista corta -- las transiciones de privilegio, los `swapgs` y la
  mitad Rust. Contar es gratis; suponer cuesta tandas.

- **R-CPU8.** **** **UN PRESUPUESTO TIENE DUENO: LA MAQUINA EN QUE SE MIDIO.** Un
  techo en ticks pertenece a un CPU y a un TSC concretos; el mismo kernel
  arranca en cualquier x86-64, y alli esos numeros no son estrictos ni laxos,
  son **de otra maquina** -- falsa regresion en un CPU mas lento, falso aprobado
  en uno mas rapido. Asi que la tabla vive **en el perfil** (`cpu_vendor/`), no
  en el kernel, y declara familia, modelo y TSC. Si el silicio no cuadra, las
  filas contestan `sin declarar` y **el juez se calla**. Estrenar un CPU es
  copiar el perfil y pegar tres cifras medidas: cero lineas de kernel.
- **R-CPU9.** ** **UN "NO COINCIDE" LLEVA LOS DOS LADOS.** Lo esperado y lo leido,
  en el mismo campo. Un `bool` frena el trinquete y no lo arregla: obliga a leer
  codigo para saber si fallo el modelo o el reloj. Con los dos numeros delante,
  el arreglo es cambiar una cifra. *(Lo pago el mismo dia: el arbol declaraba el
  modelo de este chip en dos sitios con valores distintos --`19h/01h` y
  `19h/21h`-- y nadie habia leido nunca el byte.)*

- **R-CPU10.** **** **EL SUELO SE MIDE, EL MULTIPLICADOR SE ESCRIBE.** Una medida
  de rendimiento son dos cosas pegadas: el **suelo** del silicio (cruzar el
  anillo, que BMO no puede cambiar) y el **sobrecoste** que BMO anade encima.
  Solo el segundo es merito o culpa de este kernel, y **es el unico que sobrevive
  a un cambio de CPU**. Asi que el suelo puede autocalibrarse --es un dato del
  CPU-- y el multiplicador que lo convierte en techo lo escribe una persona.
  *Un presupuesto que se recalibrara solo entero se ceniria a lo que hubiera,
  **incluida una regresion**: la convertiria en la talla nueva y aprobaria
  siempre.* **Un trinquete que se ajusta solo no es un trinquete.**
- **R-CPU11.** ** **UNA CIFRA DERIVADA NO SE PRESENTA COMO MEDIDA.** Un techo
  sacado de `suelo x multiplicador` es una PRIMERA TALLA: sirve para tener
  trinquete el dia uno en una maquina nueva, y lo sustituye la medida en cuanto
  haya una tanda. Quien lo imprime dice cual de las dos es -- si no, una
  estimacion acaba citandose como un hecho, que es como nacieron los `~150`
  ciclos de cruce que este arbol arrastro cuatro tandas.

** Ver `docs/componente/LA_PUERTA_POR_DENTRO.md`: los once elementos de una puerta con su
fichero, su coste **en ciclos**, y el experimento que decide cada uno.

**EL PRECIO.** El `#GP(0)` en `xrstor64` costo cinco fotos y dos explicaciones
falsas. `r11` se pago **tres veces**. `!(-6)` valia `-256`, y por eso
`if (!strcmp(a,b))` acertaba siempre que `a` fuera alfabeticamente menor que
`b`: DOOM iba a escribir su configuracion encima del WAD.

---

### C2 -- CACHE: la linea de 64 bytes

**PARA QUE EXISTE.** Porque la DRAM es lentisima comparada con el nucleo. La
cache es **la unica razon de que un CPU moderno no este parado la mayor parte
del tiempo**. No es una optimizacion del hardware: es la premisa sobre la que se
diseno el hardware.

**QUE EXIGE.**

```
   [SILICIO]   L1d   32 KB, 8 vias, linea 64 B, por nucleo
   [SILICIO]   L1i   32 KB, 8 vias, linea 64 B, por nucleo
   [SILICIO]   L2   512 KB, 8 vias, linea 64 B, por nucleo (victima)
   [SILICIO]   L3    32 MB, 16 vias, linea 64 B, COMPARTIDA por los 12 hilos
```

** **La linea de 64 B es el atomo de toda la maquina.** No existe "leer 8 bytes":
existe traer 64. Tocar un `u8` cuesta lo mismo que tocar los 64 vecinos, y
tocar dos `u8` separados por 64 bytes cuesta el doble que tocarlos juntos.

```
   [LITERATURA]  L1 ~4-5 ciclos | L2 ~12-14 | L3 ~46 | DRAM ~70 ns
   [LITERATURA]  a ~4,6 GHz, esos 70 ns son del orden de 300 ciclos
```

[!] **Los cuatro numeros de arriba son de fuera y NO estan medidos aqui**, y esa
es la carencia mas grande de este documento. La sonda que los convierte en
`[MEDIDO]` ya se puede escribir con lo que hay: un recorrido de punteros que
salte mas de 64 B por paso sobre buferes de 16 KB / 256 KB / 8 MB / 128 MB,
cronometrado con el mismo metro de `c/coste.bex`. **Cuatro numeros, una tarde.**

**LAS REGLAS.**

- **R-CACHE1.** Un fallo a DRAM cuesta **del orden de una puerta entera**. Por
  tanto: **todo analisis de ciclos que no diga cuantas lineas de 64 B toca el
  camino esta incompleto.** 872 ciclos de puerta son unos tres fallos.
- **R-CACHE2.** Lo que se recorre junto vive junto. La eleccion entre "array de
  structs" y "struct de arrays" **la decide el bucle que los recorre**, no el
  gusto de quien declara el tipo.
- **R-CACHE3.** **Ningun dato escrito por dos nucleos comparte linea.** El
  relleno se declara con el 64 escrito **una sola vez** en una constante con
  nombre, nunca a mano en cada struct.
- **R-CACHE4.** Mientras no haya PMC, **el eje CACHE no se puede invocar para
  ganar una discusion**. Se puede razonar sobre el (R-CACHE1 y R-CACHE2 son
  geometria, no medida), pero nadie puede decir "esto mejoro la cache" sin
  numero. Es L0 aplicada a este componente.

**EL PRECIO.** Ninguno visible todavia -- **y eso es exactamente la alarma**. Un
componente que nunca ha aparecido en una autopsia en un sistema que ya lleva
cientos de miles de ciclos analizados no es un componente sano: es un componente
que **no se esta midiendo**.

---

### C3 -- RAM y MMU: el quirofano y la frontera

**PARA QUE EXISTE.** La RAM es donde el programa **esta trabajando**, no donde
vive. La MMU es lo unico que impide que un programa toque a otro; sin ella no
hay Ring 3, no hay capabilities y no hay sistema operativo, hay un cargador.

**QUE EXIGE.**

```
   [SILICIO]   pagina                     4096 B
   [SILICIO]   escribir CR3 tira el TLB entero (salvo PCID)
   [SILICIO]   un motor DMA no entiende de direcciones virtuales
   [MEDIDO]    pila de Ring 3             65.536 B (16 paginas)
   [MEDIDO]    MAX_BEX                    4 MiB (ha subido varias veces)
```

**LAS REGLAS.**

- **R-RAM1.** Un motor DMA recibe direcciones **FISICAS**, y el nombre de la
  funcion lleva el contrato: `read_sectors_phys`, no `read_sectors`.
- **R-RAM2.** El estado que vive todo el programa va a `.bss`. La pila es para
  lo temporal. Con 64 KiB de pila o con 1 MiB, subir la pila **solo mueve el
  dia**.
- **R-RAM3.** ** Mover un struct a `.bss` **no baja la pila por si solo**. Lo que
  la baja es que **ningun valor grande cruce una frontera de funcion**: ni como
  retorno, ni como argumento, ni como temporal de un literal. Las tres hay que
  cerrarlas, y se verifica midiendo **el marco maximo del binario funcion por
  funcion**, no el de `_start`.
- **R-RAM4.** Reflejar, no copiar. Una copia que existe solo para cambiar de
  dueno es trabajo que la maquina no tiene por que hacer.
- **R-RAM5.** Todo tope estatico lleva **margen medido, no margen sentido**. Al
  90% ya esta roto; solo falta la linea que lo empuje.

**EL PRECIO.** El compositor estuvo muerto **cinco commits** por un refactor
correcto: 52 locales agrupadas en un struct pasaron el marco de `_start` de
35.560 a **95.544** con 65.536 de pila. El `rip=0x4000001B` era la sonda de pila
que emite LLVM cuando el marco es grande. Y el arreglo obvio --moverlo a
`.bss`-- **volvio a desbordar** por los temporales. Aparte: `MAX_BEX` estuvo al
94% sin que nadie mirara, y una linea nueva puso el compositor en 82 KiB de
golpe.

---

### C4 -- BUS, MMIO y PCIe: hablar con lo que no esta dentro del CPU

**PARA QUE EXISTE.** Los aparatos no viven en el nucleo. Un registro de un
controlador es una direccion que **no es memoria**: es un cable.

**QUE EXIGE.**

- **El MMIO es UC por defecto** (lo dejan asi los MTRR del firmware): cada
  escritura es una transaccion de bus por su cuenta.
- **Una LECTURA de MMIO es no-posted**: va y vuelve por el bus. Una escritura se
  suelta y sigue; una lectura **para el nucleo hasta que el aparato conteste**.
  No estan en la misma liga y no se pueden tratar igual.
- **Un rango declarado mayor que el real ALIASEA.** Los puertos que no existen
  devuelven lo mismo que otros que si.
- **El agujero de MMIO no esta mapeado en el CR3 de usuario.**

**LAS REGLAS.**

- **R-BUS1.** Leer del MMIO dentro de un bucle caliente esta **prohibido**. Se
  lee una vez, se guarda, y si hace falta refrescar se dice cada cuanto y por
  que.
- **R-BUS2.** Ningun camino que escriba MMIO corre bajo **contexto ajeno** sin
  cambiar CR3, con un solo camino de salida. (El arreglo barato pendiente:
  mapear el agujero de MMIO como supervisor en todo espacio de direcciones y
  ahorrarse los dos vaciados de TLB por llamada.)
- **R-BUS3.** Todo campo de bits del hardware se cita **con su rango del
  manual** en el sitio donde se lee. Un limite de enumeracion mal leido **no es
  cosmetico si el bucle escribe**.
- **R-BUS4.** Valores duplicados con periodo potencia de dos = **aliasing de
  MMIO**, no log repetido.

**EL PRECIO.** `CAP.NP` se leia del bit equivocado: 20 puertos donde hay 8, y
como `port_link_up` **escribe**, cada puerto fantasma mandaba un COMRESET a un
puerto real ya levantado. Y el teclado de Ring 3 murio con `#PF err=0x2` en el
tick 144 porque `poll_ascii` escribe el `ERDP` del xHCI y el camino se recorrio
por primera vez **desde dentro de un SYSCALL**.

---

### C5 -- FRAMEBUFFER y GPU: la unica salida que existe

**PARA QUE EXISTE.** Es por donde sale el trabajo. Y en esta casa tiene un peso
extra: **el escritorio no tiene salida** --al shell de Ring 0 no se vuelve--,
asi que lo que no se pinta, no ocurrio.

**QUE EXIGE.**

- Es un **BAR de PCIe**, no RAM. Los MTRR del firmware lo dejan en UC: sin
  arreglo, cada pixel es una transaccion de bus.
- ** Ya esta arreglado: `s1_cpu::init_pat` deja **una entrada del PAT en
  Write-Combining**, que es el camino de `ioremap_wc()` de Linux. Con MTRR=UC y
  PAT=WC, el tipo efectivo es WC. La secuencia del manual (apagar cache, vaciar,
  desarmar MTRR, tirar TLB, escribir PAT, tirar TLB, rearmar, vaciar, encender)
  **no es opcional**: saltarse un paso no da error, da una maquina que se cuelga
  o corrompe memoria mas tarde.
- **Write-Combining junta escrituras SEGUIDAS.** Saltar rompe el bufer de
  combinacion y se pierde todo el beneficio.
- **Leer de memoria WC es carisimo**: no cachea, y cada lectura es un viaje.

```
   [MEDIDO]      blit a 1600x1000              ~300 MB/s
   [ARITMETICA]  1600x1000x4 = 6,4 MB/frame -> 60 fps pide 384 MB/s
```

**LAS REGLAS.**

- **R-FB1.** ** **Del framebuffer NO SE LEE. Jamas.** Ni para mezclar, ni para
  leer un pixel, ni para "ver que habia". El doble bufer vive en RAM; el
  framebuffer es de **una sola direccion**.
- **R-FB2.** Se escribe **secuencial y por lineas completas**, para que el WC
  tenga algo que juntar.
- **R-FB3.** El eje del framebuffer es **THROUGHPUT**, no latencia. Nadie espera
  un pixel; se espera un frame.
- **R-FB4.** Ningun camino que pinta corre bajo CR3 ajeno (es R-BUS2, y aqui se
  repite porque es donde mas veces ha reincidido).

** Ver `docs/componente/EL_COMPOSITOR_Y_EL_ESCANER.md`: los dos relojes que nadie
sincroniza, y la aritmetica que dice que **volcar la pantalla entera (27,6 ms)
dura mas que un frame de video (16,7 ms)** -- o sea que el escaner alcanzaria al
volcado siempre, por construccion. Lo unico que hoy lo sostiene es que no se
vuelca la pantalla: se vuelcan hasta ocho cajas de lo tocado.

**EL PRECIO.** El primer `.bex` de hola-mundo **si ejecuto**: murio pintando. El
framebuffer no estaba mapeado bajo CR3 de usuario, el flush daba `#PF`, **y el
reporte de faults tambien pintaba** -> `#PF` recursivo -> congelacion total. Y
antes que eso: los GUID de GOP estaban mal copiados a mano, asi que **el
framebuffer no funciono nunca y nadie lo noto porque el proyecto corria por
serie**.

---

### C6 -- ALMACENAMIENTO (AHCI / SATA): lo unico que sobrevive al apagon

**PARA QUE EXISTE.** Es el unico componente cuyo contenido sigue ahi cuando se
va la luz. Todo lo demas es volatil por diseno.

** **Y en esta maquina el almacenamiento tiene una regla que no es tecnica:** el
NVMe es el Windows del dueno. BMO-X vive en el Kingston SATA. La escritura al
NVMe esta **cerrada a proposito**.

**QUE EXIGE.**

```
   [SILICIO]  sector                        512 B, unidad indivisible
   [SPEC]     lista de comandos             alineada a 1024 B
   [SPEC]     area de FIS                   alineada a 256 B
   [SPEC]     tabla de comandos             alineada a 128 B
   [SPEC]     PRD: direccion par (word), DBC = bytes MENOS UNO, max 4 MB
   [SPEC]     el DMA quiere FISICAS         (ver R-RAM1)
   [HECHO]    el registro PI del firmware MIENTE
   [HECHO]    el firmware PARA los puertos SATA al salir de boot services
```

Y una exigencia de forma, no de numero: **un disco da caudal cuando tiene cola**.
Una peticion en vuelo desperdicia el aparato; su latencia es del orden de
microsegundos-milisegundos y **no se arregla con ciclos de CPU**.

**LAS REGLAS.**

- **R-DISCO1.** **Identidad antes de escribir.** Un dispositivo que no puede
  decir quien es no se escribe. El superbloque de ESTRATOS graba el `disco_id`
  dentro del volumen justamente para poder negarse a escribir en un clon.
- **R-DISCO2.** Todo campo de conteo del hardware se cita con **su unidad y su
  sesgo**: `-1`, exponente, milisegundos. Cuando el hardware dice OK y no pasa
  nada, se relee la spec **del campo**, no de la operacion.
- **R-DISCO3.** El eje del disco es **THROUGHPUT**. Optimizar su latencia con
  ciclos es trabajar en la columna equivocada.
- **R-DISCO4.** Un enumerador que devuelve vacio se barre **por fuerza bruta**
  contra lo que declara el registro de capacidad, ignorando el mapa.
- **R-DISCO5.** ** Si un modulo tiene dos caminos al mismo dispositivo (uno lento
  con copia, otro directo), **la conversion de coordenadas va en una funcion que
  ambos tengan que atravesar**.

** Ver `docs/componente/EL_DISCO_EXIGE.md`: **medio, ranura y aparato son tres
preguntas distintas** --gira o no gira, cuantos comandos caben en vuelo, y que
trae dentro-- y el arbol las trataba como una. Alli van `R-DISCO6..10` y el
perfil de almacenamiento, con la doctrina de R-CPU8: se PREGUNTA lo que el
aparato responde y se DECLARA solo lo que calla.

**** Y el hueco que destapo, porque es de este componente: **BMO-X no ha leido
nunca la palabra 217 del IDENTIFY**, que es la que dice si el medio es
rotacional. Se razona sobre TRIM y sobre colas sin haber comprobado el hecho del
que dependen las dos. Es L5 incumplida, y esta a una lectura de 16 bits en un
buffer que ya se pide.

**EL PRECIO.** `PI` declaraba los puertos 0,1,4,5 y el disco estaba en el 2. Y la
suma `+ part_lba` estaba escrita cuatro veces y **faltaba en los tres sitios del
camino rapido**: un `.bex` se leia de dentro de la ESP -- codigo x86-64 real y
ajeno. Dos dias. La firma era *"el directorio se lee bien y el contenido no"*.

---

### C7 -- USB / xHCI: por donde entra la voluntad del usuario

**PARA QUE EXISTE.** Sin esto no hay teclado, y sin teclado el sistema no es de
nadie. Es el componente mas hostil de la caja porque **el que manda el tiempo es
el bus, no el CPU**.

**QUE EXIGE.**

```
   [SPEC]   microframe (High Speed)      125 us   <- el bus manda el reloj
   [SPEC]   frame (Full/Low Speed)         1 ms
   [SPEC]   el campo Interval es un EXPONENTE: 2^n x 125 us
   [SPEC]   el anillo de eventos es UNO para TODO el controlador
   [SPEC]   un reset de puerto GENERA un evento de cambio de puerto
   [HECHO]  un aparato tarda en engancharse lo que a el le da la gana
```

** Y la exigencia que convierte un descuido en un aparato muerto: **en un
endpoint de interrupcion, el evento ES el permiso para volver a encolar.** Tirar
un evento ajeno no pierde una pulsacion: **para la bomba para siempre**, dejando
el endpoint en `Running` y sin un solo error.

**LAS REGLAS.**

- **R-USB1.** De una cola compartida, lo que se saca y no es mio **se APARCA,
  nunca se tira** -- y los que se pierden por aparcadero lleno **se cuentan y se
  exponen**. Un contador que tiene que ser cero es una aguja gigante.
- **R-USB2.** Todo descubrimiento de hardware necesita **segunda oportunidad**, y
  la buena es **reactiva** (el aviso del propio bus), no un reintento a ciegas.
- **R-USB3.** Antes de responder a un evento del hardware con una accion **sobre
  ese mismo hardware**, preguntar si la accion regenera el evento. Si puede,
  hacen falta **las dos guardas**: no tocar lo que ya funciona (estado) y un tope
  de intentos (contador). Una sola no basta.
- **R-USB4.** No se enciende una bomba mientras todavia se enumera.
- **R-USB5.** Un recurso pedido en un camino que puede fallar **se devuelve en la
  MISMA funcion** que lo pidio.
- **R-USB6.** **** **UNA AVERIA VIVA ES UN ESTADO, NO UN EVENTO.** Un `fault()` se
  dice una vez e informa a quien ya estaba mirando; una averia que **sigue
  ocurriendo** necesita un indicador encendido mientras dure, y **en el sitio
  donde vive el dueno** -- el escritorio, no un log que hay que abrir.
  *"El bus no late"* no es una noticia: es una condicion, y una condicion se
  pinta como una luz. Es el patron 33 con una vuelta mas: alli el motivo salia
  por un canal cerrado; aqui sale por uno abierto **pero una sola vez**. Aplica
  igual a `sin RAPL`, `disco no listo` y `fugas > 0`.
- **R-USB7.** Un endpoint **parado** (`Halted`) no se reintenta: **se resucita**,
  y en el orden de la spec -- `Reset Endpoint`, luego `Set TR Dequeue Pointer`,
  y solo entonces encolar y tocar el timbre. El xHC **ignora el timbre de un
  endpoint parado**, asi que reintentar sin resucitar es tocar un timbre roto; y
  resetear sin recolocar el puntero deja el endpoint leyendo TRBs viejos.

** Ver `docs/componente/EL_TECLADO_EXIGE.md`: las **seis exigencias** del teclado con su
estado y, sobre todo, **el numero que dice cual fallo**. Las seis estan puestas
desde el 2026-08-17: R-USB6 son `INFO_USB_SALUD` + `INFO_USB_AVERIAS`
(`dev/usb/salud.rs`) y la luz fija de la barra (`scene/testigo.rs`). Sin
verificar en metal.

- **R-USB8.** ** **UN ESTADO QUE SE FOTOGRAFIA NECESITA UNA EDAD AL LADO.** La
  salud del bus se saca en el bombeo --el unico sitio con el PML4 del kernel
  cargado, que es donde se puede leer el Device Context y `USBSTS`-- y por tanto
  **se congela si el bombeo muere**, contestando *"todo bien"* justo el dia malo.
  La edad del ultimo latido viaja pegada a los bits y se calcula **al
  preguntar**: es lo unico que envejece solo, o sea lo unico que puede delatar al
  que escribe la foto. Vale para cualquier telemetria cacheada, no solo para el
  USB.
- **R-USB9.** ** **UNA LUZ QUE SOLO APARECE CUANDO HAY AVERIA NO SE DISTINGUE DE
  UNA LUZ QUE NO FUNCIONA.** Si la primera vez que se ve es el dia del fallo,
  nadie sabe que aspecto tenia sana y lo que diga no se puede creer. El testigo
  se pinta **siempre**, tambien en verde -- la misma razon por la que la ficha de
  CABINA esta siempre aunque su ventana este cerrada.

**EL PRECIO.** Un teclado programado a **35 minutos** entre sondeos porque el
`bInterval` crudo se escribio donde iba un exponente -- y Configure Endpoint
devolvio EXITO. Un informe del raton leido como *"el comando salio bien"*. 445
eventos perdidos y el teclado muriendo a los pocos segundos por un bucle de
reset, **delatado por el RGB del raton parpadeando**: un periferico que reinicia
su firmware esta contando las vueltas de tu bucle a la vista.

---

### C8 -- RELOJES: el componente del que depende este documento entero

**PARA QUE EXISTE.** Sin reloj no hay medida, y sin medida **ninguna regla de
este fichero puede existir**. Es el unico componente que se mide a si mismo.

**QUE EXIGE.**

```
   [SILICIO]  TSC invariante en Zen 3: cuenta igual aunque el nucleo cambie de
              P-state -- por eso vale como metro
   [MEDIDO]   rdtsc                        69 ciclos  <- EL METRO PESA
   [MEDIDO]   una puerta de consola        ~2,2 M ciclos
   [SPEC]     rdtsc no serializa por si solo
   [SPEC]     el HPET declara su direccion en un GAS: el campo esta en +44
   [SPEC]     los contadores de energia son de 32 bits y dan la vuelta (~16 min)
```

**LAS REGLAS.**

- **R-TIME1.** El metro se resta a si mismo. Toda ventana declara si el coste del
  instrumento esta dentro. (De los ~90 ciclos de `dispatch`, buena parte **son
  los dos `rdtsc`**; por eso su meta de 60 pasa por sacar el metro de ahi, no
  por afinar un `match` de dos brazos.)
- **R-TIME2.** ** **Una ventana de medida no contiene una puerta de consola.** Se
  declara `cerrada_sin_imprimir`, y quien lo ponga en `false` **pierde el
  veredicto**, que es lo correcto. El juez no puede comprobarlo: los numeros de
  una ventana sucia son **coherentes y falsos**.
- **R-TIME3.** La respuesta es el **MINIMO**; la media mide al planificador.
  Restar una media de un minimo esta permitido **solo diciendo la direccion del
  sesgo**.
- **R-TIME4.** Un contador es una **diferencia entre dos instantes**. Leerlo una
  vez da el total desde el arranque, y eso no es una medida de ahora.
- **R-TIME5.** Dos testigos independientes o no hay numero.
- **R-TIME6.** **** **UN TICK DE TSC NO ES UN CICLO DE NUCLEO, y la maquina lo
  dice con sus dos instrumentos a la vez.** El TSC invariante cuenta a la
  frecuencia BASE --3.700 MHz-- mientras el nucleo corre a la que le deje el
  boost:

  ```text
     [MEDIDO]  reloj base   3700 MHz   el TSC
     [MEDIDO]  reloj ahora  4519 MHz   MPERF/APERF
               1 tick = 0,27 ns = 1,22 ciclos de nucleo a esa frecuencia
               884 ticks = 240 ns = ~1.086 ciclos de nucleo
  ```

  Por tanto **todo numero medido con `rdtsc` se divide entre 3.700 M/s, nunca
  entre la frecuencia de boost**. Mezclarlas da un porcentaje que parece
  razonable y es falso por un 22%. Es el patron 2 de la casa --un campo que
  viene en otra unidad-- y se pago aqui: la primera version de
  `docs/CENSO_DE_EJES.md` uso el denominador equivocado.
- **R-TIME7.** ** **Lo que cuesta una instruccion NO es una constante: depende de
  lo que tenga alrededor.** El mismo `rdtsc` midio **107 en un bucle de 4 ticks
  y 69 en uno de 43** -- en el bucle largo el CPU fuera de orden lo solapa con el
  trabajo de al lado. Se dan **los dos numeros**, nunca una media que no describe
  ninguno de los dos casos. Y cuando el instrumento cueste tanto como lo medido
  --`dispatch` mide 87-104 con un `rdtsc` de 69-107 dentro-- **la fila no se
  puede leer**, por bien que se comporte el numero.

**EL PRECIO.** `dispatch` "media" 309-319 ciclos durante **cuatro tandas**. Era
un `printf` disfrazado de dispatcher: 225 ciclos por puerta metidos dentro de la
ventana. Con la ventana limpia son 84 y 99. Y antes de encontrarlo se dieron
**dos explicaciones falsas**, una de ellas *"una parte no puede exceder al
todo"* -- falsa, porque una era media y el otro minimo.

---

### C9 -- INTERRUPCIONES (APIC / MSI): la alternativa a preguntar

**PARA QUE EXISTE.** Para no sondear. Sondear gasta un nucleo entero para
enterarse de algo que ocurre mil veces por segundo.

**QUE EXIGE.**

- El manejador corre **sobre la pila del interrumpido** (o la de `TSS.RSP0`), a
  profundidad variable y en cualquier momento.
- MSI se **arma antes** de abrir la puerta del aparato, nunca despues.
- Todo lo que tarde dentro de un manejador **se lo quita a todos los demas**.

**LAS REGLAS.**

- **R-IRQ1.** Un manejador **apunta y sale**. No imprime, no pinta, no toma
  cerrojos largos, no llama a nada que cruce el bus mas de lo imprescindible.
- **R-IRQ2.** Armar antes de abrir.
- **R-IRQ3.** ** **Guardar estado antes de decidir si el cambio ocurre es la
  clase de bug, no el sintoma.** Un contexto anotado como vigente que el epilogo
  ya consumio es pila libre con una etiqueta mintiendo encima. El sello es de
  **un solo uso** y el epilogo lo borra.

**EL PRECIO.** Tres dias y tres fotos por `schedule_locked`. La tarea 0 --el
shell, que corre en la pila de arranque y se interrumpe a profundidad variable--
se comia 1256 B de su propia area. Aparecio de golpe al llegar el compositor,
que cede miles de veces por segundo.

---

### C10 -- ALIMENTACION Y TERMICA: el limite real de un CPU moderno

**PARA QUE EXISTE.** Cada transistor que conmuta convierte energia en calor. En
un chip de 2020 **el limite no es el reloj: es el vatio y el grado**. El boost
sube la frecuencia mientras haya presupuesto termico y electrico; cuando no lo
hay, baja. Por eso "cuantos ciclos tarda" y "cuanto consume" no son dos temas:
son el mismo tema mirado dos veces.

**QUE EXIGE, y aqui va la verdad incomoda.**

** **BMO-X no pone voltajes, y no deberia.** La cadena real es:

```
   VRM de la placa  ->  entrega la corriente y fija el voltaje fisico
   SMU del chip     ->  lo negocia por SVI segun carga, grados y limites
   firmware         ->  programa los limites (PPT, TDC, EDC) al arrancar
   el SO            ->  PIDE P-states (frecuencia) y ELIGE C-states (residencia)
```

O sea que las palancas del sistema operativo son **dos**: que frecuencia pide y
**cuanto tiempo pasa cada nucleo sin hacer nada util**. Escribir que "el SO
controla el voltaje" seria falso, y este documento no puede permitirselo.

```
   [CATALOGO]  5600X: 65 W de TDP nominal
   [MEDIDO]    RAPL disponible en este silicio; la unidad se PREGUNTA al chip
               (1 unidad = 1/2^N julios, N leido de un MSR)
   [MEDIDO]    milivatios de paquete y de nucleo, por diferencia entre lecturas
   [HECHO]     apagar nucleos funciona; ENCENDER no -- falta MWAIT
```

**LAS REGLAS.**

- **R-PWR1.** ** **Un nucleo que espera no gira: se para.** Un bucle de espera sin
  `HLT`/`MWAIT` es un calefactor que ademas roba presupuesto de boost a los
  nucleos que si trabajan. *"Un obrero que espera no duerme, GIRA"* era la frase;
  ahora tiene numero al lado.
- **R-PWR2.** Toda afirmacion de consumo trae **dos lecturas y su intervalo**.
  Una sola lectura son julios acumulados desde el arranque, no vatios.
- **R-PWR3.** **Cero no es "no consume": es "no se puede medir"**, y se dice con
  esas palabras. Un TDP de catalogo puesto donde falta una medida es un numero
  plausible inventado, y un numero plausible se cree y se usa para decidir.
- **R-PWR4.** El eje ENERGIA tiene **un unico dueno declarado**: el ocio y AXION.
  En una maquina enchufada no se sacrifica latencia por vatios en ningun otro
  sitio. El dia que haya bateria, esta regla se reescribe **con su motivo**, no
  se amplia por costumbre.
- **R-PWR5.** ** Pedir P-states esta permitido; **tocar voltajes o subir limites
  electricos, NO.** No es una regla de estilo: un error de ciclos se diagnostica
  con una foto y se revierte con un commit; un error de voltaje se diagnostica
  con un chip muerto.

**EL PRECIO.** Ninguno todavia, y por una razon buena: el lector de energia se
escribio **antes** de que hiciera falta discutir. Lo que si estuvo tres dias sin
numero fue la frase de AXION sobre los nucleos que giran -- *"escrita desde el
08-11 sin un solo numero al lado"*.

---

### C11 -- FIRMWARE (UEFI): el componente que miente

**PARA QUE EXISTE.** Enciende la maquina, encuentra el disco, entrega un mapa de
memoria y un framebuffer, y se aparta. Es imprescindible y **no es de fiar**.

**QUE EXIGE.**

- El mapa que entrega es verdad **solo hasta `ExitBootServices`**.
- Los GUID son contratos de 16 bytes exactos.
- Al salir, el firmware **para** cosas que estaban en marcha (los puertos SATA).
- Deja los MTRR como el quiere y **el descriptor de CS cacheado** en el nucleo.
- Sus registros de capacidad describen a veces una maquina que no es esta.

**LAS REGLAS.**

- **R-FW1.** Nada que venga del firmware se copia a mano: GUID, offsets,
  versiones y layouts van a **constante compartida con guardian de drift**.
- **R-FW2.** Lo que el firmware **declara** se comprueba contra lo que el aparato
  **hace**. Si no coinciden, gana el aparato.
- **R-FW3.** Al tomar el mando se recarga **todo** el estado del CPU, CS
  incluido, con far-jump. Un `lgdt` sin recargar CS deja dos sintomas sin
  relacion aparente y una sola raiz.

**EL PRECIO.** `boot_context::VERSION=3` con s1 poniendo `version=2` a mano
tumbaba el arranque entero. El HPET se leia en `addr+40+8` en vez de `addr+44`.
Y el `swapgs` espurio mas el `#GP` del `iretq` **eran el mismo bug**: CS con el
descriptor de UEFI desde siempre.

---

### C12 -- SMP y topologia: seis nucleos que comparten una L3

**PARA QUE EXISTE.** Porque la frecuencia dejo de subir. La unica forma de hacer
mas trabajo por segundo es hacerlo en paralelo.

**QUE EXIGE.**

```
   [SILICIO]  6 nucleos / 12 hilos, UN solo CCD
   [SILICIO]  L3 de 32 MB COMPARTIDA por los 12 hilos
```

** Eso ultimo es una ventaja real de este chip y hay que decirla: **la
comunicacion entre dos nucleos de esta maquina pasa por una L3 comun**, no por
el Infinity Fabric entre dos CCD. Compartir datos entre nucleos aqui es
comparativamente barato -- **siempre que no compartan LINEA de escritura**
(R-CACHE3).

**LAS REGLAS.**

- **R-SMP1.** No se reparte nada mientras no haya **trabajo pesado que
  repartir**. Hoy no lo hay. Encender doce nucleos para que once giren es
  R-PWR1 al reves.
- **R-SMP2.** Dos nucleos que escriben el mismo dato **comparten linea** hasta
  que alguien lo impida. Ver R-CACHE3.
- **R-SMP3.** Despertar un nucleo dormido cuesta latencia, y esa latencia **se
  declara** antes de repartir nada por ella.

**EL PRECIO.** Una prediccion mia sobre `smp` la tumbo el metal. Queda escrito
aqui como recordatorio de que este componente **no se razona: se mide**.

---

## 4. Como se aplica esto a un fichero

Una ley que no se puede citar desde el codigo se convierte en L0 (prosa). El
formato es el mismo que ya usan las cabeceras de este arbol:

```rust
//! `syscall::dispatch` -- el reparto de una puerta.
//!
//! [eje]     LATENCIA -- paga TAMANO y CACHE
//! [fila]    DISPATCH (techo 105, meta 60)
//! [exige]   R-CPU1, R-CPU2, R-TIME1, R-BUS2
```

Tres campos y ninguno es opcional:

- **`[eje]`** dice **que se sacrifica**, no que se persigue. *"Este fichero es
  rapido"* no es una declaracion; *"este fichero puede gastar tamano para comprar
  latencia"* si.
- **`[fila]`** ata el fichero a una fila del presupuesto. **Un `[eje]` sin
  `[fila]` es L0**: un eje sin juez.
- **`[exige]`** lista las reglas que este fichero tiene que respetar por los
  componentes que toca. Es lo que lee quien vaya a modificarlo.

### Las cuatro reglas de aplicacion

- **A1.** ** **Un cambio que mejora un eje y empeora otro trae LOS DOS numeros o
  no entra.** Hoy una mejora se justifica con una medida; esto exige ademas la
  medida de lo que empeoro. Sin eso, "optimice" vuelve a ser una anecdota.
- **A2.** ** **Dos ejes en un mismo fichero significa que el fichero esta mal
  cortado.** Un `memcpy` no puede servir a la puerta (latencia) y al blit
  (caudal): o son dos, o uno declara que gana y el otro lo acata por escrito.
- **A3.** **Cambiar el `[eje]` de un fichero es un commit aparte**, con su
  motivo. Cambiar el eje es cambiar el contrato, igual que apretar un techo.
- **A4.** **El techo se aprieta con lo que YA se consiguio, nunca con lo que se
  cree que se va a conseguir.** Si se pone la estimacion y la pieza sale peor de
  lo previsto, el trinquete grita por una mejora.

### El guardian, y por que no lleva lista

La tabla del presupuesto nombra los ficheros de cada fila; el guardian barre
**esos** ficheros y exige que su cabecera diga el mismo eje que dice la tabla.

**La lista es la tabla, no una copia dentro del guardian.** Es la leccion que
`build.ps1` ya lleva escrita: *un guardian con lista tiene el mismo fallo que
vigila*. Y se prueba diciendo que no (L4).

---

## 5. Tabla de conformidad -- lo que hoy tiene dientes y lo que no

| componente | reglas | metro | juez | guardian | estado |
|---|---|---|---|---|---|
| C1 CPU | R-CPU1..5 | doble testigo | `bmo-juicio` | 4 en `build.ps1` | **COMPLETO** |
| C2 CACHE | R-CACHE1..4 | **ninguno** | no | no | **DECLARADO SIN JUEZ** |
| C3 RAM/MMU | R-RAM1..5 | tamanos en el build | no | no | falta trinquete + marco maximo |
| C4 BUS/MMIO | R-BUS1..4 | no | no | no | reglas de forma, verificables leyendo |
| C5 FRAMEBUFFER | R-FB1..4 | blit a mano | no | no | falta ventana declarada |
| C6 DISCO | R-DISCO1..5 | no | no | no | identidad SI cableada |
| C7 USB | R-USB1..5 | contadores de aparcadero | no | CABINA | contadores puestos |
| C8 RELOJES | R-TIME1..5 | el metro mismo | `bmo-juicio` | 16 pruebas | **COMPLETO** |
| C9 IRQ | R-IRQ1..3 | no | no | sellos de un solo uso | defensa puesta |
| C10 ENERGIA | R-PWR1..5 | RAPL | no | no | falta antes/despues de `smp stop` |
| C11 FIRMWARE | R-FW1..3 | no | no | drift guard | **el guardian ES el metro** |
| C12 SMP | R-SMP1..3 | no | no | no | sin trabajo que repartir |

** **Esta tabla es la parte mas honesta del documento y la que hay que mirar
primero.** Dos componentes completos de doce. Lo demas son reglas correctas sin
nadie que las obligue -- que es exactamente el estado en el que estaba el eje de
ciclos hace una semana, y por eso se sabe cuanto cuesta arreglarlo.

### El orden en que conviene cerrarlas

```
   1  TAMANO       marco maximo por funcion + trinquete de .bex   barato, ya sabes hacerlo
   2  CACHE        la sonda de latencias (4 numeros, una tarde)   desbloquea R-CACHE1
   3  ENERGIA      un antes/despues de `smp stop`                 el metro ya existe
   4  THROUGHPUT   ventana limpia para el blit                    el mas caro de los cuatro
```

---

## 6. Lo que este documento NO cubre, y por que

Un descarte con motivo se discute; uno sin motivo es un agujero. (Regla 0 de la
casa.)

- **Red.** No hay driver cableado. Cuando lo haya, es un componente entero con
  su eje --THROUGHPUT, y con la latencia como enemigo declarado-- y su seccion.
- **GPU de verdad (RDNA).** Hoy el unico camino a pantalla es el framebuffer.
  Vulkan esta **aparcado con plan escrito**, no descartado. El rasterizador
  propio es el **oraculo** con el que se juzgara a la GPU el dia que llegue.
- **Contadores de rendimiento (PMC).** Es lo que le falta a C2. No esta
  descartado: esta **pendiente y nombrado**, que es distinto.
- **Cifrado, RAID, POSIX.** Fuera del alcance por la regla de la esencia
  acotada.

---

## 7. La frase que ordena todo el documento

> **Un componente no negocia.** La linea son 64 bytes, el sector son 512, el
> microframe son 125 microsegundos y cruzar un anillo cuesta lo que costaba en
> 1995. Un sistema operativo no es el que decide esas cifras: es el que se
> organiza para no pelearse con ellas.
>
> Por eso estas reglas no son estrictas porque las firme el dueno. **Son
> estrictas porque el que las firma no lee este fichero.**

---

*Ver `docs/CENSO_DE_EJES.md` para la **aplicacion** de esta ley: que camino del
arbol gasta que recurso, con la aritmetica que TACHA lo que no hay que mirar.
Y `presupuesto.rs` para el eje de ciclos ya cableado, `docs/identidad/LA_RAM.md` para la
identidad de C3.*
