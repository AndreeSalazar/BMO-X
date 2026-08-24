# PLAN EL ASISTENTE -- un ayudante que corre DENTRO de BMO-X

> Escrito el 2026-08-23, el dia que entraron AVX2 y el monton grande.
>
> **Este documento fusiona cuatro que ya existian** y que contestaban trozos de
> la misma pregunta sin saberlo:
>
> | de donde | que aporta |
> |---|---|
> | `docs/maestro/RED_MAESTRO.md` | por que los protocolos son de Ring 3, y el orden de la red |
> | `platform/drivers/gpu/rdna4/src/lib.rs` | la meta A: SDMA, del tamano de AHCI |
> | `platform/drivers/gpu/rdna4/PLAN_VULKAN.md` | la meta B: el 3D y el muro del PSP |
> | `docs/maestro/INTI_MAESTRO.md` | que sabe hacer hoy el lenguaje |
>
> Y existe porque a la pregunta *"cuanto falta"* se contesto **"meses"** sin
> desglosarlo, y el dueno pidio el desglose. Tenia razon: **"meses" era una
> palabra, no una medida** -- y al medirlo salieron tres cosas que la palabra
> escondia. Estan en la seccion 7.

---

# 0. LA CADENA DE PORQUES, de una vez

Un documento de plan que no diga **por que existe el objetivo** es una lista de
tareas. Esta es la cadena entera, y cada eslabon se sostiene solo:

```text
  Eddi vive en el escritorio de BMO-X y NO vuelve al shell de Ring 0
      -> lo que solo es orden del kernel es codigo que el no puede usar
  Un asistente que le guie tiene que ser una APP, no un comando
      -> tiene que correr en Ring 3, con los 2 syscalls congelados
  Una app que responde tiene que leer un modelo y multiplicar matrices
      -> monton grande + AVX2 + ficheros. Las TRES entraron ya
  Y si ademas busca en internet, tiene que hablar TLS
      -> y ahi esta el muro, que NO es la red: es la CRIPTOGRAFIA
```

**El objetivo no es "tener una IA".** Es que el escritorio deje de ser un sitio
donde solo se mira, sin que eso obligue a volver a Ring 0. Un asistente es la
primera app que justifica de verdad todo lo que se construyo debajo.

---

# 1. LO QUE UN ASISTENTE NECESITA, pieza a pieza

No en abstracto: las cosas concretas que un motor de inferencia hace, y contra
que se apoyan en BMO-X.

| # | lo que hace | contra que | estado |
|---|---|---|---|
| 1 | abrir el fichero del modelo | `abre_para_leer`, `lee_bloque` | [si] existe |
| 2 | tenerlo en memoria | el monton de la tarea | [si] **hoy** (`necesita monton`) |
| 3 | parsear la cabecera binaria | `bufer de natural8`, `crudo` | [si] es lo que INTI hace mejor |
| 4 | el vocabulario del tokenizador | `tabla de texto a entero64` | [si] **hoy** |
| 5 | trocear el texto de entrada | `texto`, `lista` | [si] hoy |
| 6 | multiplicar matrices | `funde_de_cuatro` (FMA de 4) | [si] **hoy** |
| 7 | desempaquetar pesos cuantizados | `bits_y`, `desplaza`, `natural8` | [si] existe |
| 8 | `exp` para el softmax | -- | [no] **no hay** |
| 9 | repartir el calculo entre nucleos | `plat/smp/crew.rs` | [!] existe y es **de Ring 0** |
| 10 | escribir en una ventana | DIRECTOR + el buzon de 2c | [si] hoy |
| 11 | buscar en internet | la pila de red | [no] ver seccion 3 |

**Ocho de once en verde, y tres de esos ocho entraron hoy.**

## 1.1 -- Lo que falta de verdad (8 y 9), medido

### `exp` -- y por que NO es "portar libm"

Un softmax necesita `exp` sobre flotantes. INTI no lo tiene y no lo hereda de
nadie: no hay libc.

**Y es la pieza mas barata de la lista.** Un `exp` para inferencia no necesita
ser el de IEEE: necesita ~6 cifras y ser rapido. Eso es
`2^k * polinomio(r)` -- una descomposicion y un Horner de 5 terminos. Cabe en
`runtime/mate/exp.inti` en menos de 80 lineas, con AVX2 si se quiere las cuatro
de golpe.

> **[!] Y aqui hay una trampa que hay que ver antes de caer:** la tentacion es
> traer `musl` o `openlibm` y "ya esta". Eso mete miles de lineas de C con
> `errno`, modos de redondeo y casos denormales -- para usar UNA funcion, y
> encima entra por la puerta que este proyecto cerro a proposito. Escribir la
> que hace falta es **menos trabajo** que portar la que no.

**Coste: dias.** Y es trabajo que se puede empezar hoy.

### El reparto entre nucleos -- [!] EL HALLAZGO QUE CORRIGE LO QUE SE DIJO

Se dijo *"SMP: semanas, AXION apaga pero no enciende"*. **Las dos mitades eran
enganosas**, y `crew.rs` lo dice en su primera linea:

```text
   los APs arrancaron -- 12 de 12 en el Ryzen
   `crew` reparte una funcion pura entre n partes, con barrera al final
```

O sea: **el reparto multinucleo YA FUNCIONA**, y esta probado en metal. Lo que
falta no es SMP.

**Lo que falta es que un programa pueda darle SU trabajo.** Y hay que decirlo
con precision, porque a medias suena a otra cosa:

```text
   lo que Ring 3 YA puede    TASK_OP_SMP_DESPERTAR: arrancar, parar, y MEDIR
   lo que Ring 3 NO puede    decir "reparte ESTA funcion mia entre n partes"
```

`crew::prueba` corre una faena **del kernel**. No hay camino para una funcion de
Ring 3, y `crew.rs` lo dice sin rodeos: *"no hay tareas de Ring 3 corriendo en
otro nucleo"*.

Y eso es exactamente el problema que la memoria del proyecto ya tiene con otro
nombre -- *"el escritorio no tiene salida: lo que solo es orden del kernel es
codigo que Eddi no puede usar"*. Aqui los nucleos estan encendidos, medidos, y
son del kernel.

[!] **Y el paso 0 sigue sin foto.** `smp prueba` contesto `0.00x` en metal el
2026-08-08, y desde entonces lleva tres testigos --`ENTRARON`/`VIERON`/`HECHOS`--
que nadie ha fotografiado. Disenar la puerta sobre un reparto que no se sabe si
funciona seria disenar sobre nada: **esa foto va antes.**

Lo que MWAIT arregla es otra cosa y hay que separarla: hoy un obrero en espera
**gira al 100%** en vez de dormir. Eso es consumo, no capacidad. Para un
asistente que calcula, los obreros no esperan: trabajan.

```text
   lo que hace falta para el asistente   una operacion de reparto en el ABI
   lo que MWAIT arregla                  que once nucleos no giren en vacio
```

**Son dos trabajos distintos y solo el primero bloquea.** Coste del primero:
semanas, y es diseno de contrato, no de silicio.

---

# 2. POR QUE INTI Y NO C? -- la pregunta del dueno, contestada con lo concreto

Se podria escribir el motor en BMO C, que existe y compila. La respuesta es que
si, INTI, y **no por preferencia**: por cuatro cosas que se pueden senalar.

### 2.1 -- El bucle interior es exactamente donde C miente

Un motor de inferencia son tres bucles anidados sobre indices calculados. Es el
sitio donde `a[i]` fuera de rango en C **no falla**: lee memoria de otro y sigue.

En INTI ese mismo bucle tiene la Regla 2 puesta por el compilador, y cuando de
verdad estorba se escribe `crudo` -- que **se cuenta y sale en el manifiesto del
`.bex` con un numero**. En C todo el fichero es `crudo` y no hay numero que
mirar.

> ** Y esto no es teoria de este documento: es la leccion de la sonda de DOOM.
> La muerte de DOOM se localizo en UNA linea, y era una flecha sobre un puntero
> calculado que resolvia a offset CERO. En C eso corrio meses.

### 2.2 -- Los pesos son un formato binario, y ahi INTI tiene ventaja de forma

Leer un GGUF es recorrer bytes con desplazamientos. INTI tiene `bufer de T`
--una direccion, sin longitud, indexable bajo `crudo`-- que es literalmente el
tipo que hace falta, **y tiene `lista de T` al lado para todo lo demas**. La
frontera entre "aqui nadie comprueba" y "aqui si" es una palabra, y se ve al
leer.

### 2.3 -- AVX2 entra por la misma puerta que todo lo demas

`funde_de_cuatro` es una fila de `intrinsics.toml`, pide `crudo`, y se cuenta.
En C seria un intrinsic del compilador con su propio camino. Aqui el sitio donde
se toca el silicio **tiene un numero en el binario**.

### 2.4 -- Y la que de verdad decide: el asistente ES la app insignia

Si la primera aplicacion grande de BMO-X se escribe en C, entonces BMO-X es un
sistema donde lo serio se hace en C y INTI es el lenguaje de los ejemplos. El
lenguaje del sistema se demuestra escribiendo el sistema con el.

> **Lo honesto que hay que decir en contra:** BMO C es mas maduro y tiene mas
> banco de pruebas. Si algo se atasca en INTI, escribir esa pieza en C **no es
> una derrota**: los dos producen `.bex` y conviven. Pero se empieza por INTI.

---

# 3. LA RED -- analizada de verdad, que es lo que se pidio

## 3.1 -- Lo que se dijo mal

Se dijo: *"BMO-X no tiene pila de red en absoluto; son meses"*. **Es falso en
la primera mitad**, y la segunda esconde donde esta el coste.

## 3.2 -- Lo que hay HOY, comprobado

| paso | que es | estado |
|---|---|---|
| 0 | encontrar la NIC y preguntarle quien es | [si] **VERIFICADO EN EL RYZEN** |
| 1 | anillo RX: recibir tramas sin transmitir | [si] escrito, [..] falta la foto en metal |
| 2 | el contrato `KIND_RED` | [no] |
| 3 | transmitir + ARP en Ring 3 | [no] |
| 4 | IP + UDP, y un `ping` que conteste | [no] |

El paso 0 no es papel: `docs/metal/PRUEBA_EN_METAL.md` tiene la lectura del
hardware real --

```text
   red:  MAC                      =2C:F0:5D:D9:3C:E3
   red:  enlace ARRIBA, megabits  =100
```

-- y estaba **predicha antes de mirar**, contra lo que dice el Windows de la
misma maquina. Es el metodo de las cinco sondas del `#GP` de julio.

## 3.3 -- *** DONDE ESTA EL COSTE DE VERDAD, y no es donde parece

Del paso 0 al paso 4 hay **semanas**, no meses: son tramas Ethernet, ARP e IP,
que caben en unos cientos de lineas cada uno y no tienen criptografia dentro.

**El muro es TLS**, y `RED_MAESTRO.md` ya se negaba a prometerlo por escrito:

> *"No va a haber TLS pronto. Sin curva eliptica ni AES no hay HTTPS, y eso esta
> detras de la misma deuda que aplazo la firma Ed25519."*

Para "buscar en internet" hace falta HTTPS, y HTTPS es:

```text
   X25519          intercambio de claves sobre curva eliptica
   AES-GCM         o ChaCha20-Poly1305
   SHA-256         y HKDF encima
   X.509           validar la cadena de certificados -- ASN.1, fechas, CRL
```

Cada una es criptografia de verdad: escribirla mal no falla, **funciona y no
protege**. Y hay una deuda apuntada que apunta al mismo sitio -- `verify_ed25519`
hoy dice que si a una firma de ceros.

> *** **Y de ahi sale una salida que cambia el orden entero.** Ver la seccion 5.

## 3.4 -- El reparto, que ya estaba decidido

```text
   Ring 0                      Ring 3
   ------                      ------
   tramas Ethernet crudas      ARP, IP, TCP, DNS, TLS
   la MAC, el enlace, el DMA   todo lo que tiene versiones
                               y por tanto se equivoca
```

**El kernel no sabe lo que es una IP.** Una pila TCP es la superficie de ataque
mas grande de un sistema conectado, y aqui se puede morir sin llevarse la
maquina. Windows y Linux la tienen dentro del nucleo porque en 1990 no habia
otra forma.

Y eso conecta con el punto 2 de este documento: **la pila de red es otra app de
Ring 3, y se escribe en INTI por los mismos cuatro motivos.**

---

# 4. LA GPU -- reescrito el 2026-08-23 bajo la LEY 24

> **[!] Esta seccion decia "meses" y estaba mal medida.** No por optimismo ni
> por pesimismo: **le puso precio al proyecto equivocado**. Lo dejo escrito el
> dueno el mismo dia: *"si hablas de meses en RDNA4 SOLO UNO para perfilar con
> todo generico ahi estas chocando"*.
>
> Se conserva el error porque el error es la leccion. Ver `BITACORA.md`, ley 24.

## 4.1 -- El choque, dicho entero

`amdgpu` son millones de lineas. Son millones porque **soporta quince anos de
tarjetas**: descubrimiento de bloques en tiempo de ejecucion, decenas de juegos
de firmware, mapas de registros por generacion, gestion de energia para cada una.

**Eso es el precio de ser generico, y BMO-X no lo paga en hardware.** Lo que
aqui se escribe es un **perfil de UNA tarjeta**: un device id, un juego de
blobs, un mapa de registros, una secuencia de arranque. Es lo mismo que
`cpu_vendor/profile.rs` dice de un CPU -- *"estrenar otro CPU es cambiar un
perfil, nunca editar el kernel"*.

Estimar "meses" mirando `amdgpu` es **estimar otro proyecto**.

## 4.2 -- Las seis piezas de B2, releidas por perfil

`PLAN_VULKAN.md` ya las tenia contadas. Lo que faltaba era leerlas con la ley
delante:

| # | pieza | lo generico | **lo que es un PERFIL** |
|---|---|---|---|
| 1 | enumerar PCIe, mapear BAR | el bus es una **especificacion** | -- ya hecho (xHCI, AHCI) |
| 2 | **el PSP** | -- | ⚠ una secuencia, no una tabla. Ver 4.3 |
| 3 | anillos + timbres | -- | **la forma es la de xHCI**, ya peleada en metal |
| 4 | VRAM, GTT, tablas de pagina | `amdgpu` lo hace para 15 anos de aperturas | **UNA apertura, UN formato**: se conoce, se escribe |
| 5 | SPIR-V -> ISA de RDNA | -- | **la ISA esta PUBLICADA**, y una ISA es una TABLA |
| 6 | la API de Vulkan | ** SI es software: generico, y se reutiliza de B1 | -- |

*** **Y la fila 5 es la que mas cambia al mirarla asi.** "Escribir un compilador
de sombreadores" suena a proyecto de anos. Pero `sem-asm` existe y su promesa
esta escrita: *"anadir una instruccion = 1 entrada TOML, CERO Rust"*. Un
`tables/arch/gfx1200/` es **la misma forma** que `tables/arch/x86_64/`, que ya
tiene cuatro ficheros y 72 intrinsecos.

No es que sea facil: es que **no es un proyecto nuevo, es una carpeta nueva en
uno que ya funciona.** Y la fila 6 es software, o sea generica, o sea que se
escribe una vez y no se vuelve a tocar al cambiar de tarjeta.

## 4.3 -- ⚠ EL PSP: lo unico que el perfil NO encoge, y por que

Un perfil recorta **variantes**. El PSP no es una variante: es un **apreton de
manos con un procesador de seguridad**, y tiene los mismos pasos se soporte una
tarjeta o cincuenta.

**Pero de ahi no sale un numero, y ese fue el error de verdad.** `PLAN_VULKAN.md`
lo dejo escrito antes de que nadie estimara nada:

> *"No escribas un plan de fechas sobre esto hasta haberlo mirado. Es exactamente
> el tipo de cosa que parece de dos semanas y son seis meses."*

Y se escribio una fecha igual. **Eso es la ley 11 incumplida** --*a un aparato se
le pregunta, no se le supone*-- y esta citada en el fichero de al lado, dos
lineas mas arriba de donde se rompio.

### Lo que hay que hacer en su lugar, y es barato

```text
   leer la secuencia del PSP de Navi 4x en `amdgpu`, y CONTAR LOS PASOS
   -> un dia de lectura, cero hardware, cero dinero
```

Eso convierte *"no se sabe"* en un numero. Hasta entonces la respuesta honesta
sobre el PSP es **"no esta medido"**, y no es lo mismo que "es largo".

## 4.4 -- Por que AMD, y por que no es una preferencia

Palabras del dueno (2026-08-23): *"tenia RTX 3060 12G y lo use pero ingenieria
inversa, la verdad es historia. Con AMD es el motivo, no me importa el costo,
porque se que se puede y punto."*

Y esa frase tiene el dato tecnico dentro: **con Nvidia el camino era ingenieria
inversa**; con AMD el camino esta **abierto**:

| | |
|---|---|
| firmware | publicado en `linux-firmware` y **redistribuible** |
| ISA de RDNA | **publicada por AMD** |
| driver de referencia | `amdgpu` es abierto y **se puede leer** |

** Con eso, la pregunta deja de ser *"se puede?"* y pasa a ser *"cuanto"*. Y
"cuanto" es lo que la seccion 4.3 dice que hay que medir en vez de suponer.

## 4.5 -- Y donde vive: RING 3

`rdna4/src/lib.rs` ya lo declara: *"como todo driver de BMO, esto corre en Ring
3 como un servidor BEX detras de un estuario de Canal. Ring 0 nunca gana codigo
de GPU."*

** No es un detalle de colocacion. Un driver de GPU es de los mas grandes que
tiene un sistema, y **aqui puede morirse sin llevarse la maquina** -- la misma
decision que puso la pila TCP en Ring 3 (seccion 3.4). El kernel entrega el
aparato y se aparta.

## 4.6 -- Lo que sigue siendo verdad, y no cambia con la ley

Dos cosas de la version anterior sobreviven enteras:

1. **Las dos metas siguen separadas.** SDMA para el compositor (meta A) y el 3D
   con computo (meta B2) son proyectos distintos, y confundirlos es *"la forma
   clasica de no terminar ninguna de las dos"*. La meta A **no toca el display**
   --hereda el framebuffer del UEFI y se salta DCN entero-- y por eso es del
   tamano del driver de AHCI.

2. **Manda la regla 4: PRIMERO EL NUMERO.** `perf` dice KiB por fotograma. La
   respuesta puede ser que la GPU no compre nada para lo que BMO-X hace hoy. Eso
   se mira antes de gastar un sol, y se vuelve a mirar despues.

Y para DOOM la respuesta ya esta medida y no depende de nada de esto: a
1600x1000 el deficit **entero** es el blit, ~300 MB/s al framebuffer. Eso es
literalmente lo que hace SDMA.

---

# 5. *** LA REORGANIZACION -- el orden, y por que este

Lo que sigue no es una lista de deseos ordenada por ganas: cada escalon
**desbloquea al siguiente** o **cobra algo que ya esta pagado**.

## Escalon 1 -- El asistente LOCAL, sin red (semanas)

```text
   1a  `exp` en INTI                      dias      -- lo unico que falta de mates
   1b  el reparto de nucleos en el ABI    semanas   -- `crew` existe, falta la puerta
   1c  el motor de inferencia en INTI     semanas   -- y se puede EMPEZAR HOY
```

**1c no espera a 1a ni a 1b.** Cargar el modelo, tokenizar y hacer la primera
multiplicacion no necesitan ninguno de los dos; los necesita para ir rapido y
para dar la ultima capa. Empezar por el cargador de GGUF es trabajo real desde
esta misma tarde.

Al final de este escalon hay **un asistente que responde sobre tus ficheros, sin
internet, en tu maquina.** Que es lo que se pidio.

## Escalon 2 -- La red que NO necesita criptografia (semanas)

```text
   2a  la foto del anillo RX en el Ryzen   una tarde -- el codigo ya esta
   2b  KIND_RED                            el contrato, escrito con una trama en la mano
   2c  transmitir + ARP en Ring 3
   2d  IP + UDP, y un `ping` que conteste
```

El paso 2d es el que `RED_MAESTRO.md` llama *"lo que el dueno queria"*, y trae
la unica prueba honesta de que el diseno vale: **la latencia de ida y vuelta,
en microsegundos, contra la que da Windows en el mismo cable.**

[!] **Esto NO da "buscar en internet".** Da red que funciona y se puede medir.

## Escalon 3 -- La criptografia, que es la frontera de verdad (meses)

```text
   3a  SHA-256                       y de paso cierra la deuda de `verify_ed25519`
   3b  X25519 + AES-GCM
   3c  TLS 1.3
   3d  X.509 y la cadena de confianza
```

> *** **Y AQUI ESTA LA CONEXION QUE JUSTIFICA EL ORDEN, y que ninguno de los
> cuatro documentos fusionados podia ver solo:**
>
> `verify_ed25519` dice que si a una firma de ceros, y no lo llama nadie
> **todavia**. La firma de los `.bex` la necesita `bmo-verify`, los mods de
> codigo, y el modelo comercial entero -- porque una Base inmutable que no se
> puede verificar no es un argumento de venta.
>
> **La criptografia que hace falta para HTTPS es la MISMA que hace falta para
> que BMO-X pueda firmar lo que ejecuta.** Se creia que eran dos deudas y es
> una. Eso mueve el escalon 3 de *"lo que hace falta para navegar"* a *"lo que
> hace falta para que el sistema sea lo que dice ser"*.

## Escalon 4 -- La GPU (meses, y con un numero delante)

```text
   4a  medir con `perf` si la GPU compra algo    <- ANTES de gastar
   4b  meta A: SDMA para el compositor           <- y DOOM deja de ir a tirones
   4c  meta B2: compute                          <- el muro del PSP
```

---

# 6. LA TABLA QUE CONTESTA "CUANTOS MESES"

| lo que se quiere | cuanto | que lo bloquea de verdad |
|---|---|---|
| un asistente local, sobre tus ficheros | **semanas** | nada de diseno: es trabajo |
| que use los 12 nucleos | +semanas | una operacion de reparto en el ABI, y la foto de `smp prueba` |
| red que funciona y se mide | **semanas** | nada: el paso 0 ya esta en metal |
| **buscar en internet** | **meses** | *** la CRIPTOGRAFIA, no la red |
| que DOOM vaya fino | meta A (SDMA) | **no toca el display**: del tamano de AHCI |
| Vulkan en Ring 3 (meta B2) | ⚠ **NO MEDIDO** | el PSP -- ver 4.3, y **es un dia de lectura** |

*** **La ultima fila NO dice "meses", y esa es la correccion.** Decia el
proyecto mas grande del repo, y eso le ponia precio a `amdgpu` --generico,
quince anos de tarjetas-- cuando lo que se escribe aqui es un perfil de UNA
(ley 24). De las seis piezas de B2, **cuatro ya estan hechas, son tablas, o son
software que se reutiliza**; la que no se sabe es el PSP, y no saberlo no es lo
mismo que saber que es largo.

**"Meses" queda en UNA sola casilla**: la criptografia. Y esa es la unica del
cuadro que es un invento y no trabajo.

---

# 7. LAS CUATRO COSAS QUE "MESES" ESCONDIA

Se escriben aparte porque son el motivo entero de que este documento exista, y
porque las cuatro corrigen algo que se habia dicho mal:

1. **La red no esta a cero.** El paso 0 esta verificado en el Ryzen, con la MAC
   predicha antes de mirarla, y el anillo RX esta escrito. Lo que falta del lado
   de la red son semanas.

2. **Los nucleos ya se reparten trabajo.** `crew.rs` corre 12 de 12 en metal. Lo
   que falta no es SMP: es una puerta a Ring 3. Y lo que MWAIT arregla --que once
   nucleos no giren en vacio-- es consumo, no capacidad.

3. *** **Lo que de verdad separa a BMO-X de internet es la criptografia, y esa
   deuda ya estaba apuntada en otro sitio con otro nombre.** Es la misma que
   impide firmar un `.bex`. Pagarla una vez cobra dos.

4. *** **Y la cuarta la caza el dueno, sobre esta misma pagina (2026-08-23):**
   el "meses" de la GPU le ponia precio a `amdgpu`, que es generico, cuando lo
   que aqui se escribe es **un perfil de una tarjeta**. Es la ley 24, y no
   estaba escrita -- su evidencia llevaba repartida en cuatro sitios del repo
   sin que ninguno la nombrara.

   ** La correccion util no es "es menos de lo que dijiste": es que **el PSP no
   esta MEDIDO**, y convertir un desconocido en un numero era incumplir la ley
   11 con la ley 11 citada dos lineas mas arriba.

---

# El resumen en una frase

> **Para que el asistente exista no falta ningun invento: falta trabajo. Lo que
> falta de invento es la criptografia -- y esa ya se debia por otro lado.**
