# BMO ABI — Especificación v1.0

> **Estado**: vivo, en evolución.
> **Última revisión**: v1.8.8 (post-Opus).
> **Mantenedor**: equipo FastOS.

---

## 0. ¿Qué es BMO ABI?

**BMO ABI** es la **interfaz binaria y de programación** que define cómo un
programa (compilado a BEF, sea BMO, C, o cualquier frontend que use este ABI)
interactúa con FastOS.

Reemplaza:

- el **C ABI** (cdecl/stdcall/Win64/SysV AMD64) y
- la **C standard library** (`<stdint.h>`, `<stddef.h>`, `<string.h>`,
  `<errno.h>`, `<time.h>`, `<stdio.h>`, etc).

**No** es un reemplazo de los lenguajes: es un **contrato** que **todos los
lenguajes pueden usar** para hablar con FastOS.

```
╭──────────────╮
│ C            │────┐
│ BMO          │────┤
│ Java-BMO     │────┼──► BMO ABI ──► BEF ──► Ring 3 ──► BMO CORE
│ Python-BMO   │────┤
│ Rust-BMO     │────┘
╰──────────────╯
```

---

## 1. Principios de diseño

| # | Principio | Significado |
|---|-----------|-------------|
| 1 | **Modular** | Cada sub-módulo es autocontenido. Una app puede importar solo lo que necesita. |
| 2 | **Sin VM obligatoria** | AOT es preferido, pero se permite runtime modular por lenguaje. |
| 3 | **BEF es el formato canónico** | Todo programa compilado a BEF puede cargarse. |
| 4 | **ABI explícito, no implícito** | Tamaños, alineaciones, layouts están documentados. |
| 5 | **Manejo de errores unificado** | `BmoError` con códigos extendidos, propagable a través de `BmoResult<T>`. |
| 6 | **Zero-copy donde sea posible** | IPC, surfaces, strings: pasar `(ptr, len)`, no copiar. |
| 7 | **Handles opacos** | El usuario ve `BmoHandle(0xABCD)`, el kernel sabe qué tipo es. |
| 8 | **Determinismo** | `BmoInstant` es monotónico, no afectado por NTP. |

---

## 2. Estructura del ABI

```
bmo_abi/
├── fundamentals/    — Tipos que TODO código usa
│   ├── primitives/  — int8..int64, uint8..uint64, f32, f64, bool
│   ├── status/      — BmoStatus, ErrorCode
│   ├── handle/      — BmoHandle (genérico, kind, opaco)
│   ├── option/      — Option<T>
│   ├── result/      — BmoResult<T>
│   ├── memory/      — slice, range, align
│   ├── sync/        — atómicos, SpinLock
│   ├── error/       — BmoError unificado
│   ├── convert/     — BmoError ↔ BmoStatus ↔ ErrorCode
│   ├── fmt/         — BmoFormatter, write!
│   └── io/          — File/Pipe/Socket handles
│
├── values/          — Tipos valor
│   ├── string/      — BmoStr, BmoString, ASCII
│   ├── time/        — BmoInstant, BmoDuration
│   ├── reflect/     — TypeDescriptor, Mirror
│   ├── net/         — IPv4/IPv6, SocketAddr
│   ├── math/        — sqrt, sin, cos, pow
│   └── hash/        — FNV-1a, CRC32
│
├── windowing/       — Contrato de ventanas
├── drawing/         — Primitivas 2D
├── input/           — Eventos de teclado/ratón
├── fs/              — Filesystem (tipos, flags)
├── time/            — Tiempo (alta resolución)
├── ipc/             — Ports y mensajes
├── surface/         — Superficies CPU/GPU
├── process/         — Procesos, threads
├── memory/          — Allocator interface
├── error/           — Códigos extendidos
├── gpu/             — Shaders, buffers, dispatch (skeleton)
├── bef/             — Formato BEF header
├── entry/           — Punto de entrada
├── runtime/         — Interfaz para runtimes de lenguajes
├── profile/         — BmoLanguageProfile
├── befcore/         — Protocolo BEFCore (app ↔ BMO CORE)
└── syscalls/        — Tabla única 0x100..0x1FF
```

---

## 3. Syscalls (0x100..=0x1FF)

| Rango | Familia |
|-------|---------|
| 0x100..0x10F | Window manager |
| 0x110..0x119 | Drawing primitives |
| 0x120..0x125 | Window painting |
| 0x130..0x134 | Compositor |
| 0x140..0x149 | Filesystem |
| 0x150..0x153 | Time |
| 0x160..0x162 | Input |
| 0x170..0x173 | Audio |
| 0x180..0x188 | Process / Thread |
| 0x190..0x193 | Memory |
| 0x194..0x197 | **BEFCore** (send/recv/poll/register) |
| 0x1A0..0x1A3 | IPC |
| 0x1C0..0x1C2 | Surface mapping |
| 0x1F0..0x1F3 | Debug / diagnostics |

Ver `syscalls/mod.rs` para la lista completa.

### Convención de llamada (SysV AMD64)

```
RAX = syscall number
RDI = arg0
RSI = arg1
RDX = arg2
R10 = arg3
R8  = arg4
R9  = arg5
RAX = return value (o 0xFFFF_FFFF_FFFF_FFFF on error)
```

---

## 4. Handles

Todo objeto opaco (ventana, archivo, port IPC, surface) es un `BmoHandle`:

```rust
pub struct BmoHandle(pub u32);
```

- `0` = `BmoHandle::NULL` (siempre inválido).
- El **kind** se almacena en una tabla global del kernel.
- El proceso solo ve el `u32`. Para saber qué tipo es, llama
  `bmo_handle_kind(h: BmoHandle) -> BmoHandleKind`.

---

## 5. Errores

`BmoError` es un `u32` con dos campos:

```
bits  0..15  = code (BmoErrorCode, ver error/mod.rs)
bits 16..31  = flags (severity, recoverable, etc.)
```

Códigos: `Ok`, `OutOfMemory`, `InvalidHandle`, `PermissionDenied`,
`NotFound`, `Busy`, `Timeout`, `InvalidArgument`, `Io`, `Internal`,
`Unsupported`, `Cancelled`, `Deadlock`, `Again`.

---

## 6. BEF (Binary Executable Format)

Ver `bef/mod.rs`. Resumen:

```
┌────────────────────────────────┐
│ BEF Header (128 bytes)         │
│   magic: "BEF\0"               │
│   version: (1, 0)              │
│   entry: u64                   │
│   flags: u32                   │
│   bss_size: u32                │
│   ...                          │
├────────────────────────────────┤
│ .text  (código x86-64)         │
├────────────────────────────────┤
│ .rodata                        │
├────────────────────────────────┤
│ .data                          │
├────────────────────────────────┤
│ .relocs                        │
├────────────────────────────────┤
│ .symtab (opcional)             │
└────────────────────────────────┘
```

---

## 7. Punto de entrada

Todo programa BMO ABI exporta un símbolo `_bmo_start`:

```rust
#[no_mangle]
pub extern "sysv64" fn _bmo_start(argc: u64, argv: *const *const u8) -> ! {
    // Llamar a la función main del usuario
    let rc = user_main(argc, argv);
    bmo_exit(rc as i32);
}
```

El loader de BEF salta a `_bmo_start` después de configurar el stack, las
relocalizaciones y los handles por defecto.

---

## 8. Lenguajes y runtimes

```
frontend ─┬─► BMO AST ─┐
          ├─► C  AST  ─┼─► BMO IR ──► AOT x86-64 ──► BEF
          ├─► Rust AST ┤
          └─► etc.     ─┘
                            ▲
                            │
                     runtime hook (opcional)
```

- **C / BMO**: AOT puro, runtime mínimo (`c_min`: `_start`, `memcpy`, syscall wrappers).
- **C++**: AOT + runtime (`cpp_min`: ctor globales, dtor, vtables).
- **Java-BMO**: AOT + `java_core` (class model, GC modular).
- **Python-BMO**: AOT typed + `python_core` (dict, types dinámicos).

Cada lenguaje declara su perfil en `bmo_abi::profile::BmoLanguageProfile`.

---

## 9. Garantías

1. **ABI estable dentro de la misma versión mayor**: un BEF v1.x carga en
   cualquier kernel con BMO_ABI_VERSION 1.y.
2. **No se agregan syscalls en un parche (1.0 → 1.1)**, solo en minor
   (1.x → 1.(x+1)) con deprecation warning de 1 minor.
3. **Handles son procesos-locales**: un handle de un proceso no es válido
   en otro (salvo vía IPC explícito).
4. **Strings son UTF-8 válido obligatorio**. Una función que recibe un
   `BmoStr` puede asumir UTF-8 válido (o se especifica lo contrario).
5. **Time es monotónico**: `BmoInstant::now()` no retrocede, no es
   afectado por NTP.

---

## 10. Glosario

- **ABI**: Application Binary Interface.
- **AOT**: Ahead-Of-Time (compilación antes de ejecutar).
- **BEF**: BMO Executable Format.
- **BEFCore**: protocolo de mensajes app ↔ BMO CORE.
- **BMO**: lenguaje nativo de FastOS.
- **Handle**: referencia opaca a un objeto del kernel.
- **Ring 0**: kernel (CPU privilege level 0).
- **Ring 3**: userland (CPU privilege level 3).
- **Runtime**: código que un lenguaje necesita para ejecutarse (GC, RTTI,
  vtables, etc.).
- **Syscall**: llamada al kernel mediante `syscall` instruction.
