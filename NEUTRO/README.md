# NEUTRO -- donde acaba la ley, y por que tiene carpeta propia

> **NEUTRO es lo que ejecuta codigo que BMO-X no escribio, no puede leer y no
> puede detener -- y que alcanza la RAM sin pasar por el orquestador.**

Lo nombro el dueno el **2026-09-07**, hablando de la GPU:

> *"considero que si hablamos de GPU, ese ya no vive en RING 0 ni RING 3. Vive
> en **Neutro**. Ese mismo es por algo, para facilitar"*

Y al dia siguiente pidio la carpeta, con el motivo entero dentro:

> *"el NEUTRO es carpeta independiente, cuando llegue mi GPU es por su PSP...
> luego ese mismo NEUTRO va a vivir otras arquitecturas de CPU si es que llega,
> por algo se llama **agnostico**"*

---

## 1. ★★ POR QUE ES UNA CARPETA Y NO UN CAPITULO

Es la misma pregunta que se le hizo a `VALKYRIE-ABI/`, y tiene la misma forma de
respuesta. Un capitulo vive dentro de algo; **esto no vive dentro de nada**:

```text
   Ring 0    lo que ejecuta el CPU con privilegio
   Ring 3    lo que ejecuta el CPU sin el
   NEUTRO    lo que NO EJECUTA EL CPU
```

** Meterlo en `Ultra_kernel_x86-64/` seria decir que es una parte de este
kernel. **No lo es**: la lista de aparatos neutros de esta maquina seria
identica con otro kernel, y el dia que BMO-X arranque en ARM la lista **no
cambia**.

> Un anillo describe un privilegio del CPU. El neutro describe **lo que no
> depende del CPU en absoluto**. No caben en la misma carpeta.

### La diferencia con V-ABI, que tambien esta fuera de los anillos

```text
   V-ABI    no tiene anillo porque NO EJECUTA en la maquina (es un contrato)
   NEUTRO   no tiene anillo porque EJECUTA DEMASIADO: tiene procesador propio
```

Los dos estan fuera, por razones opuestas. Y por eso son dos carpetas y no una.

---

## 2. ★★ AGNOSTICO -- y esto es lo que de verdad justifica la carpeta

Es el argumento del dueno y es correcto. **La categoria no cambia al cambiar de
CPU. Solo cambia el nombre del guardia que no tenemos.**

| | x86-64 | ARM64 | RISC-V |
|---|---|---|---|
| quien escribe sin permiso | un aparato PCIe | un aparato en el bus | igual |
| como llega a la RAM | DMA | DMA | DMA |
| **quien lo vigilaria** | VT-d / AMD-Vi | **SMMU** | **IOMMU** |
| esta puesto en BMO-X? | **no** | -- | -- |

★ **Las tres primeras filas son la MISMA frase.** Solo la cuarta cambia de
nombre, y es justo la que no esta implementada en ninguna de las tres.

** Asi que `NEUTRO/` puede escribirse hoy, entera, sin saber en que CPU va a
correr -- que es la definicion de agnostico y la razon de que no viva dentro de
`Ultra_kernel_x86-64/`. Ver [`ARQUITECTURAS.md`](ARQUITECTURAS.md).

---

## 3. Que hay en esta carpeta

| fichero | contesta |
|---|---|
| [`LEY.md`](LEY.md) | las **seis reglas** N1..N6, cada una con su sacrificio (L3) |
| [`CENSO.txt`](CENSO.txt) | **quien es neutro hoy**, uno por linea. La lista, no la prosa |
| [`ARQUITECTURAS.md`](ARQUITECTURAS.md) | lo agnostico: la misma categoria en x86-64, ARM64 y RISC-V |
| [`REQUISITOS.md`](REQUISITOS.md) | ★ **que falta para completarlo**, con lo hecho marcado |
| [`FRONTERA.txt`](FRONTERA.txt) | que ES neutro y que NO, para no estirar la palabra |
| [`ORDEN.md`](ORDEN.md) | ** **donde va lo que llegue**, decidido antes: se ordena por MECANISMO y no por aparato, y la GPU ya tiene sitio |
| [`DMA/`](DMA/README.md) | ** desde el **09-09**: el MECANISMO. La tercera condicion de esta frontera, por completo |

### Y lo que NO esta aqui, a proposito

```text
   la CATEGORIA y su porque   ->  docs/identidad/EL_NEUTRO.md
   el juez de la cesion       ->  platform/.../bmo-mmio-juicio
   la etiqueta de los marcos  ->  Ultra_kernel_x86-64/.../mm/duenno.rs
   que COPIAR del mundo       ->  docs/maestro/DMA_MAESTRO.md   (el CUANDO)
                              ->  docs/maestro/IOMMU_MAESTRO.md (el DONDE)
   las casillas               ->  docs/plan/PLAN_EL_NEUTRO_VIGILADO.md
```

** Los dos maestros viven en `docs/maestro/` y no aqui **porque un maestro no es
"el documento grande de un tema"**: `docs/README.md` ordena por PREGUNTA, y la
suya es *"que copiar del mundo y que seria un error copiar"*. Esta carpeta
contesta otra --*quien alcanza la RAM sin permiso*-- y por eso los cita en vez de
guardarlos. Ver [`DMA/README.md`](DMA/README.md), seccion 1.

** Es el mismo reparto que `VALKYRIE-ABI/`: **la carpeta guarda el estandar, no
el codigo que lo cumple.** `EL_NEUTRO.md` se queda en `identidad/` porque
contesta *"por que BMO-X hace esto distinto"*, que es otra pregunta y tiene otra
familia (ver `docs/README.md`).

---

## 4. ⚠ LO QUE ESTA CARPETA NO PROMETE

```text
   [ ] no hace la maquina mas segura. Nombrar un agujero no lo tapa
   [ ] no propone escribir una IOMMU. La nombra como lo unico que cerraria esto
   [ ] no aparece en el ABI: Ring 3 no tiene que saber que el neutro existe
   [ ] y ninguna de sus reglas puede convertirse en codigo que ejecute un
       aparato neutro. No mandamos ahi. De eso trata todo esto
```

> El neutro no es un anillo nuevo. Es el nombre de donde acaba la ley.
