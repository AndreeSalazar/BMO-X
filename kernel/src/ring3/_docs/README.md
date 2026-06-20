# Ring 3 — User Space

> Ring 3 es el **user space** de FastOS. Aquí corren los procesos
> de aplicación, sin privilegios. Toda interacción con el kernel
> pasa por la API 0x100..0x1FF (expuesta por BMO Core).

## Estado actual (v1.7.4)

En v1.7.4, Ring 3 es **experimental**:

- Existe el módulo `ring3` con la estructura básica.
- La transición a Ring 3 está parcialmente implementada
  (vía `cpu::ring3_test`).
- Las apps de usuario se cargan como ELF o BMO bytecode
  (todavía no hay un loader estable).

## Estructura

```
ring3/
├── mod.rs        — Init de Ring 3 (carga el primer proceso)
├── _docs/        — Esta documentación
│   ├── README.md — Éste archivo
│   ├── ARCHITECTURE.md — Memory map, transición ring 0↔3
│   ├── loader.md — ELF + BMO loader
│   └── startup.md — Cómo arranca el primer proceso
│
├── loader/       — ELF + BMO bytecode loader
├── startup/      — _start de la primera app
├── syscall/      — Wrapper de syscalls (syscall6 instruction)
├── lib/          — libfastos (user-space C-like runtime)
├── progs/        — Programas de usuario pre-cargados
└── tests/        — Tests de apps de usuario
```

## Concepto

Ring 3 es donde el código de aplicación vive. Tiene:

- Su propio **page table** (CR3 por proceso).
- Su propio **kernel stack** (16 KB) para syscalls.
- Acceso a la API via la instrucción `syscall` (que entra a
  ring 0 via `IA32_LSTAR`).
- **NO** puede tocar hardware directamente.
- **NO** puede leer/escribir memoria de otros procesos.
- **NO** puede modificar sus propios page tables.

## Memory map de un proceso Ring 3

```
0x0000_0000_0000 ──┐
                   │  código (.text)
                   │  datos de solo lectura (.rodata)
                   │  datos inicializados (.data)
                   │  datos no inicializados (.bss)
                   │  heap (crece con brk)
0x0000_0040_0000 ──┘
0x0000_0040_0000 ──┐
                   │  stack (crece hacia abajo)
0x0000_0080_0000 ──┘
0x0000_0080_0000 ──┐
                   │  mmap'd regions (bibliotecas, etc)
                   │  shm regions
0x0000_FFFF_FFFF ──┘
```

## Transición ring 0 → ring 3

1. `coordinator::main` llama a `ring3::init()`.
2. `ring3::init()` carga el ELF del primer proceso (`/init.elf`).
3. Crea un task con `schedule::spawn(entry, user_stack, prio)`.
4. El primer context switch carga el CR3 user, RSP user, y
   hace `iretq` con CS = 0x33 (ring 3), SS = 0x2B (ring 3).
5. La app corre en ring 3.
6. Cuando hace `syscall`, vuelve a ring 0, ejecuta la syscall,
   y al final hace `sysretq` de regreso a ring 3.

## Reglas de Ring 3

- **NO** `unsafe` permitido en user space (las apps se
  compilan sin unsafe; cualquier unsafe de Rust en user
  space panic + kill).
- **NO** acceso directo a Ring 0.
- **NO** modificación de CR3, CR0, CR4.
- **NO** `in`/`out` (I/O ports).
- **SÍ** syscall para todo (256 disponibles).

## Apps incluidas (en `progs/`)

- `init.elf` — el init, abre `/dev/console`, lee `/etc/rc.cfg`,
  y lanza los demás servicios.
- `console.elf` — terminal simple.
- `desktop.elf` — window manager mínimo.
- `test_hello.elf` — sanity test (hace un print y exit).

## Compilación

Las apps se compilan con un target JSON custom (`x86_64-fastos.json`)
que configura:

- `panic-strategy = "abort"`
- `llvm-target = "x86_64-fastos-none"`
- `data-layout` con `e-m:e-...`
- `is-builtin = false`
- Sin `start-builtin = true` (cada app provee su `_start`)

## Toolchain

`cargo` con el target custom:

```bash
rustc --target x86_64-fastos.json -O my_app.rs -o my_app.elf
```

## Debugging

Las apps se pueden debuggear con GDB remoto (planeado para v1.8.0):

- `gdbserver` en el kernel (stub simple que habla por serial).
- GDB cliente conecta y puede hacer `break`, `step`, `continue`.

## Limitaciones v1.7.4

- Sin swap.
- Sin copy-on-write.
- Sin shared libraries.
- Sin dynamic linking.
- Sin POSIX subsystem completo (sólo lo que la API 0x100..0x1FF provee).
- Sin fork (sólo spawn).
- Sin threads (cada proceso es single-threaded en v1.7.4).

Ver `ARCHITECTURE.md` para más detalle de memory map y
transición ring 0↔3.
