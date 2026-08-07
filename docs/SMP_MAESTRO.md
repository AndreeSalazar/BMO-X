# SMP MAESTRO — perfilar el PARALELISMO como se perfila el CPU

> Escrito el **2026-08-06**. Pregunta del dueño: *"¿SMP perfilar MAESTRO basado
> en Ryzen 5 5600X? Es algo inspirado TOTALMENTE en CELL de PS3 pero MÁS allá,
> para poder exprimir — porque el poder de BMO-X es perfilar en hardware: en la
> entrada es perfilar TOTAL, y en software es genérico."*
>
> El principio es correcto y es el mismo que sostiene el BEF y el BSF. Este
> documento lo lleva al paralelismo, y lo primero que hace es **separar la mitad
> de Cell que hay que copiar de la mitad que sería un error copiar**.

---

# ★ 1. QUÉ FUE CELL DE VERDAD

| | |
|---|---|
| **PPE** | un PowerPC normal. El maestro: orquesta, y computa poco |
| **8 × SPE** | los obreros. 256 KB de **local store** cada uno |
| Acceso a RAM | **ninguno directo**. Sólo por **DMA explícito** |
| Coherencia de caché | **no existe** entre SPEs |

El programador movía los datos a mano, a tiempo, y en trozos de 256 KB. De ahí
salieron los dos titulares de la PS3: *potencia bruta* y *un infierno para
programar*.

## ★★ Y aquí está el detalle que lo decide todo

**El local store no era una idea de diseño: era una respuesta a una carencia.**
Los SPE no tenían caché coherente porque el silicio de 2005 no podía darles una
sin arruinar el presupuesto de transistores. El DMA explícito era **el precio de
esa falta**, no una virtud.

Quien copie el modelo de memoria de Cell en un CPU moderno está **pagando el
precio de una carencia que no tiene**.

---

# ★★ 2. LO QUE DICE EL SILICIO QUE HAY DEBAJO

No es opinión: son los campos que `s1_cpu::detect_cpu` ya rellena por CPUID en
esta máquina.

| Campo detectado | Valor en el 5600X |
|---|---|
| `cores_per_ccx` | **6** |
| `ccx_count` | **1** — *monolítico* |
| `threads_per_core` | **2** (SMT) |
| `l3_size_kb` | **32 MB, COMPARTIDA por los seis** |

**Los seis núcleos ya comparten una "local store" de 32 MB, coherente por
hardware, sin una línea de DMA.** Es más grande que las ocho local stores de
Cell juntas (8 × 256 KB = 2 MB) por un factor de dieciséis, y no hay que
gestionarla.

> **Veredicto**: copiar el modelo de MEMORIA de Cell en este CPU es escribir a
> mano un transporte que el silicio ya te regala, y además hacerlo peor.

## ⚠️ Y el aviso que hace este documento necesario: NO todos los Ryzen son así

Un **Ryzen 5 3600X** (Zen 2) tiene **dos CCX de 3 núcleos**, cada uno con su
propia L3 de 16 MB. Dos núcleos de CCX distintos **no comparten caché**: hablar
entre ellos cuesta un viaje por el Infinity Fabric, y ahí sí hay una frontera de
tipo NUMA que un planificador debería respetar.

**El mismo código, en dos CPUs de la misma marca, quiere dos repartos
distintos.** Por eso esto va en el PERFIL y no en el kernel — que es exactamente
lo que el dueño pidió.

---

# ★ 3. LO QUE SÍ SE COPIA DE CELL: EL MODELO DE EJECUCIÓN

Cell acertó en algo que no dependía de su hardware:

1. **Un maestro que orquesta y no compite.** El PPE reparte; no se pone a hacer
   el trabajo de un SPE.
2. **Tareas cerradas** (*run-to-completion*): un obrero recibe un trabajo con
   **todo lo que necesita**, lo hace, y contesta. No pregunta a mitad.
3. **Sin estado global compartido.** Un SPE no podía tocar lo del vecino aunque
   quisiera.

★ **Y las tres encajan con lo que BMO-X ya es.** Lo que en Cell imponía el
silicio, aquí lo impone el **contrato**: un obrero que sólo tiene las
capabilities de su trabajo *no puede* tocar nada más — no porque no alcance,
sino porque no se le concedió.

> **Un obrero con su capability es un SPE con su local store. La diferencia es
> que aquí el aislamiento es una decisión revisable, y allí era una pared.**

Eso es el *"más allá"* de la pregunta: **Cell aisló por hardware y no pudo
soltarlo nunca; BMO-X aísla por capability y puede afinar cuánto**.

---

# ★★ 4. EL MODELO MAESTRO, CONCRETO

```
  Núcleo 0   MAESTRO   dueño del kernel: drivers, CABINA, scheduler,
                       los 209 `static mut`. NO CAMBIA NADA.
  Núcleos 1-5 OBREROS  sólo tareas cerradas. Nunca tocan un driver.
```

## La regla de oro, y por qué hace esto viable HOY

> **Un obrero no entra en Ring 0 más que por su propio syscall, y sólo puede
> pedir lo que su capability le concede.**

Lo que esto compra es enorme y conviene decirlo con el número delante: **hay 209
`static mut` en el kernel**, y son una carrera de datos el día que dos núcleos
los toquen. Pero *sólo si los dos los tocan*. Un obrero que computa y no llama a
drivers **no toca ni uno**, y entonces los 209 no son un bloqueo: son una lista
de lo que ese obrero tiene prohibido.

★ Y la infraestructura mínima **ya está puesta**: `SpinLock`
(`ring0/plat/spin.rs`) ya protege los dos únicos sitios que un segundo núcleo
tocaría inevitablemente — `mm/phys.rs` (el asignador físico) y `obj/cap.rs` (la
tabla de capabilities).

## ⚠️ El dato incómodo: SMT no son seis núcleos más

6 núcleos / **12 hilos**. Dos hilos del mismo núcleo comparten L1, L2 y las
unidades de ejecución. Para trabajo de cómputo puro, **12 obreros no son 12: son
6 con ruido**, y a veces son *peor* que 6 porque se pisan la caché.

El hermano SMT es bueno para lo que **espera memoria**, no para lo que calcula.
`Topology` ya distingue `total_cores` de `total_threads`; el reparto tiene que
mirar el primero por defecto.

---

# ★ 5. LA CABINA DE SMP — qué tiene que confesar

La filosofía del proyecto es *"transparencia total: CABINA lo confiesa todo"*.
Aplicada al paralelismo, lo que hay que poder mirar es:

| Línea | Por qué importa |
|---|---|
| APs despiertos **/ esperados**, y **cuáles no** por APIC ID | un núcleo que no arranca hoy se ve como "va más lento" |
| `tid → núcleo` | sin esto, "se colgó" no distingue *qué* núcleo se colgó |
| Veces que un obrero pidió algo que sólo el maestro da | mide si la regla de oro se está respetando |
| **Contención del `SpinLock`** | si sube, dos núcleos pelean por lo mismo — es el aviso temprano de la carrera |

★ Sin la última línea, SMP se depura a fotos y a suerte. Con ella, la contención
es un número que sube antes de que nada falle.

---

# ★★ 6. LO MODULAR — cómo entra un CPU nuevo sin tocar el kernel

El gancho **ya existe**: `ring0::cpu_vendor::profile::active()`, con
`cpu_vendor/ryzen_5_5600x/` al lado.

La regla es la del resto del proyecto —**tablas, no cerebros**— y es la misma
que sostiene los 62 intrínsecos de sem-asm en un TOML y los mods:

```
  perfil de SMP  =  una FILA, no un módulo de código

    núcleos · hilos por núcleo · CCX · qué núcleos comparten L3
    cuál es el maestro · cuántos obreros · quién NO se usa
```

Un CPU nuevo es **una fila más**. El kernel lee la fila; no sabe qué CPU es.

> Y la prueba de que la fila está bien elegida: **un Zen 2 y un Zen 3 tienen que
> poder describirse con las mismas columnas** y salir dos repartos distintos. Si
> hace falta un `if` por modelo en el kernel, la tabla está mal diseñada.

---

# ★ 7. EL ESTADO REAL HOY, medido el 2026-08-06

| | |
|---|---|
| `smp_startup()` | **existe y no lo llama nadie** (`faggin/s1_cpu`) |
| `ap_entry64` | hace `lock inc` de un contador y `hlt` para siempre |
| Memoria del trampolín | **ya reservada**: `phys.rs` reserva <1 MiB con el comentario *"future SMP trampoline lives here"* |
| `SpinLock` | existe, y ya cubre `phys` y `cap` |
| `static mut` en el kernel | **209** (el peor: `dev/usb.rs` con 30) |

## ⚠️ Y el hallazgo que hay que decir antes de tocar nada

**`smp_startup()` no está sólo sin llamar: está MAL COLOCADO.** Se apoya en
`s1_cpu`, *antes* de `ExitBootServices`, y ahí:

1. **El firmware todavía es dueño de los APs.** UEFI los tiene aparcados en su
   propio bucle (MP Services). Mandarles INIT+SIPI por debajo mientras el
   firmware sigue vivo es pelearse con él por unos núcleos que aún no son
   nuestros — y lo siguiente que hace `s1_cpu` es **llamar al firmware otra
   vez** (`con_mark`, `ExitBootServices`).
2. **Escribe en 0x7000–0x18200**, memoria que antes de EBS pertenece al
   firmware.
3. **Habla sólo por serial** (`ser_print!`), y en esta máquina **no hay cable**:
   se llamaría y no se vería absolutamente nada.

El propio código lo dejó dicho: *"SMP remains disabled until its real-mode
trampoline and low-memory page tables are reserved and built correctly. Boot the
BSP reliably first."* La reserva ya está hecha —la hizo el kernel—; lo que falta
es **mover el bring-up a después de EBS**, que es cuando esa memoria y esos
núcleos son de BMO.

---

# ★★ 8. EL ORDEN QUE PROPONE ESTE DOCUMENTO

1. **Portar el bring-up a post-EBS** (al kernel, que ya tiene LAPIC, IDT propia
   y la memoria baja reservada). Es mover trampolín + GDT de SMP + `ap_entry`.
2. **Que se reporten en CABINA, no por serial.** El hito es una línea en
   pantalla: `SMP: 6/6 nucleos · 12/12 hilos`, y **decir cuál falta si falta**.
3. **Un obrero que sólo compute.** Sin drivers, sin CABINA, sin scheduler. Es
   seguro hoy y sin tocar ninguno de los 209.
4. **Medir antes de dar trabajo de verdad**: contención del SpinLock y
   `tid → núcleo` en CABINA.
5. **El hilo del bus USB, EL ÚLTIMO.** Y esto va subrayado: es lo que pide el
   comentario de `usb.rs`, y es **el peor primer trabajo posible** — 30
   `static mut`, el máximo del repositorio, y el compositor sondeándolo desde
   dentro de un syscall en el BSP. Estrenar SMP ahí es estrenarlo en el camino
   de entrada, que ya es el que falla de forma invisible.

---

# El resumen en una frase

> **De Cell se copia el reparto, no el transporte.** El transporte —mover datos a
> mano entre local stores— existía porque a los SPE les faltaba coherencia; a los
> seis núcleos del 5600X les sobra, con 32 MB de L3 compartida. Lo que sí vale de
> Cell es un maestro que orquesta y obreros que reciben trabajos cerrados, y eso
> en BMO-X no hay que inventarlo: **es una capability**.

Ver [`ARQUITECTURA.md`](../ARQUITECTURA.md), `PLAN_VULKAN.md` para el mismo
método aplicado a la GPU, y `AVANCES.md` para el estado.
