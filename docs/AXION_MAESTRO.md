# AXION MAESTRO -- el mando de los nucleos, por PERFIL

> Escrito el **2026-08-11**, antes del crate. Pregunta del dueno: *"AXION,
> inspirado en la PS3, para usar todo el CPU pero teniendo CONTROL: apagar o
> encender, inteligente. No tengo la Cell, pero quiero aplicarlo a mi chip a
> base de perfil -- que va en cada nucleo y POR QUE"*.
>
> La idea es buena y este documento la aterriza. Empieza por el dato que la
> reordena entera, porque cambia lo que hay que construir primero.

---

# 0. EL DATO QUE HAY QUE DECIR ANTES DE DISENAR NADA

**Hoy BMO-X no tiene trabajo pesado que repartir.**

No es un problema del kernel ni del reparto. Es que el sistema todavia no hace
nada que sature un nucleo:

| Lo que hace BMO-X hoy | Se puede repartir? |
|---|---|
| Compositor: pintar ventanas | si, por bandas -- pero le sobra tiempo |
| DOOM | **no**: es de 1993 y es de un solo hilo por dentro |
| Disco, FAT32, USB | no: **esperan al aparato**, no calculan |
| Shell, CABINA, el arranque | no: son microsegundos |
| BLAKE3 de las secciones al cargar | **si**, y es el unico real hoy |

O sea que si `smp prueba` diera **6x** manana, el escritorio no iria mas rapido
ni un fotograma. Y eso **no invalida AXION**: lo ordena.

> **AXION es el mecanismo. Lo que falta es la demanda.**

Por eso este documento no reparte los seis nucleos entre trabajos inventados.
Define **el mando** --quien manda, quien obedece, quien duerme y quien lo
dice-- y deja el reparto para cuando exista carga que repartir. Inventar seis
roles fijos hoy seria escribir un e1000: una respuesta guardada a una pregunta
que nadie ha hecho.

---

# 1. QUE SE COPIA DE LA PS3, Y QUE NO

Ya esta decidido en [`SMP_MAESTRO.md`](SMP_MAESTRO.md) y AXION no lo cambia:

| De Cell | Se copia? |
|---|---|
| **Un maestro que orquesta y no compite** | ★ **SI**. Es el corazon de AXION |
| **Trabajo cerrado, en trozos, con barrera** | ★ SI |
| Local store de 256 KB sin coherencia | **NO** |
| DMA explicito para tocar RAM | **NO** |

Los dos que se descartan eran **el precio de una carencia del silicio de 2005**,
no una virtud. Un 5600X tiene 32 MB de L3 **compartida por los seis**: el
transporte que en Cell habia que programar a mano, aqui es la cache. Copiar el
modelo de memoria de Cell seria pagar un precio que este chip no cobra.

**De Cell se copia el reparto, no el transporte.**

---

# 2. EL PERFIL MANDA, Y NO UN NUMERO ESCRITO A MANO

AXION **no puede llevar un `6` dentro**. Lo que sabe del silicio se lo pregunta
al perfil, que ya existe (`ring0/cpu_vendor/profile.rs`) y ya contesta:

```text
   nucleos = 6      hilos = 12      ccx = 1      L3 = 32 MB compartida
```

En un 5950X serian 16 / 32 / 2, y en un CCX doble **la regla de reparto cambia**
--dos grupos que no comparten L3 son dos maquinas pequenas--. Un numero fijo
haria que AXION acertara en esta maquina y mintiera en la siguiente. Es la misma
regla que ya costo dos bugs este mes:

> **Por lo que ES, nunca por donde esta.**

---

# 3. ⚠ SMT: DOCE HILOS NO SON DOCE NUCLEOS

El dato incomodo, y va aqui porque decide la tabla del apartado 4.

Dos hilos del mismo nucleo **comparten L1, L2 y las unidades de ejecucion**.
Para calculo puro, doce obreros no son doce: son **seis con ruido**, y a veces
son *peor* que seis, porque se pisan la cache.

| Tipo de trabajo | Sirve el hermano SMT? |
|---|---|
| Calculo puro (hash, rasterizar, comprimir) | **NO**. Reparte entre 6 |
| Trabajo que **espera memoria** | SI, y ahi si suma |

**El techo honesto de esta maquina para calculo DENSO es ~6x, no 12x.**

## ⚠ Y el metal corrigio esta seccion el 2026-08-11: dio **11,44x**

```text
   ticks con UN nucleo =2212525371
   ticks con todos     =193336026     ->  11,44x con 12 partes
```

La regla de arriba no se rompe: **se precisa**. La faena de `smp test` es una
**cadena de dependencias** --cada vuelta necesita el resultado de la anterior--
asi que cada hilo pasa casi todo el tiempo esperando su propia multiplicacion.
Dos hilos asi **se turnan sin pisarse**, que es exactamente para lo que sirve
SMT.

> **SMT no da el doble de calculo: da el doble de ESPERAS solapadas.**

O sea que 11,44x es **el techo del caso mas favorable que existe**, no lo que va
a dar un programa real. Un trabajo que sature las unidades de ejecucion
--vectorizado, con buen IPC-- se quedara cerca de los seis nucleos fisicos.

Y de ahi sale una regla para el reparto, que es lo que importa aqui: **el numero
de obreros no puede salir de un `6` ni de un `12` fijos**. Sale del tipo de
faena, y el unico que lo sabe es quien la escribe. Por eso `repartir` recibe
cuantos, en vez de decidirlo.

Por eso el hermano SMT no entra en el reparto por defecto: entra **cuando se
pida explicitamente**, y AXION tiene que saber cual es hermano de cual. Eso sale
del APIC ID, no de contar.

---

# 4. EL MANDO: CUATRO ESTADOS, Y CADA NUCLEO CON SU MOTIVO

No son roles fijos por nucleo. Es una **tabla de estados**, y el motivo viaja
con el estado -- porque *"el nucleo 3 esta apagado"* sin el porque es
exactamente lo que hace imposible depurar un sistema al mes siguiente.

| Estado | Que significa | Quien lo pone |
|---|---|---|
| **MAESTRO** | dueno del kernel: drivers, CABINA, scheduler, los 236 `static mut` | fijo, el BSP. **No se negocia** |
| **OBRERO** | acepta faenas cerradas. **Nunca toca un driver** | el mando |
| **DORMIDO** | no gira, no consume, y **puede volver** | el mando |
| **RESERVADO** | existe y se deja en paz a proposito | el mando, con motivo escrito |

```text
   Nucleo 0        MAESTRO    el kernel entero. Nunca cambia.
   Nucleos 1..5    OBREROS    por defecto. Calculo, nada mas.
   Hilos 6..11     DORMIDOS   hermanos SMT: solo si la faena espera memoria.
```

## La regla de oro, que es lo que hace esto viable HOY

> **Un obrero no entra en Ring 0 mas que por su propio syscall, y solo puede
> pedir lo que su capability le conceda.**

Hay **236 `static mut`** en el kernel y son una carrera de datos el dia que dos
nucleos los toquen. Pero *solo si los dos los tocan*. Un obrero que computa
sobre su rango **no toca ni uno** -- y entonces esos 236 no son un bloqueo: son
**la lista de lo que un obrero tiene prohibido**.

★ Y ese contador es la medida de lo que falta para SMP de verdad. **Eran 209 el
08-08 y son 236 el 11-08: sube solo.** El trampolin --lo que la gente llama
"hacer SMP"-- ya esta y arranco 12 de 12 a la primera. Ese 10% esta hecho; el
90% es esta lista.

---

# 5. ★★ EL HUECO REAL DE "ENCENDER Y APAGAR"

Aqui esta lo que hay que construir, y es una sola pieza.

**Apagar funciona. Encender no.**

```rust
   pub fn parar()     // los obreros vuelven a `hlt` y AHI SE QUEDAN
   pub fn reanudar()  // "solo tiene efecto para los que se despierten DESPUES"
```

Un obrero en `hlt` **no sale solo**. Sacarlo pediria una IPI, y para atender una
IPI un AP necesita GS por-CPU y su propia TSS -- que es justo el trabajo que
`obra.rs` evita para poder existir en cien lineas. Asi que hoy "apagar" es un
viaje de ida: para volver hace falta un INIT+SIPI entero.

Y la otra mitad del problema es peor y ya esta medida: **un obrero que espera no
duerme, GIRA**. Con los doce en pie, once nucleos al 100% y la maquina consume
como si trabajara. Eso es lo contrario de "inteligente".

## La salida: `MONITOR` / `MWAIT`, no una IPI

Hay una instruccion para exactamente esto, y evita la TSS y el GS por-CPU:

```text
   MONITOR  <direccion>    "vigila esta direccion de memoria"
   MWAIT                   "duermeme hasta que alguien la escriba"
```

El obrero vigila `RONDA` --la variable que el BSP ya incrementa para publicar
trabajo-- y se duerme. Cuando el maestro publica una faena, **el nucleo despierta
solo**, sin interrupcion, sin TSS y sin tocar el kernel.

| | girar (hoy) | `MWAIT` |
|---|---|---|
| Consumo esperando | **100%** | el de un nucleo dormido |
| Hace falta IPI / TSS / GS por-CPU | no | **no** |
| Puede volver del sueno | si | **si** |
| Latencia para arrancar la faena | minima | unos cientos de ciclos |

⚠ **No se supone: se comprueba.** `MONITOR`/`MWAIT` tienen su bit de CPUID
(`CPUID.01H:ECX[3]`), y hay firmwares que los deshabilitan. AXION pregunta al
arrancar; si el bit no esta, se queda girando **y lo dice**. Un sistema que
cree estar durmiendo y esta girando es un sistema que miente sobre su consumo.

---

# 6. LO QUE AXION TIENE QUE CONFESAR

Igual que CABINA con todo lo demas. Un nucleo que no hace lo que se cree es
indistinguible de uno que si, hasta que se mide:

| Linea | Que delata |
|---|---|
| estado y **motivo** de cada nucleo | *"el 3 esta apagado"* sin porque no se depura |
| APs despiertos / esperados, y **cuales** por APIC ID | un nucleo que no arranca se ve como "va lento" |
| `ENTRARON` / `VIERON` / `HECHOS` del ultimo reparto | parte el fallo en sus tres tramos |
| **aceleracion real de la ultima faena** | si no sube con mas obreros, el reparto no sirve |
| contencion del `SpinLock` | el aviso **temprano** de la carrera, antes de que rompa |
| ciclos girando vs ciclos durmiendo | si "inteligente" es cierto o es una palabra |

★ Las dos ultimas son las que hacen honesto el resto. Sin la de contencion, SMP
se depura a fotos y a suerte.

---

# 7. EL ORDEN, por lo que cuesta terminarlo

### Paso 0 -- LA FOTO QUE FALTA (coste: un arranque)

`smp prueba` **ya existe** y el 2026-08-08 contesto `0.00x`: `repartir` se
rindio esperando. Ya lleva los tres testigos puestos y **nadie los ha
fotografiado**. Antes de escribir una linea de AXION hay que saber si los
obreros entran al bucle, ven la ronda, o mueren en la faena.

**Disenar sobre un reparto que no se sabe si funciona es disenar sobre nada.**

### Paso 1 -- LA TABLA DE ESTADOS, y decirla

Cuatro estados y un motivo por nucleo, en CABINA y en el shell. Es leer, no
mandar: barato, y convierte "los nucleos estan por ahi" en un dato.

### Paso 2 -- `MWAIT`, con su CPUID comprobado

Es la pieza que hace real "apagar y encender". Y de paso mata el consumo del
100% que hoy es el precio de tenerlos en pie.

### Paso 3 -- REPARTO POR PERFIL

Que `repartir` pregunte al perfil cuantos **nucleos** hay --no hilos-- y que el
hermano SMT haya que pedirlo a proposito.

### Paso 4 -- ★ CREAR LA DEMANDA

Y este es el que de verdad justifica todo lo anterior. Candidatos reales, en
orden de honestidad:

| Faena | Por que es buena candidata |
|---|---|
| **BLAKE3 de las secciones al cargar** | ya existe, ya es pesada, y es puro calculo |
| Rasterizar por bandas en el compositor | trabajo real, y se ve |
| Un raycaster por columnas | ya esta escrito (`ray.bex`), se parte solo |
| Comprimir / descomprimir | cuando exista |

**Hasta que exista una de estas, AXION es un mando sin nada que mandar** -- y
decirlo por escrito es lo que evita celebrar un 6x que no mueve un fotograma.

---

# 8. Y EL CRATE, CUANDO TOQUE

`platform/plat/axion` no se crea hoy. La leccion es de esta misma semana y salio
cara: `platform/drivers/net` fueron 287 lineas de un driver para una tarjeta que
esta maquina no tiene, en el workspace y **sin un solo llamante**, durante meses.

> **Un crate huerfano no es trabajo adelantado: es una respuesta guardada a la
> pregunta equivocada.**

AXION nace el dia que el paso 1 tenga codigo que no cabe en `smp/obra.rs`. Hasta
entonces vive aqui, que es donde se decide.

---

---

# 9. LA TERMINAL DEL CPU -- F7 y F8

> Pedido por el dueno el 2026-08-12: *"me gustaria que el CPU tenga como su
> terminal que avise... cuantos consumen y la parte de temperatura y en general,
> lo mismo pasaria con RAM y otros elementos, eso podria usar en F8 y F7... para
> verificar por que o cuales se consumen en tiempo real"*.

## 9.1 -- La sorpresa: casi todo esto son TRES `rdmsr`

Suena a driver de sensores y no lo es. **Ni uno de los cuatro numeros que
importan necesita un driver nuevo**: son registros del propio CPU, leidos con la
instruccion que este kernel ya usa en `cpu_vendor/ryzen_5_5600x/errata.rs`.

| Que | De donde | Cuesta |
|---|---|---|
| **Frecuencia efectiva** | `APERF` (0xE8) / `MPERF` (0xE7) | dos `rdmsr` y una division |
| **Vatios** | RAPL de AMD: `PWR_UNIT` 0xC0010299, `CORE_ENERGY` 0xC001029A, `PKG_ENERGY` 0xC001029B | tres `rdmsr` y una resta |
| **Temperatura** | SMN, por config PCI indirecta del root complex (offset `0x00059800` en Zen 2/3) | dos escrituras y una lectura de config PCI -- lo que `dev/pci.rs` ya hace |
| **RAM** | ya esta | `mem` lo dice desde julio |

★ **Y los dos primeros son CONTADORES, no medidas.** Suben solos y no se pueden
leer "ahora": se leen dos veces y se resta. Eso no es un detalle de
implementacion, es lo que decide la forma del panel -- **una terminal de consumo
es una diferencia entre dos instantes**, y por eso vive en un bucle y no en una
orden.

## 9.2 -- Lo que cada numero contesta, y por que no sirve otro

- **Frecuencia efectiva** = `(APERF/MPERF) x frecuencia base`. **No es la
  nominal**: dice a que va DE VERDAD el nucleo ahora mismo, con su boost y su
  bajada por calor. Es el unico numero que distingue *"esta al 100% de
  ocupacion"* de *"esta al 100% y ademas a 4,6 GHz"*.
- **Vatios** = energia consumida entre dos lecturas, dividida por el tiempo. Es
  **la respuesta a la pregunta de la seccion 5**: un obrero que gira consume, uno
  que duerme no, y hasta ahora eso era una afirmacion sin numero. Con RAPL,
  `smp stop` deja de ser un acto de fe y pasa a ser un antes y un despues.
- **Temperatura** dice si el boost va a durar. Una frecuencia alta con 90 grados
  es una frecuencia que esta a punto de bajar.
- **Ocupacion por nucleo** -- y este es el unico que **no** sale de un registro:
  hay que contarlo en el planificador (ticks con tarea / ticks totales, por
  nucleo). Es el mas util y el mas caro de los cuatro, y conviene no confundirlo
  con los otros tres.

## 9.3 -- Como se reparte, para no repetir el error de CABINA

CABINA ya es *"lo que el kernel ve"* y esta llena. La terminal del CPU **no es
otra CABINA**: es una vista de POCOS numeros que cambian rapido, y esa es una
forma distinta.

```text
   F11   CABINA      HECHOS puntuales, historicos, con su sitio y su intento
   F7    CPU         POCOS numeros, refrescados, del instante
   F8    MEMORIA     idem, para RAM y para el reparto a Ring 3
```

★ **La regla que los separa**: en CABINA se apunta lo que PASO; en F7/F8 se mira
lo que ESTA PASANDO. Un evento no se repinta; una medida sin repintar no vale.

Y por eso el kernel **no debe llevar historial de estas medidas**: da el numero
crudo cuando se lo piden y Ring 3 decide si lo pinta como barra, como grafica o
como nada. Es el mismo reparto que `TASK_OP_INFO` ya usa -- **el kernel da
enteros, las unidades y las barras son de Ring 3** -- y ese contrato ya funciona.

## 9.4 -- El orden, y el primero es de 20 lineas

1. **Frecuencia efectiva del BSP.** Dos `rdmsr`, una fila en `INFO`. Contesta
   *"a que va esta maquina ahora"*, que hoy no se puede saber. **Es el escalon
   mas barato de todo este documento.**
2. **Vatios del paquete.** Tres `rdmsr` mas. Y con eso, la seccion 5 deja de ser
   teoria: se mide `smp stop` con un numero.
3. **F7, la vista.** Ring 3, con lo que ya existe: `TASK_OP_INFO` y una fila mas
   en su tabla de campos. Anadir un dato es una fila.
4. **Temperatura.** Config PCI indirecta. Va la cuarta porque es la unica que
   depende del modelo de CPU y hay que leerla por PERFIL (seccion 2), no a mano.
5. **Ocupacion por nucleo.** Toca el planificador, y **va detras de que los APs
   tengan trabajo**: hoy la ocupacion de once nucleos girando en vacio seria
   100% y no significaria nada.
6. **F8, memoria.** La mitad ya esta medida; lo que falta es refrescarla y partir
   el reparto por dueno.

## 9.5 -- Lo que esta seccion se niega a prometer

- **Cambiar la frecuencia.** Leer es seguro; escribir `P-states` es meterse con
  el silicio del dueno, y este documento ya dijo en la seccion 4 que el mando es
  por PERFIL. Primero medir un mes, despues hablar.
- **Un numero por nucleo sin SMP de verdad.** `APERF`/`MPERF` **se leen en el
  nucleo que se quiere medir**: para los otros once hace falta que ellos mismos
  se lean, o sea trabajo repartido, o sea la seccion 5 antes.
- **Precision de laboratorio.** RAPL de AMD es un ESTIMADOR del propio chip, no
  un vatimetro. Sirve para comparar dos instantes de la misma maquina, que es
  justo para lo que se quiere aqui, y no para publicar cifras.

---

# El resumen en una frase

> **El maestro no compite, el obrero no toca el kernel, el que espera duerme --
> y cada nucleo dice en que estado esta y por que.**
