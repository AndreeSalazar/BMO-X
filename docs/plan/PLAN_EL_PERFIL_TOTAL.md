# PLAN EL PERFIL TOTAL -- todo lo que ESTA maquina puede dar sin comprar nada

> Escrito el 2026-08-24, a peticion del dueno: *"me gustaria plan total, TODO lo
> que es perfil a base de mi PC para construir. El plan alcanzable, lo que hay,
> menos la GPU."*
>
> Es el companero de `PLAN_EL_ASISTENTE.md`. Aquel dice **que falta para una
> meta**; este dice **que da el hardware que ya esta encima de la mesa**.

---

# 0. LA LEY QUE ORDENA ESTE DOCUMENTO

`BITACORA.md`, ley 24:

```text
   hardware   ->  PERFIL     se nombra EXACTO; estrenar otro es cambiar
                             una tabla, nunca editar el nucleo
   software   ->  CONTRATO   no nombra a nadie, y por eso vale para todos
```

Todo lo de abajo esta clasificado por esa linea. Y tiene una consecuencia
practica que se ve enseguida: **hay piezas que NO hay que escribir**, porque son
software y ya existen escritas por otro. Confundir los dos lados es como se
acaba escribiendo un driver de e1000 para una maquina que lleva una Realtek.

---

# 1. EL INVENTARIO -- que hay en esta maquina, y en que estado

| aparato | lo que es, EXACTO | estado | donde vive |
|---|---|---|---|
| CPU | **AMD Ryzen 5 5600X** | [si] **PERFILADO**, 952 lineas / 9 ficheros -- ⚠ pero su TOPOLOGIA no es de fiar: ver 5.4 | `cpu_vendor/ryzen_5_5600x/` |
| RAM | 14,8 GiB, y BMO-X ocupa 5,4 MiB | [si] en metal | `mm/` |
| Disco | AHCI/SATA + FAT32 + ESTRATOS | [si] lee **y escribe** en metal | `bmo-ahci`, 1.103 lineas |
| USB | xHCI + HID, teclado y raton propios | [si] en metal | `bmo-xhci`, 1.871 lineas |
| Pantalla | framebuffer lineal del UEFI, 1920x1080 | [si] en metal | heredado del firmware |
| **Red** | **Realtek RTL8111/8168**, MAC `2C:F0:5D:D9:3C:E3` | [si] paso 0 en metal, [..] RX escrito | `bmo-net`, 630 lineas |
| Sonido | **USB Audio Class 1.0**, `VID_1B3F&PID_2008` | ◐ el volumen manda; reproducir pide isocronas | `dev/uaudio.rs` |
| Reloj | RTC + TSC calibrado | [si] en metal | `bmo-rtc`, 319 lineas |
| GPU | **ninguna** -- se usa el framebuffer del firmware | fuera de este plan | -- |

** El altavoz de la placa **no suena y no es un fallo**: la placa no trae
zumbador (`aparatos = 1` y silencio, visto el 2026-08-09). El aparato de sonido
de verdad es el audifono USB, y por eso el camino de audio va por USB y no por
PC speaker. Eso es un perfil de esta maquina, escrito.

---

# 2. LO QUE CUESTA UN PERFIL, MEDIDO EN ESTA CASA

No hay que estimar: hay cuatro comparables ya escritos y corriendo en metal.

| pieza | lineas | que hace |
|---|---|---|
| perfil del Ryzen 5 5600X | **952** | identidad, topologia, cache, TSC, MSR, erratas |
| driver AHCI | **1.103** | detectar, enumerar puertos, DMA, leer/escribir sectores |
| driver xHCI | **1.871** | anillos, timbres, enumeracion, transferencias |
| driver de red (solo reconocer) | **630** | encontrar la NIC, MAC, PHY |
| tablas de `arch/x86_64/` | **1.363** de TOML | la maquina entera como DATOS |

*** **Ese es el tamano de un perfil en BMO-X: entre 600 y 1.900 lineas.** No es
una opinion, son cuatro medidas. Y sirve para calibrar cualquier cosa que venga
despues -- incluida la que este documento deja fuera.

---

# 3. LA RED -- EL PLAN TOTAL

Lo que sigue es el desglose que se pidio. Cada paso lleva **quien lo escribe**
y **de que lado de la ley 24 cae**, porque eso decide si hay que escribirlo o
traerlo.

## 3.0 -- ⚠ LA CORRECCION QUE ESTE PLAN TRAE

`docs/identidad/QUE_DESBLOQUEA.md` decia, en su fila 1 de red:

> *"Cablear el e1000: anillos TX/RX sobre las 287 lineas de esqueleto"*

**Eso ya no existe y estaba mal.** El e1000 era la NIC **de QEMU**; esta maquina
lleva una Realtek. Las 287 lineas se borraron y en su sitio hay un perfil del
aparato de verdad. Es la ley 24 en su forma mas cara: *un driver generico es un
driver para un ordenador que no tienes.*

## 3.1 -- Los pasos, con su lado

| paso | que | lado | quien lo escribe | tamano |
|---|---|---|---|---|
| **0** | encontrar la NIC, MAC, PHY | **hardware** | perfil propio | [si] **HECHO Y EN METAL** |
| **1** | anillo **RX**: recibir tramas | **hardware** | perfil propio | [..] escrito, falta la foto |
| **2** | el contrato `KIND_RED` | contrato | propio | dias |
| **3** | anillo **TX**: transmitir | **hardware** | perfil propio | ~300 lineas |
| **4** | ARP, IPv4, ICMP, UDP, TCP | ** **SOFTWARE** | ** **NO SE ESCRIBE** | ver 3.3 |
| **5** | `KIND_SOCKET` y sus operaciones | contrato | propio | dias |
| **6** | DNS | software | propio o traido | dias |
| **7** | **TLS** | software, pero **criptografia** | ver 3.4 | ★ proyecto |

## 3.2 -- El paso 1, que es el mas barato de la lista y no lo parece

**Un cable enchufado ya lleva trafico.** ARP, mDNS, el broadcast del router.
Montando **solo el anillo RX**, BMO-X imprime bytes que mando otro ordenador --
sin IP, sin ARP, sin TCP, y **sin transmitir**.

Y como no se transmite, **un error no puede molestar a nadie mas de la red**.
Es el hito que casi nadie aprovecha y aqui sale gratis.

El codigo ya esta. Lo que falta es un arranque y una foto:

```text
   net rx        -> "receptor ARMADO, anillo en la fisica =0x..."
   (esperar unos segundos)
   net rx        -> "red: trama de 2CF0..." tipo 0806 (ARP) u 0800 (IPv4)
```

[!] **Cero en la primera vuelta es lo esperado**, no un fallo.

## 3.3 -- *** EL PASO 4 NO SE ESCRIBE, Y ESA ES LA LEY 24 PAGANDO

ARP, IPv4, ICMP, UDP y TCP son **software**. No nombran ningun aparato: son
formatos de paquete y maquinas de estados, iguales en toda maquina del mundo.

> ** Y por eso **traer `smoltcp` es la decision CORRECTA**, mientras que traer
> un driver de NIC generico era la incorrecta. Es exactamente la misma ley
> contestando distinto a los dos lados:
>
> ```text
>    driver de NIC generico   -> hardware -> MAL: codigo para un PC que no tienes
>    pila TCP generica        -> software -> BIEN: no nombra ningun aparato
> ```

`smoltcp` es una crate `no_std`, sin asignador, escrita para sistemas
empotrados. Encaja en Ring 3 detras de `KIND_RED` sin tocar el kernel. **Es la
pieza mas grande de la red y es la unica que no hay que escribir.**

[!] Y la alternativa honesta, por si se prefiere: escribirla. ARP son ~150
lineas, IPv4 ~250, UDP ~120, ICMP ~100. TCP es otra cosa -- ventanas,
retransmision, congestion -- y es donde `smoltcp` compra de verdad.

## 3.4 -- Y el muro, dicho con su nombre

Para HTTPS hace falta TLS, y TLS es criptografia de verdad:

```text
   X25519      intercambio de claves sobre curva eliptica
   AES-GCM     o ChaCha20-Poly1305
   SHA-256     y HKDF encima
   X.509       validar la cadena -- ASN.1, fechas, revocacion
```

Escribirla mal no falla: **funciona y no protege**.

*** **Y es la MISMA deuda que impide firmar un `.bex`.** `verify_ed25519` dice
hoy que si a una firma de ceros. La curva eliptica que pide HTTPS es la que pide
la firma. **Se creian dos deudas y es una: pagarla una vez cobra dos.**

## 3.5 -- Lo que la red da SIN TLS, y no es poco

Con los pasos 0 a 6 hay **red de area local que funciona y se mide**:

- `ping` que contesta, y **la latencia contra Windows en el mismo cable**
- transferir ficheros entre esta maquina y otra de la casa
- terminales de banca hablando con un servidor local -- que es el caso de uso
  declarado del proyecto

** El paso 4 de `RED_MAESTRO.md` lo llama *"lo que el dueno queria"*, y trae la
unica prueba honesta de que el diseno vale: **microsegundos de ida y vuelta,
contra los que da Windows en la misma maquina y el mismo cable.**

---

# 4. LOS OTROS PERFILES QUE ESTA MAQUINA ADMITE

## 4.1 -- ⚠ Sonido de verdad: transferencias isocronas -- **SIGUE PENDIENTE (25-08)**

> Se relee el 2026-08-25 y **no ha cambiado nada**: `bmo-xhci` sabe control e
> interrupt, y reproducir sigue pidiendo **isocronas**. Es el escalon 6 y es lo
> unico de la escalera que no lo bloquea otra cosa: se puede hacer hoy.
>
> ★ Y lo que compra no es solo oir. Las isocronas son transferencias **con
> presupuesto de tiempo**, o sea la primera vez que este sistema tiene que
> cumplir un PLAZO. Eso ejercita el planificador de una forma que nada mas lo
> hace, y ese ejercicio es lo que hace falta antes de que llegue el video.

Hoy el volumen del audifono se manda por control transfer. **Reproducir pide
isocronas**, que `bmo-xhci` no tiene: sabe control e interrupt.

```text
   lado hardware   el tipo de transferencia isocrona en xHCI   ~400 lineas
   lado contrato   KIND_AUDIO ya existe y ya reparte           hecho
```

** Y no es solo "poder oir": las isocronas son transferencias **con presupuesto
de tiempo**, o sea la primera vez que este sistema tiene que cumplir un plazo.
Eso ejercita el planificador de una forma que nada mas lo hace hoy.

## 4.2 -- La foto de `smp prueba`

`smp prueba` contesto `0.00x` en metal el 2026-08-08 y lleva desde entonces tres
testigos --`ENTRARON` / `VIERON` / `HECHOS`-- **que nadie ha fotografiado**.

Cuesta un arranque. Y hasta que se haga, cualquier plan que reparta trabajo
entre nucleos esta **construido sobre un numero que no se sabe si es cierto**.

## 4.3 -- Los nucleos, abiertos a Ring 3

`crew.rs` reparte trabajo y corre 12 de 12. Ring 3 puede arrancarlos, pararlos y
medirlos -- **pero no darles trabajo suyo.**

Falta una operacion en el ABI. Es diseno de contrato, no de silicio, y **va
despues de 4.2**: disenar la puerta sobre un reparto sin foto seria disenar
sobre nada.

## 4.4 -- MWAIT

Hoy un obrero en espera **gira al 100%**. Once nucleos girando en vacio es
consumo, no falta de capacidad. Se mira `CPUID.01H:ECX[3]` primero, porque hay
firmwares que lo deshabilitan **y un sistema que cree dormir y esta girando
miente sobre su consumo.**

Es del perfil del CPU, o sea del lado hardware, o sea de esta maquina.

## 4.5 -- Lo que pide la banca, y que ya estaba contado

De `QUE_DESBLOQUEA.md`, y sigue vigente:

| hueco | piezas que faltan | sirve al banco? |
|---|---|---|
| las 3 ops de `KIND_ARCHIVO` | 3 pequenas | ★★★ |
| el enlazador | cerrado el 07-08 | ★★★ |
| ESTRATOS escribir | ya guarda desde Ring 3 (18-08) | ★★★ |

** *"No hace falta elegir entre madurar el sistema y llegar al banco: durante
los tres primeros huecos son el mismo trabajo."*

---

# 5. EL TECHO -- donde se acaba lo que esta maquina puede dar

> **Reescrito el 2026-08-24 por la tarde**, despues de 39 commits. Tres escalones
> cayeron, aparecio uno que no estaba, y **el primero dejo de ser uno para ser
> tres cosas que caben en el MISMO arranque.**

## 5.0 -- *** UN ARRANQUE, TRES FOTOS

Esto es lo que mas desbloquea por lo que menos cuesta, y ya no es una foto:

```text
   net           el censo. No toca nada
   net rx        arma el receptor -- y ahora la foto trae DE, PARA, tipo y largo
   placa         el censo del firmware: que tablas, ECAM, IOMMU
   smp prueba    los tres testigos que llevan desde el 08-08 sin fotografiar
```

** Las cuatro son de LECTURA. `net rx` es la unica que configura algo, y lo que
configura no transmite: un error no puede molestar a nadie mas de la red.

*** Y las tres respuestas estan **predichas por escrito** antes de arrancar, en
`docs/metal/PRUEBA_RED_PASO_1.md`. Un resultado que no se predijo no distingue
"funciono" de "salio algo".

## 5.1 -- La escalera, con lo que cayo hoy

```text
   [X] `exp` en INTI            HECHO   y de camino salio que `-1.0` valia -4,0
   [X] el monton grande         HECHO   `necesita monton 64 megas "por que"`
   [X] AVX2 / `tabla`           HECHO
   [X] 1. UN ARRANQUE, tres fotos       HECHO 24/25-08. Ver 5.0 y las dos hojas
   ---------------------------------------------------------------------
   2. la puerta a los nucleos       semanas       *** LO UNICO que separa el
                                                  motor de inferencia de existir
                                                  [!] Y VOLVIO A BLOQUEARSE.
                                                  Ver 5.4
   3. el asistente local            semanas       la app insignia
   4. KIND_RED + TX                 semanas       la primera trama que SALE
   5. smoltcp en Ring 3             semanas       ping, y la medida contra Windows
   6. isocronas en xHCI             semanas       *** EL AUDIO. Hoy BMO-X
                                                  CONTROLA EL VOLUMEN Y NO
                                                  PUEDE EMITIR UNA MUESTRA.
                                                  ~400 lineas en `bmo-xhci`,
                                                  y trae los PLAZOS
   7. la IOMMU                      ?             *** NUEVO. Ver 5.2
   ---------------------------------------------------------------------
   8. criptografia                  ** YA NO SON MESES: 6 de 9 piezas puestas
                                    el 24-08. Ver 5.5. El techo BAJO
```

## 5.1.1 -- El escalon 1, cerrado: las cuatro fotos salieron

```text
   net / red rx   [X]  16 tramas, 7.967 bytes, 0 perdidas. IPv4 16
   placa          [X]  el censo del firmware: tablas, ECAM, IOMMU tipo 0x10
   smp prueba     [X]  0,99x dormidos -> 11,59x con `smp all`
```

** Y el 11,59x trajo su propia correccion, que esta anotada en la hoja del
24-08: **es cierto Y no se puede extrapolar.** La faena del banco esta ligada a
LATENCIA, y ahi los dos hilos SMT se rellenan los huecos; el motor de inferencia
esta ligado a THROUGHPUT (matmul con AVX2, unidades saturadas) y ahi el techo
sigue siendo **~6x**. Medir el reparto con un producto de matrices es lo que
dira el numero que importa, y **es una faena distinta de la que corre hoy**.

*** **De las once piezas del motor de inferencia quedan DIEZ hechas.** La que
falta es el escalon 2, y no es un invento: es una operacion en el ABI. `crew`
reparte trabajo y corre 12 de 12 en metal; lo que no hay es camino para una
funcion de Ring 3.

[!] Y el escalon 2 va **detras de la foto de `smp prueba`**. Disenar la puerta
sobre un reparto que contesto `0.00x` y nadie ha vuelto a mirar seria disenar
sobre nada.

## 5.2 -- *** EL ESCALON QUE APARECIO HOY: la IOMMU

No estaba en ninguna escalera, y ahora tiene numero porque **se puede leer**:

```text
   [placa] IOMMU tipo 0x10  registros en 0x...
           la NIC trae N caps extendidas (offset >= 0x100)
           0xD  ACS -- impide que dos funciones se salten la IOMMU
```

** Y lo que abre no es rendimiento: es el agujero que hoy tiene el modelo de
seguridad, y es la frase de portada del sistema puesta a prueba.

> **Una capability dice que puede hacer un PROCESO. No dice NADA de lo que puede
> hacer un APARATO.**

Un aparato con bus-master escribe donde le den la direccion -- sin pasar por el
kernel, sin pasar por las tablas de pagina, y sin que nadie se entere. Es la
mina del PRDT de AHCI, y **sigue armada**.

*** Y ACS es la mitad que casi nunca se cuenta: **sin ella, dos funciones detras
del mismo puente pueden hacer DMA la una contra la otra sin que la IOMMU se
entere.** Encenderla sin mirar ACS es poner una puerta en una habitacion que
tiene otra puerta -- y ese dato sale del comando `placa`, gratis, antes de
escribir una linea de driver.

[!] El escalon lleva `?` y no un numero **a proposito**: encender un IOMMU es
programar tablas de pagina para aparatos, y eso no se estima sin haberlo mirado.
Es la ley 11 -- se pregunta, no se supone -- y es la misma leccion que costo el
"meses" de la GPU.

## 5.4 -- ⚠⚠ EL ESCALON 2, BLOQUEADO OTRA VEZ: la topologia dio 27/54

El 2026-08-25, en el mismo arranque que cerro la red, el panel contesto:

```text
   nucleos    27 fisicos
   hilos      54 logicos
   en pie      1 de 54
```

**Este CPU es 6/12.** No hay ningun Ryzen 5 5600X de 27 nucleos.

### De donde sale el numero, y es UN solo sitio

`cpu_vendor/ryzen_5_5600x/topology.rs`, y no hay segunda fuente:

```rust
   let total_threads = core_count_u32() as u32;   // CPUID.1:EBX[23:16]
   let total_cores   = total_threads / 2;         //  <-- NO SE MIDE
```

Se lee **una vez** al arrancar, se cachea en `CPU_TOPOLOGY`, y de ahi salen
`INFO_CPU_NUCLEOS` e `INFO_CPU_HILOS` para todo el sistema. **Ni un limite, ni
un careo, ni una segunda opinion.**

### *** LA COMPROBACION QUE EXISTE NO PUEDE FALLAR NUNCA

`plat/smp/mod.rs` tiene una funcion `hermanos()` que valida la topologia asi:

```rust
   if hilos != nucleos * 2 { ... }    // "SMT esta encendido"
```

Con 54 y 27 esa condicion **pasa**. Y pasa siempre, con cualquier basura, porque
`nucleos` esta DEFINIDO como `hilos / 2` doce ficheros mas atras:

```text
   nucleos := hilos / 2        y luego se comprueba que    hilos == nucleos * 2
```

> **Un testigo que solo se puede confirmar a si mismo no es un testigo.**
> La comprobacion no fallo: es que no puede.

Es la misma clase de fallo que la tabla de seguridad del 25-08 y que C5 de
`PLAN_SEGURIDAD.md` -- **un limite comparado contra el numero equivocado**, tres
veces en dos dias. Ya no es una casualidad: es un patron de este arbol.

### ★ Lo que SI esta bien hecho, y es lo que salva el arranque

`plat/smp` **ya carea contra un segundo testigo**: la MADT de ACPI, que es la
lista de nucleos que declara la placa. Y cuando no coinciden, grita:

```text
   "el firmware declara otros hilos que el silicio (BIOS?)"
```

O sea que **el bring-up NO despierta 54 fantasmas**: despierta los que declara
la MADT, y el `hilos-1` solo se usa cuando no hay MADT. **Lo que esta roto es lo
que se ENSENA, no lo que se hace.**

*** Y de ahi sale la casilla, que es de una linea de diseno y no de codigo:

> **El careo existe y vive dentro de `smp`. Los paneles no lo consultan: leen el
> perfil crudo.** El segundo testigo ya esta en la casa y no se le pregunta.

### [X] Las tres casillas -- **HECHAS el 2026-08-25**

```
   [X] a  el careo corre EN EL ARRANQUE y llega al ESCRITORIO   (2026-08-25)
   [X] b  `total_cores` se MIDE: CPUID.0B.0:EBX                  (2026-08-25)
   [X] c  el perfil declara (6,12) y DESMIENTE al silicio        (2026-08-25)
```

**a** -- El careo existia y vivia dentro de `smp::despertar()`, a la que **solo
se llega tecleando `smp`**. Por eso no dijo nada el 25-08. Ahora corre desde
`phase::main`, sin que nadie lo pida, y sube al escritorio por dos campos
nuevos del ABI: `INFO_CPU_TOPOLOGIA_DUDA` (0x4D) y `INFO_CPU_HILOS_POR_NUCLEO`
(0x4E).

> Una comprobacion que hay que invocar no protege del caso en el que nadie la
> invoca -- y ese es justo el caso en el que hace falta.

**b** -- *** **El segundo testigo ya estaba dentro de la funcion y se tiraba al
suelo.** `detect_bsp` leia la hoja 0x0B **dos veces** y descartaba las dos
respuestas (`_smt_count`, `_core_count`) para luego coger la hoja heredada y
dividir entre dos.

[!] Y las dos lineas descartadas **ademas estaban mal**: en la hoja 0x0B el
conteo vive en `EBX[15:0]`, y `ECX[15:8]` es el **tipo de nivel**. Leian el tipo
creyendo que leian una cuenta. Que estuvieran descartadas es lo unico que
impidio que se notara -- **un dato que no se usa no se comprueba nunca**. Del
mismo tiron: el x2APIC ID se cogia de `EAX` (que es el desplazamiento) en vez de
`EDX`. Se pudo arreglar sin riesgo porque `bsp`, `linear()` y `cpu_count` **no
los lee nadie**.

**c** -- El perfil declara `topologia_esperada: Some((6, 12))` y **no corrige:
grita**. Corregirlo dejaria un sistema que ensena el numero bueno y esconde que
su fuente esta rota, que es exactamente como se llego hasta aqui. Es la doctrina
que `xsave_componentes` ya tenia escrita: se hardcodean los CONTRATOS, se le
preguntan los HECHOS al silicio.

### ⚠ Lo que esto NO arregla, y hay que decirlo

**No se sabe por que el 25-08 dio 54 y el 24-08 dio 12.** Este trabajo no lo
averigua: lo hace **visible la proxima vez**. Si vuelve a pasar, el escritorio
dira cual de los cuatro testigos se salio de la fila, y entonces habra por donde
empezar.

★ Y el 12 del 24-08 sigue siendo lo que era: **un acierto que no se podia
demostrar.** Ahora se podria.

### [!] Y las tres casillas destaparon un limite del guardian de casillas

Las tres se escribieron primero citando su fichero entre comillas, como manda la
regla, y el guardian las denuncio: *"nombran algo que ya existe"*. Con razon
segun su regla, y **equivocandose**, porque las tres **modifican** un fichero que
tiene que existir de antes.

```text
   casilla que hace APARECER un fichero   -> el guardian la sabe comprobar
   casilla que CAMBIA un fichero          -> no puede: la citada siempre existe
```

Se resolvieron con la otra senal que el guardian acepta --**la fecha de la
medida**-- y eso es correcto: dice contra que arranque se comprueban. Pero la
mitad de las casillas de este arbol son del segundo tipo, asi que **el guardian
mide bien las que crean y no sabe mirar las que arreglan.** Queda escrito aqui y
no en su codigo porque **cambiarlo es una decision, no un arreglo**: hoy el
guardian no tiene ni un falso positivo, y eso es lo que lo mantiene encendido.

[!] **La `c` es la ley 24 cobrando lo que promete.** Un driver generico no puede
saber que 54 esta mal --cualquier numero es plausible--; **un PERFIL si**, porque
dice de que chip es. Un perfil que no se atreve a desmentir al silicio es un
driver generico con el nombre puesto.

⚠ **Y hasta que `a` este, este documento no puede decir si la causa es el
silicio, el firmware o el kernel.** El 24-08 el mismo codigo dio 12. Lo unico
honesto que se puede escribir hoy es que **el numero no es reproducible**, y que
nadie lo va a saber mientras solo lo mire un testigo.

*** Por eso el escalon 2 vuelve a estar bloqueado: repartir trabajo entre
nucleos empieza por saber cuantos hay, y hoy el sistema **no lo sabe y no lo
sabia tampoco cuando dijo 12** -- acerto sin poder demostrarlo.

---

## 5.5 -- El escalon 8 bajo de altura: 6 de 9

El techo se llamaba criptografia y se estimo en MESES. El 24-08 entro
`platform/shared/bmo-cripto`, **2.605 lineas, cada pieza contra sus vectores
oficiales**:

```text
   SHA-256   [X]     HMAC  [X]     HKDF  [X]
   X25519    [X]     AES-GCM [X]   el AZAR (RDRAND) [X]
   ----------------------------------------------------
   Ed25519   --      TLS 1.3 --    X.509 --
```

** Y la que mas compra por lo que queda es **Ed25519**: `campo25519.rs` --la
aritmetica modular sobre `2^255-19`, que es la parte que asusta-- **ya esta
escrita y probada** para X25519. A la firma le falta SHA-512 y la curva de
Edwards encima de un campo que ya existe.

*** **Eso mueve la frase de portada de este documento.** Seguia diciendo que el
techo es la criptografia y que es *"el unico escalon que es un invento"*. Sigue
siendo el unico invento; **ya no esta entero por inventar.** Y cobra dos veces,
como estaba escrito: la misma curva que pide HTTPS es la que pide firmar el
`.bex`.

---

## 5.3 -- Lo que sigue sin cambiar

*** **El techo tiene nombre y es la criptografia.** Todo lo de arriba es trabajo
sobre lo que ya existe; el 8 es el unico que es un **invento**, y es el que abre
dos puertas a la vez -- internet y la firma del `.bex`.

Y cuando se llegue al 3, **esta maquina habra dado casi todo lo que tiene sin
comprar nada.**

---

# 6. Y CUANDO SE LLEGUE AL TECHO: la RTX 3060 12G

> Idea del dueno: *"si llegamos hasta el limite de la superficie del sistema
> podriamos aprovechar el tiempo que queda en ingenieria inversa con la RTX 3060
> 12G hasta que la RDNA4 aparezca."*

Merece una respuesta con las dos caras, porque hay una parte que **no sirve** y
una que sirve **mas de lo que parece**.

## 6.1 -- [!] Lo que NO transfiere, y es casi todo

La RTX 3060 es **Ampere**. La RX 9060 XT es **RDNA 4**. Entre las dos no
comparten:

| | |
|---|---|
| la ISA | SASS de Nvidia contra GFX12 de AMD -- **nada en comun** |
| el procesador de comandos | dos disenos distintos, dos protocolos |
| el arranque del firmware | GSP contra PSP |
| el modelo de memoria | aperturas, GTT y tablas de pagina distintas |

** Y esto **es la ley 24 aplicada a si misma**: un perfil es de UNA tarjeta. Un
perfil de Ampere no acerca un perfil de Navi 44 -- **por definicion**, porque si
lo acercara no seria un perfil, seria un driver generico.

Asi que como preparacion para la AMD, la respuesta honesta es: **no sirve.**

## 6.2 -- *** PERO SIRVE PARA OTRA COSA, Y ES LA QUE FALTA

Hay una afirmacion de este proyecto que **hoy no se puede comprobar**:

> *"El bus es generico y el aparato se perfila."*

Es la mitad fina de la ley 24, esta escrita, y **BMO-X nunca ha visto dos
aparatos distintos del mismo tipo**. Nunca ha tenido ocasion de equivocarse.

Con la 3060 metida en la maquina se puede hacer el experimento **sin escribir un
driver**:

```text
   1. `pci` la encuentra, y dice VEN_10DE + su device id
   2. se mapean sus BAR, y se lee algo inofensivo
   3. `rdna4::claims()` contesta FALSE  <- Y ESO ES EL EXITO
```

*** **El paso 3 es la prueba.** Un perfil que se niega a reclamar una tarjeta
que no es la suya es la ley funcionando; un perfil que la reclamara seria un
driver generico disfrazado. Y hasta hoy `pci_devices: &[]` esta vacio **y nadie
lo ha puesto a prueba contra una tarjeta de verdad.**

Ademas separa dos cosas que hoy estan pegadas por no haber tenido nunca
alternativa:

| lo que se prueba | por que importa |
|---|---|
| enumerar PCIe funciona con **cualquier** tarjeta | es una especificacion: tiene que ser generico |
| mapear BAR no depende del fabricante | idem |
| **el reclamo de perfil dice que NO** | es un hecho sobre un chip: tiene que ser especifico |

** Coste: **un arranque y unas lecturas.** Cero escrituras al aparato, o sea
cero riesgo -- el mismo metodo del paso 0 de la red.

## 6.3 -- Y la tercera opcion, que es la que yo elegiria

Si al llegar al techo sobra tiempo, **hay algo que rinde mas que la ingenieria
inversa y no depende de que llegue ninguna tarjeta**: la criptografia.

```text
   ingenieria inversa de Ampere   ->  no transfiere a RDNA4, y desbloquea 0
   SHA-256 + X25519 + AES-GCM     ->  desbloquea internet Y la firma del .bex
```

Es el unico escalon de la seccion 5 que es un invento, es el techo, y **no
necesita hardware que no este.** Cuando la RDNA4 llegue, encontrara un sistema
que ya sabe firmar lo que ejecuta -- que es justo lo que hace falta para cargar
un blob de firmware de AMD **y poder decir que es el suyo**.

*** Y ahi las dos cosas se juntan: **la criptografia que hoy parece del lado de
internet es tambien lo que hara confiable la carga del firmware de la GPU.**

---

# El resumen en una frase

> **Esta maquina tiene ocho escalones de trabajo por delante sin comprar nada, y
> el techo se llama criptografia. La GPU no es lo siguiente: es lo de despues
> del techo -- y el techo, pagado, es tambien la mitad del arranque de la GPU.**
