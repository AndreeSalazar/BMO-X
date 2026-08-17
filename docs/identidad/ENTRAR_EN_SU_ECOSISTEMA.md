# ENTRAR EN SU ECOSISTEMA -- las tres estrategias, y sus listas

> Escrito el **2026-08-04**. La pregunta del dueno: *"podriamos adaptar que sea
> un poco mas generalista para poder entrar en su ecosistema?"*
>
> Respuesta corta: **si, y hay tres caminos distintos** con costes muy
> distintos. Solo uno de ellos toca la identidad de BMO-X, y no es el que
> parece.

---

## La pregunta que hay debajo

No es *"puede BMO correr programas de otros?"*. Es:

> **Cuanto de otro sistema hay que volverse para aprovechar su software?**

Y ahi es donde casi todos los proyectos como este se pierden: empiezan
implementando POSIX "solo un poquito" y acaban siendo un Linux peor. La forma
de no perderse es tener las tres opciones separadas y saber que cuesta cada
una -- no en trabajo, sino **en identidad**.

---

# ESTRATEGIA A -- PORTAR (recompilar el codigo fuente)

Coger el `.c` y compilarlo con BMO C.

| | |
|---|---|
| Coste por programa | alto: hay que tocar cada uno |
| Coste de identidad | **cero** |
| Que alcanza | todo lo que sea C acotado y no pida POSIX |
| Estado | ★ **es lo que ya se hace**. 32/32 sondas |

Es lo que hay hecho y funciona. Su limite no es tecnico, es aritmetico: un
programa por vez, y los grandes traen dependencias.

---

# ESTRATEGIA B -- DEVORAR (traducir el binario al cargar)

Leer un ELF estatico, colocar sus secciones como si fueran un `.bex`, y
dejarlo correr. Las banderas `PROVENANCE_ELF` / `PROVENANCE_PE` ya existen en
el header del BEF para esto.

**No es emulacion**: es el mismo procesador y las mismas instrucciones. Lo
unico que hay que traducir es **como pide cosas al sistema**.

## ★ LA LISTA -- cuantas llamadas de Linux hacen falta de verdad

Aqui esta el dato que cambia la conversacion. Linux tiene ~350 llamadas al
sistema. **Un programa estatico no usa ni una decima parte.**

### Nivel 0 -- solo computar y salir - **4 llamadas**

| Linux | Que es | Se traduce a |
|---|---|---|
| `exit_group` | terminar | `TASK_OP_EXIT` |
| `write` | escribir | `TASK_OP_CONSOLE_WRITE` |
| `brk` / `mmap` | pedir memoria | `KIND_MEMORIA` |

Con esto corre cualquier cosa que reciba datos y devuelva datos.

### Nivel 1 -- una herramienta de linea de comandos - **~15 llamadas**

Todo lo de arriba, mas:

| Linux | Se traduce a |
|---|---|
| `read` | `ARCH_OP_LEER` |
| `openat` | `TASK_OP_ARCHIVO_ABRIR` / `_CREAR` |
| `close` | `ARCH_OP_CERRAR` |
| `lseek` | `ARCH_OP_POSICIONAR` <- *pieza `3.3`, aun no esta* |
| `fstat` | `ARCH_OP_TAMANO` (parcial) |
| `mprotect` `munmap` | no-op honesto o rechazo |
| `readlink` `access` | rechazo con "no existe" |
| `ioctl(TCGETS)` | lo usa `isatty`: se contesta "no soy terminal" |

★ **Con quince filas de tabla corren `gzip`, `grep`, `sqlite3`, `lua`, `jq` y
la mitad de las herramientas de Unix** -- siempre que esten enlazadas
estaticamente.

### Nivel 2 -- con tiempo y directorios - **~22 llamadas**

Mas `clock_gettime`, `getdents64`, `stat`, `getcwd`, `uname`.

### Nivel 3 -- donde se acaba - **+300 llamadas**

`clone`, `futex`, `rt_sigaction`, `fork`, `execve`, `wait4`, `pipe`, `poll`,
`epoll`, `socket`... Aqui ya no es una tabla: es implementar hilos, senales,
procesos y red. **Es exactamente la frontera que el dueno dijo no querer**, y
resulta que la frontera cae en un sitio muy concreto y muy defendible.

## ★★ Lo que hace que esto NO cueste identidad

> **La tabla vive en Ring 3, no en el kernel.**

El traductor es una **libreria** que se enlaza con el programa devorado. Coge
la llamada al estilo Linux y la convierte en `INVOKE`. El kernel **no se entera
de que existe Linux** y no crece ni una operacion.

Eso es la diferencia entera entre *"BMO tiene una capa de compatibilidad"* y
*"BMO se ha vuelto un Linux peor"*. WSL1 lo hizo al reves --metio la
personalidad de Linux DENTRO del kernel-- y por eso acabo sustituido por una
maquina virtual.

## Lo que NO va a funcionar, dicho antes

| | Por que |
|---|---|
| Programas con enlazado dinamico | piden `ld.so` y `.so` que no existen |
| Cualquier cosa con hilos | `clone` no tiene a donde traducirse |
| Rutas de Linux (`/etc/...`, `/usr/...`) | el disco es FAT32 con nombres 8.3 |
| Todo lo que use red, senales o `fork` | nivel 3 |
| **Steam, Chrome, navegadores, juegos** | los cuatro de arriba a la vez |

| | |
|---|---|
| Coste | **~15 filas de tabla** para el nivel 1 |
| Coste de identidad | **cero, si la tabla vive en Ring 3** |
| Que alcanza | herramientas estaticas de Unix que solo computan y leen ficheros |
| Bloqueado por | el enlazador (para poder tener la tabla como libreria) y `3.3` (`lseek`) |

---

## ⚠ Y NO, con Windows no es lo mismo -- la asimetria que decide

La intuicion es razonable: *"si traduzco las llamadas de Linux, traduzco las de
Windows igual"*. Pero los dos sistemas se parecen **por arriba** y no por
abajo, y lo que hay que traducir esta abajo.

| | Linux | Windows |
|---|---|---|
| Esta documentada su capa de syscalls? | **si** | **no**. La API nativa (`NtCreateFile`, `NtWriteFile`) nunca se documento |
| Es estable? | ★ **para siempre.** Es la regla de Linus: *no se rompe el espacio de usuario*. El numero 1 es `write` desde hace decadas | **no**. Los numeros cambian entre versiones, y a veces entre parches |
| Hay binarios estaticos? | si, y son comunes | **casi nunca**. Un `.exe` siempre importa de `kernel32.dll` y compania |

Las tres filas apuntan al mismo sitio:

> **Un ELF estatico es autonomo: trae todo lo que necesita y solo habla con el
> kernel. Un `.exe` es un archivo lleno de agujeros que solo se llenan si estan
> las DLL de Windows.**

Devorar un PE te deja un programa que, en cuanto arranca, pide `kernel32.dll`.
Y esa DLL no es una tabla de quince filas: es **la API de Windows**, y
reimplementarla ES Wine -- veinticinco anos y millones de lineas, y por eso Wine
trabaja ahi arriba y no en las syscalls.

**Conclusion, sin adornos**: la estrategia de devorar vale para Linux y **no**
para Windows. Las banderas `PROVENANCE_PE` pueden quedarse en el header --
cuestan cero-- pero como plan no lo son.

---

# ESTRATEGIA C -- HABLAR SU FORMATO (que otros compilen PARA BMO)

La contraria de las dos anteriores: en vez de leer sus binarios, hacer que sus
compiladores produzcan los tuyos.

Un backend de LLVM que emita BEF. Con eso, **cualquier lenguaje que compile por
LLVM** --Rust, Zig, C, C++, Swift, Julia-- podria dirigirse a BMO-X sin tocar el
toolchain propio.

| | |
|---|---|
| Coste | ★ meses, y es un proyecto de compilador |
| Coste de identidad | cero, pero **ata a LLVM**, que es justo de lo que este proyecto huyo |
| Que alcanza | todo lo que compile por LLVM y no pida POSIX |

Se anota porque existe y porque la decision de *no* tomarlo debe ser una
decision, no un olvido.

---

# ★ EL ORDEN, y por que

1. **Seguir portando (A)** mientras el catalogo sea pequeno. Es lo que hay.
2. **El enlazador** -- que no es de este documento pero bloquea el B, porque la
   tabla de traduccion tiene que poder ser una libreria.
3. **`3.3` posicionar por byte** -- sin `lseek` no hay herramienta de Unix que
   valga.
4. **Devorar, nivel 1 (B)** -- quince filas, y entra medio Unix estatico.
5. Y **no** el nivel 3, que es donde BMO dejaria de ser BMO.

## La frase que resume el documento

> **Entrar en su ecosistema cuesta quince filas de una tabla. Quedarse a vivir
> en el cuesta el proyecto.**

Los niveles 0, 1 y 2 son una tabla de traduccion en Ring 3 y no comprometen
nada. Del nivel 3 en adelante hay que implementar hilos, senales y procesos --
y para entonces ya no estas adaptando BMO-X: estas escribiendo un Linux, con
veinte anos de retraso y una persona.
