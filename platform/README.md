# `platform/` -- los contratos, y lo que si depende del CPU

Cuatro cajones, y la frontera entre ellos es lo que hace que un puerto a otro
CPU sea trabajo acotado en vez de una reescritura:

| Cajon | Que hay | Sabe de CPU? |
|---|---|---|
| `abi/` | `bmo-abi` (superficie de syscalls, formato BEF, tipos) y `bmo-rt` | **Casi nada** -- ver abajo |
| `shared/` | `bmo-hash` (el unico BLAKE3), `bmo-channel`, `bmo-hal`, `hw-profile`, `nvram-log` | No |
| `drivers/` | AHCI, FAT32, NVMe, ESTRATOS, block, USB input, net, audio | Si, los que tocan MMIO |
| `services/` | Ring 3 | No |

## El plan de portado (y por que no hace falta una capa nueva)

La pregunta que hay que contestar antes de escribir una linea de aarch64 es:
**que cambia y que no?**

**No cambia**: la superficie de 3 syscalls (`INVOKE`/`CHANNEL_KICK`/`WAIT`), el
contenedor BEF, `bmo-channel`, `bmo-hash`, los frontends de lenguaje (COBOL, C,
Ada) hasta su ultima fase, y `USER_IMAGE_BASE`. Son **contratos**, y un contrato
no tiene arquitectura.

**Si cambia, y en dos sitios que ya existen**:

1. **`Ultra_kernel_<arch>/`** -- el arbol del kernel es por CPU **por diseno**
   (hoy `Ultra_kernel_x86-64/`). Un puerto anade un arbol hermano, no toca el
   de al lado. Ahi viven las tablas de descriptores, los stubs de entrada, el
   `SYSCALL`/`SVC`, el paginado y el arranque.
2. **El emisor de cada frontend** -- la ultima fase de cada lenguaje: `sem-asm`
   con sus tablas TOML por arquitectura
   (`toolchain/forge/sem-asm/tables/arch/<arch>/`), y `bmo_lower::x86` con su
   equivalente. Anadir una instruccion sigue siendo **una entrada TOML**; anadir
   una arquitectura es un directorio de tablas y un encoder.

Lo que **si** hay de CPU en `abi/` es **dato enumerado, no codigo**:

- `bef::BefArch` -- el campo `arch` del header BEF (`X86_64 = 0x01`,
  `Aarch64 = 0x02` ya reservado). Es exactamente lo que hace `e_machine` de ELF:
  el formato no cambia de forma, lleva un campo que dice para que maquina es.
- `cpu_profiles/` -- `x86_64_zen3.rs` y una constante `ACTIVE`. Son **hechos del
  silicio** (tamanos de linea de cache, features), y la regla del proyecto es
  preguntarselos al hardware, nunca hardcodear un contrato encima de ellos.

O sea: **el ABI no se redisena para exportar a otra arquitectura**. Se le anade
un valor a un enum y un perfil de CPU. Eso es la prueba de que la frontera
estaba bien puesta.

## Nota historica: `bmo-arch`

Aqui al lado hubo un crate llamado `bmo-arch` (antes `bmo-platform`): 1147
lineas de "el unico crate que sabe de CPU", con trait `Arch`, wrappers de
canales y un arranque de Ring 3. Se borro el 2026-07-30 por tres razones, en
este orden de peso:

1. **Cero usuarios.** Ni el kernel, ni `Ultra_userspace`, ni los frontends lo
   enlazaban. Compilaba, y no hacia nada por nadie.
2. **Su abstraccion central contradecia la superficie congelada**: exponia
   `syscall(nr, args: &[u64; 6])`, un syscall generico de seis argumentos. BMO
   tiene **tres** syscalls con un contrato de registros fijo; lo demas son
   subsyscalls dentro de `INVOKE`. Adoptar ese trait habria sido reabrir la
   superficie por la puerta de atras.
3. **Duplicaba la frontera que ya existe** -- la de este README: el arbol del
   kernel por arquitectura, mas contratos sin arquitectura. Una capa mas no
   separaba nada nuevo; solo anadia un sitio donde la verdad podia divergir.

Traia ademas un `[profile.release]` propio que cargo **ignora** por no estar en
la raiz del workspace, y soltaba un warning en cada `cargo build` del
repositorio.

Su diseno no se perdio: es la seccion de arriba, y es mas corta porque el plan
de portado real no necesitaba un crate -- necesitaba que alguien dijera que es
contrato y que es CPU.
