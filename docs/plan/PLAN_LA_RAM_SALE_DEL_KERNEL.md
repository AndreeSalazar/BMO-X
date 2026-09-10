# LA RAM SALE DEL KERNEL -- que parte es agnostica, medido

> Idea del dueno, **2026-09-10**: *"que crees una carpeta especial para RAM, que
> viva FUERA de Ring 0 y de la carpeta del kernel x86-64, porque la RAM es
> agnostica: puede vivir en cualquier arquitectura de CPU, no?"*
>
> La intuicion es correcta y **el codigo la respalda con mas margen del que la
> pregunta suponia**. Lo que sigue es la medida, no la opinion.

---

# 1. LA MEDIDA: cuanta arquitectura hay en cada fichero de `mm/`

Se conto en los doce ficheros cuantas veces aparece algo que **solo existe en
x86-64**: `cr3`, `invlpg`, `PML4`, `PDPT`, `asm!`, `TLB`, `HIGH_MEM_BASE`.

```text
   subsistema   lineas   hits de arquitectura   que es
   ----------   ------   --------------------   ---------------------------
   vmm/          1.168                     48   ESTO ES la arquitectura
   phys/           727                      0   el mapa de bits de marcos
   titular/        989                      1   de quien es cada marco
   mm/mod.rs        44                      5   las constantes y el espejo
```

★★ **El unico hit de `titular/` es una PALABRA EN UN COMENTARIO**: la linea que
explica que `Titular::Tabla` significa *"PML4, PDPT, PD o PT"*. Cero codigo.

> No es que la RAM PUEDA ser agnostica. Es que **1.716 de las 2.928 lineas de
> `mm/` ya lo son y nadie lo habia contado**.

---

# 2. PERO "SIN ARQUITECTURA EN EL TEXTO" NO ES "AGNOSTICO"

Y esta es la parte que la idea no ve todavia. Lo que ata a los dos candidatos no
es `asm!`: son **constantes que entran de fuera**.

```text
   titular/   necesita   PAGE, PHYSMAP_SIZE               <- DOS numeros
   phys/      necesita   PAGE, PHYSMAP_SIZE, phys_to_virt,
                         SpinLock, boot_context, titular
```

*** Y esa distincion ya tiene precedente de esta misma semana. `mm/titular/` no
pregunta la hora: **el que programa el descriptor trae el `cuando`**, y por eso
el bit en vuelo no abrio una flecha `mm -> task`. Aqui es el mismo movimiento un
piso mas arriba: el crate no sabe cuanto mide una pagina, **se lo dicen**.

```text
   hoy    static mut TABLA: [u8; MARCOS]     el crate reserva la memoria y
                                             para eso necesita el numero
   luego  fn marcar(tabla: &mut [u8], ...)   el KERNEL guarda, el CRATE decide
```

** Ese es todo el rediseno: **el almacenamiento se queda en el kernel y la
logica se va.** Es lo que ya hacen `bmo-dma-juicio` y `bmo-dma-forma`, que no
tienen ni un `static`.

---

# 3. EL ORDEN, Y POR QUE `titular/` VA PRIMERO

```text
   1. titular/    dos constantes y nada mas          <- el limpio
   2. phys/       seis ataduras, tres de ellas de
                  arranque (boot_context, SpinLock)  <- despues
   3. vmm/        NO SE MUEVE. Es la arquitectura    <- nunca
```

[!] **`vmm/` no es un fallo que arreglar.** Que las tablas de paginas sean de
x86-64 es correcto: ARM tiene cuatro niveles con otros bits, RISC-V tiene Sv39 y
Sv48, y una capa que los unificara seria el precio de tres maquinas que esta no
es -- LEY 24 por escrito.

★ Lo que SI viaja de `vmm/` es su **pregunta**, no su codigo: *"se puede caminar
esta direccion?"* ya vive fuera, en `bmo-fisica-juicio`. Ese es el patron.

---

# 4. [!] DONDE VA, Y LA RAIZ NO ES

La idea decia *"que viva fuera de Ring 0"*, y eso es correcto. Pero **no en la
raiz**:

```text
   la RAIZ es para FRONTERAS   VALKYRIE-ABI, PERFIL, NEUTRO. Ninguna ejecuta
   platform/shared/ es para
   lo que corre en el metal    bmo-dma-juicio, bmo-dma-forma, bmo-fisica-juicio
```

*** Un crate de marcos EJECUTA, asi que va con sus hermanos:
`platform/shared/bmo-marcos`. Y se llama **marcos** y no "ram" a proposito: la
RAM es el material, y lo que se administra son marcos. Un crate llamado `ram`
invitaria a meterle dentro el `memcpy`, el espejo y medio `vmm`.

---

# 5. LO QUE SE GANA, Y NO ES "quedar mas ordenado"

```text
   [x] 989 lineas del camino ROJO se prueban en el ANFITRION
       Hoy `titular/` no tiene ni una fila de banco: es un `static mut` dentro
       de un binario bare-metal. Fuera, es una tabla y un indice -- y las ocho
       reglas del DMA se pueden probar sin arrancar la maquina

   [x] `NEUTRO/ARQUITECTURAS.md` deja de ser una promesa
       Ese fichero dice que el neutro es la MISMA categoria en tres CPU. Con
       el titular fuera, esa frase tiene codigo detras en vez de un parrafo

   [x] el semaforo de Ring 0 baja de 176 ficheros a 172
       Y no por esconderlos: por sacarlos de donde no hacian falta
```

---

# 6. LO QUE CUESTA (L3)

```text
   1. LOS 18 FICHEROS QUE LLAMAN
      `mm::titular::marcar` lo usan 18 ficheros de fuera de `mm/`. La fachada
      tiene que seguir diciendo lo mismo -- igual que el `pub use` que el corte
      en carriles ya dejo puesto. Si costara una linea a quien llama, la
      particion se estaria pagando con el diff de otro

   2. EL GUARDIAN DEL TECHO SE PARTE
      `const _: () = { assert!(MARCOS == PHYSMAP_SIZE / PAGE) }` corre HOY en
      compilacion y es lo que impide que la tabla y el mapa de bits cuenten
      marcos distintos. Con la tabla en el kernel y la logica fuera, ese
      `assert` se queda en el kernel -- y hay que comprobar que sigue siendo
      imposible desincronizarlos, no solo improbable

   3. UN CRATE MAS QUE MANTENER
      Son ya cuatro jueces en `platform/shared/`. El quinto no es gratis
```

---

# 7. ** Y EL ORDEN QUE IMPORTA: PRIMERO EL ARRANQUE

[!] `titular/` se partio en carriles hace tres commits, N5 se cableo ayer, y el
camino de escritura hace una hora. **Nada de eso ha corrido todavia en el
Ryzen.**

```text
   mover ahora     el proximo arranque prueba DOS cosas a la vez, y si sale
                   mal no se sabe cual fue
   mover despues   una linea base verde, y luego un cambio que no toca logica
```

*** Es la misma regla con la que se decidio no encender el cerrojo del portero
duro, y la misma por la que las ocho reglas cuentan antes de cortar. Y es
tambien lo que hizo `NEUTRO/ORDEN.md`: **decidir el sitio y crear la carpeta son
dos cosas distintas.** Aqui esta la decision; la carpeta llega despues del
arranque.

## Las casillas

```text
   [ ] un arranque verde con lo que ya hay (vuelo, mudo, ajenos, centinela)
   [ ] sacar `titular/` a platform/shared/bmo-marcos, con la tabla como
       parametro y la fachada intacta
   [ ] sus filas de banco -- las ocho reglas del DMA, en el anfitrion
   [ ] y solo entonces mirar `phys/`, que trae el boot_context detras
```
