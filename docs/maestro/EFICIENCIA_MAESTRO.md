# EFICIENCIA MAESTRO -- que todo lo que no hace nada, no gaste

> Escrito el **2026-09-11**, en pareja con [`PLAN_VATIOS.md`](../plan/PLAN_VATIOS.md):
> aquel tiene las casillas; este dice **que copiar del mundo y que seria un
> error copiar**. Palabras del dueno, el mismo dia:
>
> > *"EFICIENCIA MAESTRA, que BMO-X no consuma muchos watts... no es mi enfoque
> > controlar, pero si SIEMPRE ALERTAR en que o por que"*
> >
> > *"no tocar el silicio de que esta hecho, pero mi meta posible es ir MAS
> > ABAJO... TODOS los hardware que consuman poco si no hacen nada, eso es el
> > motivo, y luego optimizar"*
>
> [!] Donde este documento y `PLAN_VATIOS.md` no digan lo mismo, **gana este**:
> se escribio despues de medir tres cosas que el plan suponia (seccion 2.1).

---

# 0. LA REGLA, EN TRES LINEAS -- y el orden no es de gusto

```text
   1. LO QUE NO HACE NADA, NO GASTA        todo aparato, no solo el CPU
   2. LO QUE GASTA, DICE QUIEN Y POR QUE    siempre, y con su numero
   3. Y DESPUES, SOLO DESPUES, OPTIMIZAR    gastar menos HACIENDO lo mismo
```

** La 1 **no es optimizar**. Optimizar es hacer lo mismo con menos; apagar lo
que no hace nada es dejar de tirar. [`OPTIMIZACION_MAESTRO.md`](OPTIMIZACION_MAESTRO.md)
dice que optimizar es LO ULTIMO y que antes hay que decir quien pone el
presupuesto. Dejar de tirar no necesita presupuesto: lo que no hace nada no
tiene cliente al que se le robe nada.

** La 2 es **el juez que le falta a C10**. En la tabla de conformidad de
[`META-KERNEL_HARD.md`](../../FUERO/META-KERNEL_HARD.md) la energia tiene metro
(RAPL) y **"juez: no"**. L0: *un eje sin juez es prosa*. Sin la 2, la 1 se
cumple el dia que se escribe y se incumple en silencio el siguiente.

*** Y la 3 va la ultima porque **no se puede optimizar lo que no se ve**. La 2
es la que dice donde merece la pena.

---

# 1. LA LINEA QUE NO SE CRUZA: el silicio no se toca

El dueno lo puso primero, y la ley ya lo decia. C10, textual: **"BMO-X no pone
voltajes, y no deberia"**. Y R-PWR5: *"un error de ciclos se diagnostica con una
foto y se revierte con un commit; un error de voltaje se diagnostica con un chip
muerto"*.

La cadena real, de la ley:

```text
   VRM de la placa  ->  entrega la corriente y fija el voltaje fisico
   SMU del chip     ->  lo negocia segun carga, grados y limites
   firmware         ->  programa los limites (PPT, TDC, EDC) al arrancar
   el SO            ->  PIDE estados. Nada mas
```

Asi que todo lo que este maestro propone es de UNA sola clase: **pedirle a un
aparato que entre en un estado que el mismo anuncia**. El aparato puede decir
que no. Nada de esta lista cambia de que esta hecho.

| se PIDE (permitido) | se TOCA (prohibido) |
|---|---|
| C-states del CPU: `MWAIT`, el puerto de E/S de C-state | voltajes: VID, `PStateDef` (`C001_0064..006B`, llevan el VID) |
| P-state por `PStateCtl`, si algun dia hiciera falta | limites electricos: PPT, TDC, EDC |
| estado D3hot de una funcion PCI **sin uso** (registro PMCSR, estandar) | curvas de ventilador (son del Super I/O y del firmware) |
| ASPM L1 en un enlace PCIe que lo anuncie | memoria: perfiles, tiempos, voltajes |
| enlace SATA en *partial/slumber* (ALPM) | la BIOS, sus opciones y su tabla AML |
| puerto USB suspendido | overclock y undervolt, en cualquier forma |

[!] Hoy la columna de la derecha se cumple **por casualidad**: revisados los
`wrmsr` del kernel el 11-09 -- EFER, PAT, MTRR, STAR/LSTAR, GS, SPEC_CTRL, TSX y
la base del APIC --, ninguno toca energia. Ningun guardian lo impide todavia.
Eso va en la seccion 6.5.

---

# 2. ★★ LO QUE HAY EN ESTA MAQUINA, pieza a pieza

LEY 24: el hardware se perfila. Inventario tomado el 11-09 desde el Windows del
MISMO equipo (`Win32_*` y `powercfg /a`, solo lectura), cruzado con `PERFIL/`.

## 2.1 El CPU -- tres medidas que corrigen al plan

```text
   [SILICIO]  CPUID 5 = eax 0x40  ebx 0x40  ecx 0x3  edx 0x11
              -> MWAIT solo enumera C0 y C1
   [MEDIDO]   Windows quieto: 83 % del tiempo en C2, 2,5 % en C1, C3 = 0,
              ~24.000 entradas a C2 por segundo (con una app abierta)
   [SILICIO]  CPUID 6 ARAT = 1, TSC invariante = 1
```

*** **La primera cambia el plan entero.** `dormir.rs` elige *"el C-state mas
profundo que enumera el CPUID"*, y en este Ryzen eso es **C1: lo mismo que
`hlt`**. El BSP (W1) y los once obreros duermen en C1. `mwaitx` les dio el
despertar por escritura y el plazo; profundidad, ninguna.

** **La segunda es la INSPIRACION, no la vara.** Windows no es la meta de nada:
es la prueba de que ESTE silicio sabe dormir hondo. Como MWAIT no puede pedir C2 aqui,
el C2 de Windows sale del **puerto de E/S de C-state** que declara ACPI -- en Zen
es el CC6: el nucleo se apaga y guarda su estado. Es una deduccion de dos
medidas, no una suposicion. El puerto lo programa la BIOS en el MSR
`C001_0073` (CStateBaseAddr) [LITERATURA: PPR de AMD], asi que **se lee sin AML**.

** **La tercera quita el miedo.** Con ARAT el temporizador del APIC sigue
contando en C-states profundos: el tick no se pierde y el nucleo vuelve.

Y dos inferencias del plan que no aguantan:

```text
   "4495 MHz en reposo = nadie descansa"   FALSO. Es APERF/MPERF: la frecuencia
                                           MIENTRAS ejecuta. Windows marca 118 %
                                           con un 85 % ocioso
   "el tick de 1 kHz impide dormir hondo"  FALSO. Windows entra en C2 ~2.000
                                           veces por segundo y por hilo, y vive
                                           ahi. El tickless (W2) vale poco
```

## 2.2 Todo el hardware, y que hace cada pieza cuando no hace nada

| pieza | BMO-X la usa | en reposo, HOY | lo que se le puede PEDIR | quien lo mide |
|---|---|---|---|---|
| **CPU** Ryzen 5 5600X | si | C1 en los 12 hilos, tick 1 kHz | C2/CC6 por el puerto de E/S | RAPL [MEDIDO 57,7 / 58,5 W] |
| **GPU** RTX 3060 | **no**, sin driver | lo que la dejo el firmware al arrancar | **nada**: es la pantalla | nadie desde BMO-X |
| **audio de la GPU** (HDA NVIDIA) | no | encendido, sin dueno | D3hot | nadie |
| **audio de la placa** (HDA) | no, sin codigo | encendido, sin dueno | D3hot | nadie |
| **audio USB** (auriculares) | si, el volumen | lo que haga el xHC | suspender el puerto sin sonido | nadie |
| **xHCI** x2 (AMD) | uno | se SONDEA desde el syscall | MSI (W3); el que no lleva teclado, D3hot | nadie |
| **red** Realtek RTL8168 | lee registros | enlace arriba, sin trafico | D3hot sin ninguna tarea de red | nadie |
| **SSD SATA** Kingston A400 x2 | uno | enlace activo siempre | ALPM *partial/slumber* | nadie |
| **NVMe** Kingston NV2 | **NO SE TOCA** | lo que dejo el firmware | **nada**, por ley (2.3) | nadie |
| **RAM** DDR4 2x8 GB | si | refresco normal | nada directo; se autorrefresca cuando el paquete entra en PC6 [LITERATURA] | nadie |
| **enlaces PCIe** | -- | sin leer: nadie mira su ASPM | L1 donde los dos extremos lo anuncien | nadie |
| **placa, VRM, ventiladores** | -- | los gobierna el firmware | nada: bajan solos si baja el calor | nadie |
| **suspension** | -- | la placa solo admite **S3** | ver 2.3 | -- |

*** **La columna que ordena este documento es la ultima.** De trece filas, UNA
tiene medida. Lo que no se ve, no se puede acusar.

** Y la fila de la RAM ensena algo que no esta en ninguna otra: **dormir hondo
el CPU arrastra al resto**. El paquete no baja a su estado mas bajo (PC6) si un
solo nucleo sigue despierto, y la memoria no entra en autorrefresco hasta que el
paquete baja [LITERATURA]. C2/CC6 no es "ahorrar en el CPU": es la llave del
suelo de toda la plataforma.

## 2.3 Lo que no se toca aunque gaste, y por que

```text
   NVMe       PERFIL/DISCO.txt: "EL NVMe NO SE TOCA". Ni para dormirlo. Un
              ahorro de un vatio no justifica la unica regla absoluta del repo
   GPU        es la pantalla. Sin driver no hay estados que pedirle, y D3 en la
              funcion 0 seria apagar la pantalla que el dueno esta mirando
   S3         la placa lo admite, y su numero (SLP_TYP) viene del objeto _S3 de
              la AML: se podria sacar en el BUILD, como se hace con la placa.
              PERO al volver de S3 nadie re-inicializa la 3060 -- sin driver, la
              pantalla vuelve NEGRA. Es un techo, no una palanca
```

[!] **La GPU es probablemente lo mas caro de esta maquina en reposo que BMO-X
no puede bajar.** Una 3060 con su driver se queda del orden de 10-15 W sin hacer
nada [LITERATURA: pruebas publicadas]; sin driver **nadie lo ha medido aqui**.
Se escribe para que el dia que el enchufe marque mas de lo esperado se sepa
donde mirar primero.

---

# 3. ⚠ LO QUE BMO-X NO VE -- y es la mitad de la factura

**RAPL mide el paquete del CPU. Nada mas.** Los 58 W son el CPU, no la
maquina: la GPU, los discos, la placa, los ventiladores y la propia fuente
quedan fuera.

```text
   instrumento           ve                        lo tiene BMO-X    cuesta
   RAPL                  el paquete del CPU        SI                nada
   HWiNFO en Windows     CPU y GPU, con sensores   no (es Windows)   nada: una REFERENCIA
   medidor de enchufe    LA MAQUINA ENTERA         no                se compra
```

** El medidor de enchufe es **el unico instrumento que ve todo**, y es el
unico de la lista que se compra. Se declara y no se exige: la regla de
[`PLAN_EL_PERFIL_TOTAL.md`](../plan/PLAN_EL_PERFIL_TOTAL.md) es *lo que la
maquina da sin comprar nada*.

*** **Y de ahi sale la regla para los aparatos: ESTADOS, NO VATIOS.** Sin
medidor, BMO-X no puede decir cuanto gasta la tarjeta de red. Pero SI puede
decir, con certeza y leyendo un registro, que **esta despierta y nadie la
usa**. Contar lo que esta despierto sin motivo es una medida; ponerle vatios
seria inventar (R-PWR3).

---

# 4. QUE COPIAR DEL MUNDO

## ★ 4.1 Linux, *runtime PM*: un aparato sin usuario se duerme solo

La idea es de dos piezas: **una cuenta de usuarios** y **un plazo**. Cuando la
cuenta llega a cero y pasa el plazo, el aparato va a su estado bajo; el primer
usuario que vuelve lo despierta.

** Se copia la IDEA, y aqui es mas simple que en Linux: BMO-X ya sabe quien usa
cada aparato, porque **usar un aparato es tener su capability**. La cuenta de
usuarios ya existe: es quien tiene el handle.

## ★ 4.2 powertop: la lista de *tunables* y los despertares

Dos cosas, y las dos se copian:

```text
   despertares/s por proceso   la columna que manda. Despertar al CPU es un
                               coste aunque se trabaje poco
   la lista de "tunables"      un renglon por aparato: esta en su estado bajo,
                               o no. "Bad" / "Good", uno por uno
```

*** **La segunda ES la alerta que pidio el dueno para los aparatos**: un
inventario que dice, fila a fila, que esta despierto sin motivo.

## 4.3 Windows: UN numero para toda la plataforma

El informe de reposo de Windows (`sleepstudy`) resume una sesion entera en un porcentaje: el
tiempo en que **toda la plataforma** estuvo en su estado mas bajo (DRIPS). Se
copia la forma: **un solo numero que diga cuanto tiempo estuvo la maquina
entera en su suelo**. Todos los demas numeros explican ese.

## 4.4 macOS: despertar cuesta

"Energy Impact" suma uso de CPU **y despertares**. Se copia que despertar
cuenta como gasto, no solo usar el CPU.

## 4.5 MS-DOS: nadie detras

Ya lo tiene BMO-X por construccion, y esta contado en `PLAN_VATIOS.md` 0.1: cero
demonios. Es la mitad del trabajo, y la barata.

---

# 5. [!] QUE SERIA UN ERROR COPIAR

## Error 1: los perfiles "rendimiento / equilibrado / ahorro"

R-PWR4: en una maquina enchufada **no se sacrifica latencia por vatios** fuera
del ocio. El SMU ya sube y baja la frecuencia solo. Un "modo ahorro" que baja
el reloj mientras el dueno juega le cobra al que trabaja para ahorrar lo que
**deberia ahorrar el que no hace nada**.

## Error 2: el undervolt y el overclock

Ryzen Master, Curve Optimizer, PBO. Todos tocan voltaje o limites: R-PWR5.

## Error 3: la AML para los estados de los aparatos

Linux usa `_PSx`, `_PRx` y `_CST` de ACPI. Son bytecode de terceros en Ring 0:
*tablas estaticas SI, AML NUNCA*. Y no hacen falta: el estado D3 esta en la
capability de gestion de energia de **cualquier** funcion PCI (estandar, igual
en todos los fabricantes), el puerto de C-state esta en un MSR, y el ASPM en un
registro de enlace.

## Error 4: vatios por proceso

powertop los estima con un modelo y los ensena con la misma cara que una
medida. RAPL es de paquete y de nucleo, **no de programa**. BMO-X dice quien
tuvo el CPU y cuantas veces lo desperto; nunca "DOOM gasta 12 W".

## Error 5: el marco generico de energia

`dev_pm_ops`, dominios, callbacks por bus: es lo correcto **para quien arranca
en diez mil placas**. Aqui hay UNA, perfilada. El mismo argumento que
[`IOMMU_MAESTRO.md`](IOMMU_MAESTRO.md), error 1: *una abstraccion de un solo
implementador es una capa de indireccion con un fichero de mas*.

## Error 6: dormir lo que se usa

Suspender el puerto del teclado para ahorrar, o poner el disco de BMO-X a dormir
con una app escribiendo. Eso no es eficiencia: es cobrarle al que trabaja. La
regla 1 dice **lo que no hace nada**, y esas tres palabras son la frontera.

---

# 6. ★★ EL JUEZ: siempre alertar, en que y por que

Esto ya se pidio una vez. [`AXION_MAESTRO.md`](AXION_MAESTRO.md), seccion 9,
el 08-12: *"una terminal que avise... para verificar por que o cuales se
consumen en tiempo real"*. F7 contesto el **cuanto**. Falta el **quien** y el
**por que**.

## 6.1 Las tres preguntas

| pregunta | de donde sale | estado |
|---|---|---|
| **cuanto** | RAPL, milivatios de paquete y nucleo | HECHO, F7 lo ensena |
| **quien** | ciclos de CPU de cada tarea + despertares de cada tarea | **medio**: el planificador ya cuenta `cpu_ciclos` por tarea y solo se lo dice a la propia |
| **por que** | una clase de un vocabulario cerrado (6.2) | falta |

** El "quien" es barato porque ya tiene molde: la memoria ya contesta *"quien
pide RAM"* con `INFO_MEM_QUIEN_PID / BYTES / PETICIONES`, el indice viajando
como `campo | (ranura << 8)`. La CPU no tiene su gemela. **L6c: la simetria
declarada hace visible el hueco.**

## 6.2 El vocabulario, cerrado

Para las tareas:

```text
   TRABAJA      pinta, calcula, juega     legitimo. "Si esta activo, consume"
   GIRA         espera sin dormir         R-PWR1: un calefactor
   DESPIERTA    despierta al CPU de mas   un bucle a 1.000 vueltas/s
   SUPERFICIE   duerme, pero en C1        cuando el silicio da C2
```

Para los aparatos:

```text
   USADO        tiene dueno y trabaja
   OCIOSO       despierto y SIN dueno     <- la alerta
   DORMIDO      en su estado bajo
   INTOCABLE    declarado en 2.3          se ensena, no alerta
```

*** INTOCABLE es lo que hace honesta la lista: el NVMe y la GPU salen **con su
motivo** en vez de desaparecer. *"Un censo que solo lista lo que puede
controlar no es un censo"* ([`NEUTRO/CENSO.txt`](../../NEUTRO/CENSO.txt)).

## 6.3 Cuando salta: SOLO en reposo

```text
   reposo = ninguna tecla ni raton en N segundos
            Y ninguna superficie repintando
            Y ningun hijo trabajando
```

Fuera del reposo **no hay alerta**, porque lo que trabaja consume. Dentro, salta
si:

```text
   alguien GIRA o DESPIERTA de mas
   hay un aparato OCIOSO
   el paquete gasta mas que el SUELO + margen     <- necesita el suelo medido
```

[!] La tercera **no se puede escribir hoy**, y el suelo que pide es **el de
BMO-X, no el de Windows**: lo mas bajo que el propio BMO-X mida con todo
dormido. Hasta que exista, un umbral seria un numero inventado. Las otras dos no
necesitan vatios: cuentan estados.

## 6.4 Donde se ve

```text
   la barra del escritorio   un testigo: verde reposo sano / amarillo algo
                             despierto de mas / rojo alguien gira. Con NOMBRE
   F7                        la tabla del quien, al lado de los vatios
   CABINA y `save`           para las hojas del metal
```

** La barra y no una orden, porque **el dueno vive en el escritorio** y al
shell de Ring 0 no se vuelve. Una alerta que solo sale donde nadie mira no
alerta.

## 6.5 El guardian del build: los giros no crecen

La alerta de ejecucion caza **quien giro**. Hace falta su gemela en el build,
que cace **quien puede girar**:

```text
   cuenta   `spin_loop()` y `pause` en esperas, `bmo_ceder()` dentro de un
            bucle de espera, `hlt` fuera de su dueno, y `wrmsr` a un MSR de
            energia fuera de su dueno
   regla    TRINQUETE: el numero no sube. Bajar se anuncia y se sella
```

** La ultima fila convierte R-PWR5 de casualidad en regla con dientes: el
dia que alguien escriba un `PStateDef`, el build lo dice.

## 6.6 El juez es un crate con pruebas

La decision -- *que clase le toca a esta tarea, esta en reposo la maquina* --
es una funcion pura: numeros entran, veredicto sale. Va a `platform/shared/`
como nieto (L7), con sus pruebas en el anfitrion, igual que `bmo-fisica-juicio`.
Y trae la prueba que hoy falta: **el 5600X solo llega a C1 por MWAIT** (EDX
`0x11`), escrita como caso para que nadie vuelva a creer lo contrario.

## ★ 6.7 YA PUESTO: el letrero `[consumo]` (L6h, R21)

La primera mitad del juez no espera a nadie. Desde el 11-09 cada fichero de Ring
0 dice, debajo de su carril, **que hace cuando la maquina no hace nada**:

```text
   NADA      en reposo no corre por su cuenta
   APAGA     en reposo ES lo que duerme la maquina
   APARATO   enciende o para una pieza de hardware
   LATE      arma un bucle o un reloj propio
```

*** De 180 ficheros, **nueve** no son NADA: el tick, el hilo del bus, la espera
del shell y el bucle del obrero LATEN; el arranque del USB, el AHCI, el audio
isocrono y el encendido de nucleos son APARATO; y `dormir.rs` APAGA. Esa es la
lista de donde mirar, y `contrato.py` la dice en cada build. Tres ficheros
mezclaban una cosa que late con otra que se pide, y se partieron por esa costura
(`META-KERNEL_HARD.md`, L6h).

## ★★ 6.8 LO QUE NO SE VE, NO SE PINTA (idea del dueno, 11-09)

> *"cuando la pantalla que renderiza consume... el juego o programas NO
> RENDERIZA porque eso el usuario no lo nota POR COMPLETO... cuando vuelve,
> sigue ejecutando y vuelve a renderizar en tiempo real"*

Es la regla 1 --lo que no hace nada, no gasta-- aplicada a los pixeles: **un
fotograma que nadie va a mirar no hace nada**, aunque cueste un nucleo
dibujarlo. Y es la ilusion de Santa Monica: lo que no se ve no hace falta que
exista; al volver, el mundo ya esta en su sitio.

```text
   quien decide        el DIRECTOR, con `bmo_golpe::vista` (pruebas en el
                       anfitrion). La app no puede declararse vista
   como lo dice        el byte 2 del estado del buzon: 0 se ve, 1 minimizada,
                       2 fuera de pantalla, 3 pantalla prestada. Cero puertas
   que se para         el dibujo y la secuencia. Logica, sonido y entrada, no
   quien desobedece    no se castiga: se ACUSA por su tid, una vez por racha
   hacia donde falla   hacia "se ve": ahorrar menos antes que congelar
```

** No es una idea exotica: es la que el mundo ya usa, y DOOM la traia puesta.
Los navegadores dejan de dibujar las pestanas de fondo; Windows le dice a un
juego tapado que esta oculto cuando presenta el fotograma; y Chocolate Doom
--del que sale el DOOM de BMO-X-- tiene `screenvisible`: a falso, `D_RunFrame`
se salta `D_Display` entero mientras `TryRunTics` y el sonido siguen. Lo que le
faltaba era que alguien le dijera la verdad.

[!] Lo que NO cubre todavia: una ventana TAPADA por otra. El DIRECTOR aun no
calcula solapes y el juez contesta "se ve" -- ante la duda, se pinta. Cuanto
ahorra lo dice el metal: `consumo` con DOOM a la vista y minimizado.

La regla es R-APP8 de `META-APP_HARD.md`.

---

# 7. EL ORDEN, por lo que desbloquea

La regla de las hojas del metal: **lo que no toca nada va primero; lo que no se
deshace va al final**. Y cada paso trae su antes y su despues (R-PWR2).

```text
   0  MEDIR, sin tocar nada
      W0 de PLAN_VATIOS: `consumo` y `save` en el escritorio quieto. Windows
      con HWiNFO es solo una referencia -- y la unica ventana a la grafica

   1  ARREGLAR W4                                    cuesta: TAREA (Ring 3)
      el reposo del escritorio no entra nunca: `quarter` pone `will_paint`
      cada 250 ms y las 500 vueltas quietas no llegan

   2  EL JUEZ Y EL "QUIEN"                           cuesta: NADA / TAREA
      exponer `cpu_ciclos` por tarea, contar despertares, el crate del juez,
      el testigo de la barra. Los datos ya existen

   3  INVENTARIO DE ESTADOS, solo lectura            cuesta: NADA
      por cada funcion PCI su estado D; por cada enlace su ASPM; los enlaces
      SATA; los puertos del xHC. Leer primero: hoy NADIE lo ha mirado

   4  C2 / CC6 DEL CPU (W6)                          cuesta: MAQUINA si no despierta
      leer `C001_0073` en el metal ANTES de usarlo. Es la palanca grande: la
      que abre PC6 y el autorrefresco de la RAM

   5  DORMIR LO QUE BMO-X NO USA                     cuesta: APARATO, reversible
      D3hot a las funciones sin codigo: el audio de la placa, el de la GPU, el
      xHC que no lleve el teclado. Nunca la funcion 0 de la GPU, nunca el NVMe

   6  LOS ENLACES                                    cuesta: APARATO
      ASPM L1 donde el firmware lo dejo apagado y los dos extremos lo anuncien.
      [!] ALPM en el SSD de BMO-X es el unico paso que dobla R-PWR4: el primer
      acceso tras un reposo llega tarde. Se decide aparte y con su motivo

   7  EL USB INTERRUMPE (W3)                         cuesta: APARATO
      el escritorio deja de sondear y duerme sobre la entrada

   8  OPTIMIZAR                                      cuesta: lo que diga el presupuesto
      gastar menos HACIENDO lo mismo. Con el juez delante, se sabe donde
```

*** El 3 va antes que el 4 aunque gane menos: **no toca nada**, y es el que
convierte trece filas de "nadie" en trece filas con un estado.

---

# 8. LO QUE ESTE MAESTRO NO PROMETE

```text
   un numero de vatios    LEY 24: el suelo lo dice el metal, no este papel
   bajar la grafica       sin driver, la 3060 gasta lo que la dejo el firmware
   tocar el NVMe          ni para dormirlo
   suspender (S3)         la pantalla volveria negra
   bajar del silicio      el suelo lo pone el SILICIO, no un sistema. Windows
                          es la INSPIRACION --prueba que aqui se duerme en C2--
                          y no la vara: sin nada detras, BMO-X puede quedar
                          POR DEBAJO de Windows quieto. Esa es la meta
```

> Un aparato que no hace nada y sigue despierto no es un aparato: es una estufa
> con conector. Y la eficiencia no empieza cuando se optimiza lo que trabaja;
> empieza el dia que **todo lo que no trabaja tiene nombre, estado y motivo** --
> y la maquina lo dice sin que nadie pregunte.
