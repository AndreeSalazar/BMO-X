# ENTRAR EN SU ECOSISTEMA — las tres estrategias, y sus listas

> Escrito el **2026-08-04**. La pregunta del dueño: *"¿podríamos adaptar que sea
> un poco más generalista para poder entrar en su ecosistema?"*
>
> Respuesta corta: **sí, y hay tres caminos distintos** con costes muy
> distintos. Sólo uno de ellos toca la identidad de BMO-X, y no es el que
> parece.

---

## La pregunta que hay debajo

No es *"¿puede BMO correr programas de otros?"*. Es:

> **¿Cuánto de otro sistema hay que volverse para aprovechar su software?**

Y ahí es donde casi todos los proyectos como éste se pierden: empiezan
implementando POSIX "sólo un poquito" y acaban siendo un Linux peor. La forma
de no perderse es tener las tres opciones separadas y saber qué cuesta cada
una — no en trabajo, sino **en identidad**.

---

# ESTRATEGIA A — PORTAR (recompilar el código fuente)

Coger el `.c` y compilarlo con BMO C.

| | |
|---|---|
| Coste por programa | alto: hay que tocar cada uno |
| Coste de identidad | **cero** |
| Qué alcanza | todo lo que sea C acotado y no pida POSIX |
| Estado | ★ **es lo que ya se hace**. 32/32 sondas |

Es lo que hay hecho y funciona. Su límite no es técnico, es aritmético: un
programa por vez, y los grandes traen dependencias.

---

# ESTRATEGIA B — DEVORAR (traducir el binario al cargar)

Leer un ELF estático, colocar sus secciones como si fueran un `.bex`, y
dejarlo correr. Las banderas `PROVENANCE_ELF` / `PROVENANCE_PE` ya existen en
el header del BEF para esto.

**No es emulación**: es el mismo procesador y las mismas instrucciones. Lo
único que hay que traducir es **cómo pide cosas al sistema**.

## ★ LA LISTA — cuántas llamadas de Linux hacen falta de verdad

Aquí está el dato que cambia la conversación. Linux tiene ~350 llamadas al
sistema. **Un programa estático no usa ni una décima parte.**

### Nivel 0 — sólo computar y salir · **4 llamadas**

| Linux | Qué es | Se traduce a |
|---|---|---|
| `exit_group` | terminar | `TASK_OP_EXIT` |
| `write` | escribir | `TASK_OP_CONSOLE_WRITE` |
| `brk` / `mmap` | pedir memoria | `KIND_MEMORIA` |

Con esto corre cualquier cosa que reciba datos y devuelva datos.

### Nivel 1 — una herramienta de línea de comandos · **~15 llamadas**

Todo lo de arriba, más:

| Linux | Se traduce a |
|---|---|
| `read` | `ARCH_OP_LEER` |
| `openat` | `TASK_OP_ARCHIVO_ABRIR` / `_CREAR` |
| `close` | `ARCH_OP_CERRAR` |
| `lseek` | `ARCH_OP_POSICIONAR` ← *pieza `3.3`, aún no está* |
| `fstat` | `ARCH_OP_TAMANO` (parcial) |
| `mprotect` `munmap` | no-op honesto o rechazo |
| `readlink` `access` | rechazo con "no existe" |
| `ioctl(TCGETS)` | lo usa `isatty`: se contesta "no soy terminal" |

★ **Con quince filas de tabla corren `gzip`, `grep`, `sqlite3`, `lua`, `jq` y
la mitad de las herramientas de Unix** — siempre que estén enlazadas
estáticamente.

### Nivel 2 — con tiempo y directorios · **~22 llamadas**

Más `clock_gettime`, `getdents64`, `stat`, `getcwd`, `uname`.

### Nivel 3 — donde se acaba · **+300 llamadas**

`clone`, `futex`, `rt_sigaction`, `fork`, `execve`, `wait4`, `pipe`, `poll`,
`epoll`, `socket`… Aquí ya no es una tabla: es implementar hilos, señales,
procesos y red. **Es exactamente la frontera que el dueño dijo no querer**, y
resulta que la frontera cae en un sitio muy concreto y muy defendible.

## ★★ Lo que hace que esto NO cueste identidad

> **La tabla vive en Ring 3, no en el kernel.**

El traductor es una **librería** que se enlaza con el programa devorado. Coge
la llamada al estilo Linux y la convierte en `INVOKE`. El kernel **no se entera
de que existe Linux** y no crece ni una operación.

Eso es la diferencia entera entre *"BMO tiene una capa de compatibilidad"* y
*"BMO se ha vuelto un Linux peor"*. WSL1 lo hizo al revés —metió la
personalidad de Linux DENTRO del kernel— y por eso acabó sustituido por una
máquina virtual.

## Lo que NO va a funcionar, dicho antes

| | Por qué |
|---|---|
| Programas con enlazado dinámico | piden `ld.so` y `.so` que no existen |
| Cualquier cosa con hilos | `clone` no tiene a dónde traducirse |
| Rutas de Linux (`/etc/...`, `/usr/...`) | el disco es FAT32 con nombres 8.3 |
| Todo lo que use red, señales o `fork` | nivel 3 |
| **Steam, Chrome, navegadores, juegos** | los cuatro de arriba a la vez |

| | |
|---|---|
| Coste | **~15 filas de tabla** para el nivel 1 |
| Coste de identidad | **cero, si la tabla vive en Ring 3** |
| Qué alcanza | herramientas estáticas de Unix que sólo computan y leen ficheros |
| Bloqueado por | el enlazador (para poder tener la tabla como librería) y `3.3` (`lseek`) |

---

## ⚠ Y NO, con Windows no es lo mismo — la asimetría que decide

La intuición es razonable: *"si traduzco las llamadas de Linux, traduzco las de
Windows igual"*. Pero los dos sistemas se parecen **por arriba** y no por
abajo, y lo que hay que traducir está abajo.

| | Linux | Windows |
|---|---|---|
| ¿Está documentada su capa de syscalls? | **sí** | **no**. La API nativa (`NtCreateFile`, `NtWriteFile`) nunca se documentó |
| ¿Es estable? | ★ **para siempre.** Es la regla de Linus: *no se rompe el espacio de usuario*. El número 1 es `write` desde hace décadas | **no**. Los números cambian entre versiones, y a veces entre parches |
| ¿Hay binarios estáticos? | sí, y son comunes | **casi nunca**. Un `.exe` siempre importa de `kernel32.dll` y compañía |

Las tres filas apuntan al mismo sitio:

> **Un ELF estático es autónomo: trae todo lo que necesita y sólo habla con el
> kernel. Un `.exe` es un archivo lleno de agujeros que sólo se llenan si están
> las DLL de Windows.**

Devorar un PE te deja un programa que, en cuanto arranca, pide `kernel32.dll`.
Y esa DLL no es una tabla de quince filas: es **la API de Windows**, y
reimplementarla ES Wine — veinticinco años y millones de líneas, y por eso Wine
trabaja ahí arriba y no en las syscalls.

**Conclusión, sin adornos**: la estrategia de devorar vale para Linux y **no**
para Windows. Las banderas `PROVENANCE_PE` pueden quedarse en el header —
cuestan cero— pero como plan no lo son.

---

# ESTRATEGIA C — HABLAR SU FORMATO (que otros compilen PARA BMO)

La contraria de las dos anteriores: en vez de leer sus binarios, hacer que sus
compiladores produzcan los tuyos.

Un backend de LLVM que emita BEF. Con eso, **cualquier lenguaje que compile por
LLVM** —Rust, Zig, C, C++, Swift, Julia— podría dirigirse a BMO-X sin tocar el
toolchain propio.

| | |
|---|---|
| Coste | ★ meses, y es un proyecto de compilador |
| Coste de identidad | cero, pero **ata a LLVM**, que es justo de lo que este proyecto huyó |
| Qué alcanza | todo lo que compile por LLVM y no pida POSIX |

Se anota porque existe y porque la decisión de *no* tomarlo debe ser una
decisión, no un olvido.

---

# ★ EL ORDEN, y por qué

1. **Seguir portando (A)** mientras el catálogo sea pequeño. Es lo que hay.
2. **El enlazador** — que no es de este documento pero bloquea el B, porque la
   tabla de traducción tiene que poder ser una librería.
3. **`3.3` posicionar por byte** — sin `lseek` no hay herramienta de Unix que
   valga.
4. **Devorar, nivel 1 (B)** — quince filas, y entra medio Unix estático.
5. Y **no** el nivel 3, que es donde BMO dejaría de ser BMO.

## La frase que resume el documento

> **Entrar en su ecosistema cuesta quince filas de una tabla. Quedarse a vivir
> en él cuesta el proyecto.**

Los niveles 0, 1 y 2 son una tabla de traducción en Ring 3 y no comprometen
nada. Del nivel 3 en adelante hay que implementar hilos, señales y procesos —
y para entonces ya no estás adaptando BMO-X: estás escribiendo un Linux, con
veinte años de retraso y una persona.
