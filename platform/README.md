# `platform/` — los contratos, y lo que sí depende del CPU

Cuatro cajones, y la frontera entre ellos es lo que hace que un puerto a otro
CPU sea trabajo acotado en vez de una reescritura:

| Cajón | Qué hay | ¿Sabe de CPU? |
|---|---|---|
| `abi/` | `bmo-abi` (superficie de syscalls, formato BEF, tipos) y `bmo-rt` | **Casi nada** — ver abajo |
| `shared/` | `bmo-hash` (el único BLAKE3), `bmo-channel`, `bmo-hal`, `hw-profile`, `nvram-log` | No |
| `drivers/` | AHCI, FAT32, NVMe, ESTRATOS, block, USB input, net, audio | Sí, los que tocan MMIO |
| `services/` | Ring 3 | No |

## El plan de portado (y por qué no hace falta una capa nueva)

La pregunta que hay que contestar antes de escribir una línea de aarch64 es:
**¿qué cambia y qué no?**

**No cambia**: la superficie de 3 syscalls (`INVOKE`/`CHANNEL_KICK`/`WAIT`), el
contenedor BEF, `bmo-channel`, `bmo-hash`, los frontends de lenguaje (COBOL, C,
Ada) hasta su última fase, y `USER_IMAGE_BASE`. Son **contratos**, y un contrato
no tiene arquitectura.

**Sí cambia, y en dos sitios que ya existen**:

1. **`Ultra_kernel_<arch>/`** — el árbol del kernel es por CPU **por diseño**
   (hoy `Ultra_kernel_x86-64/`). Un puerto añade un árbol hermano, no toca el
   de al lado. Ahí viven las tablas de descriptores, los stubs de entrada, el
   `SYSCALL`/`SVC`, el paginado y el arranque.
2. **El emisor de cada frontend** — la última fase de cada lenguaje: `sem-asm`
   con sus tablas TOML por arquitectura
   (`toolchain/forge/sem-asm/tables/arch/<arch>/`), y `bmo_lower::x86` con su
   equivalente. Añadir una instrucción sigue siendo **una entrada TOML**; añadir
   una arquitectura es un directorio de tablas y un encoder.

Lo que **sí** hay de CPU en `abi/` es **dato enumerado, no código**:

- `bef::BefArch` — el campo `arch` del header BEF (`X86_64 = 0x01`,
  `Aarch64 = 0x02` ya reservado). Es exactamente lo que hace `e_machine` de ELF:
  el formato no cambia de forma, lleva un campo que dice para qué máquina es.
- `cpu_profiles/` — `x86_64_zen3.rs` y una constante `ACTIVE`. Son **hechos del
  silicio** (tamaños de línea de caché, features), y la regla del proyecto es
  preguntárselos al hardware, nunca hardcodear un contrato encima de ellos.

O sea: **el ABI no se rediseña para exportar a otra arquitectura**. Se le añade
un valor a un enum y un perfil de CPU. Eso es la prueba de que la frontera
estaba bien puesta.

## Nota histórica: `bmo-arch`

Aquí al lado hubo un crate llamado `bmo-arch` (antes `bmo-platform`): 1147
líneas de "el único crate que sabe de CPU", con trait `Arch`, wrappers de
canales y un arranque de Ring 3. Se borró el 2026-07-30 por tres razones, en
este orden de peso:

1. **Cero usuarios.** Ni el kernel, ni `Ultra_userspace`, ni los frontends lo
   enlazaban. Compilaba, y no hacía nada por nadie.
2. **Su abstracción central contradecía la superficie congelada**: exponía
   `syscall(nr, args: &[u64; 6])`, un syscall genérico de seis argumentos. BMO
   tiene **tres** syscalls con un contrato de registros fijo; lo demás son
   subsyscalls dentro de `INVOKE`. Adoptar ese trait habría sido reabrir la
   superficie por la puerta de atrás.
3. **Duplicaba la frontera que ya existe** — la de este README: el árbol del
   kernel por arquitectura, más contratos sin arquitectura. Una capa más no
   separaba nada nuevo; sólo añadía un sitio donde la verdad podía divergir.

Traía además un `[profile.release]` propio que cargo **ignora** por no estar en
la raíz del workspace, y soltaba un warning en cada `cargo build` del
repositorio.

Su diseño no se perdió: es la sección de arriba, y es más corta porque el plan
de portado real no necesitaba un crate — necesitaba que alguien dijera qué es
contrato y qué es CPU.
