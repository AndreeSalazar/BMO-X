# SMP MAESTRO -- perfilar el PARALELISMO como se perfila el CPU

> Escrito el **2026-08-06**. Pregunta del dueno: *"SMP perfilar MAESTRO basado
> en Ryzen 5 5600X? Es algo inspirado TOTALMENTE en CELL de PS3 pero MAS alla,
> para poder exprimir -- porque el poder de BMO-X es perfilar en hardware: en la
> entrada es perfilar TOTAL, y en software es generico."*
>
> El principio es correcto y es el mismo que sostiene el BEF y el BSF. Este
> documento lo lleva al paralelismo, y lo primero que hace es **separar la mitad
> de Cell que hay que copiar de la mitad que seria un error copiar**.

---

# ★ 1. QUE FUE CELL DE VERDAD

| | |
|---|---|
| **PPE** | un PowerPC normal. El maestro: orquesta, y computa poco |
| **8 x SPE** | los obreros. 256 KB de **local store** cada uno |
| Acceso a RAM | **ninguno directo**. Solo por **DMA explicito** |
| Coherencia de cache | **no existe** entre SPEs |

El programador movia los datos a mano, a tiempo, y en trozos de 256 KB. De ahi
salieron los dos titulares de la PS3: *potencia bruta* y *un infierno para
programar*.

## ★★ Y aqui esta el detalle que lo decide todo

**El local store no era una idea de diseno: era una respuesta a una carencia.**
Los SPE no tenian cache coherente porque el silicio de 2005 no podia darles una
sin arruinar el presupuesto de transistores. El DMA explicito era **el precio de
esa falta**, no una virtud.

Quien copie el modelo de memoria de Cell en un CPU moderno esta **pagando el
precio de una carencia que no tiene**.

---

# ★★ 2. LO QUE DICE EL SILICIO QUE HAY DEBAJO

No es opinion: son los campos que `s1_cpu::detect_cpu` ya rellena por CPUID en
esta maquina.

| Campo detectado | Valor en el 5600X |
|---|---|
| `cores_per_ccx` | **6** |
| `ccx_count` | **1** -- *monolitico* |
| `threads_per_core` | **2** (SMT) |
| `l3_size_kb` | **32 MB, COMPARTIDA por los seis** |

**Los seis nucleos ya comparten una "local store" de 32 MB, coherente por
hardware, sin una linea de DMA.** Es mas grande que las ocho local stores de
Cell juntas (8 x 256 KB = 2 MB) por un factor de dieciseis, y no hay que
gestionarla.

> **Veredicto**: copiar el modelo de MEMORIA de Cell en este CPU es escribir a
> mano un transporte que el silicio ya te regala, y ademas hacerlo peor.

## ⚠ Y el aviso que hace este documento necesario: NO todos los Ryzen son asi

Un **Ryzen 5 3600X** (Zen 2) tiene **dos CCX de 3 nucleos**, cada uno con su
propia L3 de 16 MB. Dos nucleos de CCX distintos **no comparten cache**: hablar
entre ellos cuesta un viaje por el Infinity Fabric, y ahi si hay una frontera de
tipo NUMA que un planificador deberia respetar.

**El mismo codigo, en dos CPUs de la misma marca, quiere dos repartos
distintos.** Por eso esto va en el PERFIL y no en el kernel -- que es exactamente
lo que el dueno pidio.

---

# ★ 3. LO QUE SI SE COPIA DE CELL: EL MODELO DE EJECUCION

Cell acerto en algo que no dependia de su hardware:

1. **Un maestro que orquesta y no compite.** El PPE reparte; no se pone a hacer
   el trabajo de un SPE.
2. **Tareas cerradas** (*run-to-completion*): un obrero recibe un trabajo con
   **todo lo que necesita**, lo hace, y contesta. No pregunta a mitad.
3. **Sin estado global compartido.** Un SPE no podia tocar lo del vecino aunque
   quisiera.

★ **Y las tres encajan con lo que BMO-X ya es.** Lo que en Cell imponia el
silicio, aqui lo impone el **contrato**: un obrero que solo tiene las
capabilities de su trabajo *no puede* tocar nada mas -- no porque no alcance,
sino porque no se le concedio.

> **Un obrero con su capability es un SPE con su local store. La diferencia es
> que aqui el aislamiento es una decision revisable, y alli era una pared.**

Eso es el *"mas alla"* de la pregunta: **Cell aislo por hardware y no pudo
soltarlo nunca; BMO-X aisla por capability y puede afinar cuanto**.

---

# ★★ 4. EL MODELO MAESTRO, CONCRETO

```
  Nucleo 0   MAESTRO   dueno del kernel: drivers, CABINA, scheduler,
                       los 209 `static mut`. NO CAMBIA NADA.
  Nucleos 1-5 OBREROS  solo tareas cerradas. Nunca tocan un driver.
```

## La regla de oro, y por que hace esto viable HOY

> **Un obrero no entra en Ring 0 mas que por su propio syscall, y solo puede
> pedir lo que su capability le concede.**

Lo que esto compra es enorme y conviene decirlo con el numero delante: **hay 209
`static mut` en el kernel**, y son una carrera de datos el dia que dos nucleos
los toquen. Pero *solo si los dos los tocan*. Un obrero que computa y no llama a
drivers **no toca ni uno**, y entonces los 209 no son un bloqueo: son una lista
de lo que ese obrero tiene prohibido.

★ Y la infraestructura minima **ya esta puesta**: `SpinLock`
(`ring0/plat/spin.rs`) ya protege los dos unicos sitios que un segundo nucleo
tocaria inevitablemente -- `mm/phys.rs` (el asignador fisico) y `obj/cap.rs` (la
tabla de capabilities).

## ⚠ El dato incomodo: SMT no son seis nucleos mas

6 nucleos / **12 hilos**. Dos hilos del mismo nucleo comparten L1, L2 y las
unidades de ejecucion. Para trabajo de computo puro, **12 obreros no son 12: son
6 con ruido**, y a veces son *peor* que 6 porque se pisan la cache.

El hermano SMT es bueno para lo que **espera memoria**, no para lo que calcula.
`Topology` ya distingue `total_cores` de `total_threads`; el reparto tiene que
mirar el primero por defecto.

---

# ★ 5. LA CABINA DE SMP -- que tiene que confesar

La filosofia del proyecto es *"transparencia total: CABINA lo confiesa todo"*.
Aplicada al paralelismo, lo que hay que poder mirar es:

| Linea | Por que importa |
|---|---|
| APs despiertos **/ esperados**, y **cuales no** por APIC ID | un nucleo que no arranca hoy se ve como "va mas lento" |
| `tid -> nucleo` | sin esto, "se colgo" no distingue *que* nucleo se colgo |
| Veces que un obrero pidio algo que solo el maestro da | mide si la regla de oro se esta respetando |
| **Contencion del `SpinLock`** | si sube, dos nucleos pelean por lo mismo -- es el aviso temprano de la carrera |

★ Sin la ultima linea, SMP se depura a fotos y a suerte. Con ella, la contencion
es un numero que sube antes de que nada falle.

---

# ★★ 6. LO MODULAR -- como entra un CPU nuevo sin tocar el kernel

El gancho **ya existe**: `ring0::cpu_vendor::profile::active()`, con
`cpu_vendor/ryzen_5_5600x/` al lado.

La regla es la del resto del proyecto --**tablas, no cerebros**-- y es la misma
que sostiene los 62 intrinsecos de sem-asm en un TOML y los mods:

```
  perfil de SMP  =  una FILA, no un modulo de codigo

    nucleos - hilos por nucleo - CCX - que nucleos comparten L3
    cual es el maestro - cuantos obreros - quien NO se usa
```

Un CPU nuevo es **una fila mas**. El kernel lee la fila; no sabe que CPU es.

> Y la prueba de que la fila esta bien elegida: **un Zen 2 y un Zen 3 tienen que
> poder describirse con las mismas columnas** y salir dos repartos distintos. Si
> hace falta un `if` por modelo en el kernel, la tabla esta mal disenada.

---

# ★ 7. EL ESTADO REAL HOY, medido el 2026-08-06

| | |
|---|---|
| `smp_startup()` | **existe y no lo llama nadie** (`faggin/s1_cpu`) |
| `ap_entry64` | hace `lock inc` de un contador y `hlt` para siempre |
| Memoria del trampolin | **ya reservada**: `phys.rs` reserva <1 MiB con el comentario *"future SMP trampoline lives here"* |
| `SpinLock` | existe, y ya cubre `phys` y `cap` |
| `static mut` en el kernel | **209** (el peor: `dev/usb.rs` con 30) |

## ⚠ Y el hallazgo que hay que decir antes de tocar nada

**`smp_startup()` no esta solo sin llamar: esta MAL COLOCADO.** Se apoya en
`s1_cpu`, *antes* de `ExitBootServices`, y ahi:

1. **El firmware todavia es dueno de los APs.** UEFI los tiene aparcados en su
   propio bucle (MP Services). Mandarles INIT+SIPI por debajo mientras el
   firmware sigue vivo es pelearse con el por unos nucleos que aun no son
   nuestros -- y lo siguiente que hace `s1_cpu` es **llamar al firmware otra
   vez** (`con_mark`, `ExitBootServices`).
2. **Escribe en 0x7000-0x18200**, memoria que antes de EBS pertenece al
   firmware.
3. **Habla solo por serial** (`ser_print!`), y en esta maquina **no hay cable**:
   se llamaria y no se veria absolutamente nada.

El propio codigo lo dejo dicho: *"SMP remains disabled until its real-mode
trampoline and low-memory page tables are reserved and built correctly. Boot the
BSP reliably first."* La reserva ya esta hecha --la hizo el kernel--; lo que falta
es **mover el bring-up a despues de EBS**, que es cuando esa memoria y esos
nucleos son de BMO.

---

# ★★ 8. EL ORDEN QUE PROPONE ESTE DOCUMENTO

1. **Portar el bring-up a post-EBS** (al kernel, que ya tiene LAPIC, IDT propia
   y la memoria baja reservada). Es mover trampolin + GDT de SMP + `ap_entry`.
2. **Que se reporten en CABINA, no por serial.** El hito es una linea en
   pantalla: `SMP: 6/6 nucleos - 12/12 hilos`, y **decir cual falta si falta**.
3. **Un obrero que solo compute.** Sin drivers, sin CABINA, sin scheduler. Es
   seguro hoy y sin tocar ninguno de los 209.
4. **Medir antes de dar trabajo de verdad**: contencion del SpinLock y
   `tid -> nucleo` en CABINA.
5. **El hilo del bus USB, EL ULTIMO.** Y esto va subrayado: es lo que pide el
   comentario de `usb.rs`, y es **el peor primer trabajo posible** -- 30
   `static mut`, el maximo del repositorio, y el compositor sondeandolo desde
   dentro de un syscall en el BSP. Estrenar SMP ahi es estrenarlo en el camino
   de entrada, que ya es el que falla de forma invisible.

---

# El resumen en una frase

> **De Cell se copia el reparto, no el transporte.** El transporte --mover datos a
> mano entre local stores-- existia porque a los SPE les faltaba coherencia; a los
> seis nucleos del 5600X les sobra, con 32 MB de L3 compartida. Lo que si vale de
> Cell es un maestro que orquesta y obreros que reciben trabajos cerrados, y eso
> en BMO-X no hay que inventarlo: **es una capability**.

Ver [`ARQUITECTURA.md`](../ARQUITECTURA.md), `PLAN_VULKAN.md` para el mismo
metodo aplicado a la GPU, y `AVANCES.md` para el estado.
