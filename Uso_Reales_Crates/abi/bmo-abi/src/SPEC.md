# BMO ABI — Especificación v2.0

> **Estado**: vivo, en evolución.
> **Última revisión**: v2.0 (post-Opus).
> **Mantenedor**: proyecto BMO.

---

## 0. ¿Qué es BMO ABI?

**BMO ABI** es la **interfaz binaria y de programación** que define cómo un
programa (compilado a BEF) interactúa con BMO.

Reemplaza:

- el **C ABI** (cdecl/stdcall/Win64/SysV AMD64) y
- la **C standard library** (`<stdint.h>`, `<stddef.h>`, `<string.h>`,
  `<errno.h>`, `<time.h>`, `<stdio.h>`, etc).

**No** es un reemplazo de los lenguajes: es un **contrato** que **todos los
lenguajes pueden usar** para hablar con BMO.

Frontends como C y COBOL deben vivir fuera del kernel (por ejemplo en
`crates_Personal/Lenguajes/`) y generar BEF offline. El kernel solo carga BEF,
resuelve imports y ofrece syscalls BMO; no compila lenguajes en Ring 0.

---

## 1. Principios de diseño

| # | Principio | Significado |
|---|-----------|-------------|
| 1 | **Modular** | Cada sub-módulo es autocontenido. Una app puede importar solo lo que necesita. |
| 2 | **Sin VM obligatoria** | AOT es preferido, pero se permite runtime modular por lenguaje. |
| 3 | **BEF es el formato canónico** | Todo programa compilado a BEF puede cargarse. |
| 4 | **ABI explícito, no implícito** | Tamaños, alineaciones, layouts documentados + `static_assert!` en código. |
| 5 | **Manejo de errores unificado** | `BmoStatus` de 16 bytes en RAX:RDX. Sin TLS, sin errno. |
| 6 | **Zero-copy donde sea posible** | IPC, surfaces, strings: pasar `(ptr, len)`, no copiar. |
| 7 | **Handles opacos** | `BmoHandle(0xABCD)` con tag + generation + index. |
| 8 | **Determinismo** | `BmoInstant` es monotónico RDTSC-backed, no afectado por NTP. |

---

## 1.1 Perfil de CPU BMO v1

BMO v1 define un contrato nativo, no un objetivo genérico de escritorio:

| Propiedad | Contrato v1 |
|---|---|
| Arquitectura | x86-64, little-endian |
| Modelo de datos | punteros y `usize` de 64 bits |
| Páginas base | 4 KiB |
| CPU base | AMD Zen 3 (`target-cpu=znver3`) |
| ISA requerida | SSE4.2, AVX, AVX2, FMA, BMI1/2, AES, PCLMULQDQ, RDTSCP, invariant TSC |
| Perfil por defecto | AMD Ryzen 5 5600X |
| Perfil alternativo v1 | AMD EPYC Zen 3 |

El perfil se selecciona en Cargo (`cpu-ryzen-5-5600x` o
`cpu-epyc-zen3`) y queda disponible para registrarse en el manifest BEF.
El loader debe validar
los requisitos mediante CPUID antes de transferir control al programa.
Una futura arquitectura conservará el modelo de datos BMO y BEF, pero
definirá su propio contrato de registros e instrucciones.

---

## 2. Estructura del ABI

```
bmo_abi/
├── fundamentals/       Tipos que TODO código usa
│   ├── primitives/     bx_u8..u128, bx_i8..i128, bx_f32/64, bx_f16, bx_bool
│   ├── status/         BmoStatus (16 B), StatusFlags
│   ├── handle/         BmoHandle (64-bit), HandleKind (34 variants), ops trait
│   ├── capability/     BmoCap, BmoCapSet (bitset 64)
│   ├── option/         BmoOption<T> repr(C) FFI-safe
│   ├── result/         BmoResult<T,E> repr(C) FFI-safe
│   ├── error/          BmoError (16 B, code+flags+context)
│   ├── convert/        BmoStatus ↔ BmoError ↔ ErrorCode
│   ├── string/         BmoStr (16 B borrowed), BmoString (24 B owned)
│   ├── memory/         BmoSlice, BmoSliceMut, BmoRange, BmoAligned
│   ├── buffer/         BmoBuffer (32 B shared memory descriptor)
│   ├── allocator/      BmoAllocator trait, GlobalAllocator wrapper
│   ├── io/             BmoRead, BmoWrite, BmoSeek traits + BmoPipe
│   ├── fmt/            BmoFormatter stack-allocated (256 B buffer)
│   └── sync/           BmoAtomicU32/U64/Bool, BmoSpinLock
│
├── values/             Tipos valor con semántica propia
│   ├── time/           BmoInstant (RDTSC), BmoDuration
│   ├── clock/          BmoClockId, sleep, sleep_until
│   ├── uuid/           BmoUuid 128-bit (RFC 4122)
│   ├── version/        BmoVersion semver (major.minor.patch)
│   ├── math/           sqrt, sin, cos, pow (Newton/Taylor, no_std)
│   ├── hash/           FNV-1a 32/64, CRC32c (SSE4.2), CRC32
│   ├── net/            BmoIpv4Addr, BmoIpv6Addr, BmoSocketAddr
│   └── reflect/        BmoTypeInfo, ReflectQuery (hooked to TypeRegistry)
│
├── runtime/            TypeRegistry, VTableStore, LangBridge
├── windowing/          BmoWindowClass, events (paint/key/mouse/resize)
├── fs/                 BmoFileHandle, BmoOpenFlags, BmoStat, BmoDirEntry
├── surface/            BmoFormat (22 pixel formats), BmoSurfaceInfo
├── error_code/         BmoErrorCode enum (21 codes), severity, flags
├── bef/                Formato BEF completo
│   ├── header/         BefHeader 48 B, BefMagic::detect()
│   ├── sections/       SectionKind (10 types), SectionEntry 48 B
│   ├── symbols/        Symbol 32 B, SymbolKind, SymbolTable
│   ├── relocations/    Relocation 24 B (Abs64/Rel32/Got64)
│   ├── imports/        ImportEntry 24 B, ImportTable
│   ├── exports/        ExportEntry 32 B, ExportTable
│   ├── manifest/       Manifest, Identity, Provenance (Native/PeDevoured/ElfDevoured)
│   ├── tls/            TlsTemplate 24 B, TLS setup
│   ├── signing/        SectionHash 40 B, SignatureHeader 8 B, BLAKE3
│   ├── blake3/         BLAKE3 implementation (294 L, no_std)
│   ├── writer/         BefBuilder + BefSection — produce BEF válido
│   ├── validator/      validate() — comprobación estructural completa
│   └── loader/         load() — runtime loader con callback de imports
│
├── syscalls/           Tabla única 0x100..0x1FF + syscall0..syscall6 wrappers
└── profile/            BmoLanguageProfile + ALL_PROFILES
```

### Tipos repr(C) y tamaños verificados

Cada tipo `#[repr(C)]` tiene un `static_assert!` en línea que verifica su
tamaño en tiempo de compilación. 34 aserciones activas:

| Tipo | Tamaño | Área |
|------|--------|------|
| `BefHeader` | 48 B | bef |
| `SectionEntry` | 48 B | bef |
| `Symbol` | 32 B | bef |
| `Relocation` | 24 B | bef |
| `ImportEntry` | 24 B | bef |
| `ExportEntry` | 32 B | bef |
| `SectionHash` | 40 B | bef |
| `SignatureHeader` | 8 B | bef |
| `TlsTemplate` | 24 B | bef |
| `BmoStatus` | 16 B | fundamentals |
| `BmoError` | 16 B | fundamentals |
| `BmoSlice` | 16 B | fundamentals |
| `BmoSliceMut` | 16 B | fundamentals |
| `BmoRange` | 16 B | fundamentals |
| `BmoAligned` | 16 B | fundamentals |
| `BmoBuffer` | 32 B | fundamentals |
| `BmoStr` | 16 B | fundamentals |
| `BmoString` | 24 B | fundamentals |
| `BmoAllocResult` | 24 B | fundamentals |
| `BmoCap` | 8 B | fundamentals |
| `BmoCapSet` | 8 B | fundamentals |
| `BmoDuplicateResult` | 24 B | fundamentals |
| `BmoWaitResult` | 24 B | fundamentals |
| `ReadResult` | 24 B | fundamentals |
| `WriteResult` | 24 B | fundamentals |
| `SeekResult` | 24 B | fundamentals |
| `BmoVersion` | 12 B | values |
| `BmoUuid` | 16 B | values |
| `BmoIpv4Addr` | 4 B | values |
| `BmoIpv6Addr` | 16 B | values |
| `BmoTypeInfo` | 40 B | values |
| `TypeMeta` | 32 B | runtime |
| `BmoStat` | 72 B | fs |
| `BmoDirEntry` | 296 B | fs |

---

## 3. BMO ABI v2: tres syscalls

| Numero | Nombre | Responsabilidad |
|--------|--------|-----------------|
| 0x00 | `BMO_INVOKE` | Control sincrono sobre una capability |
| 0x01 | `BMO_CHANNEL_KICK` | Notificar trabajo publicado en BMO Channel |
| 0x02 | `BMO_WAIT` | Bloquear hasta cambio de secuencia o deadline |

Filesystem, red, audio, input, compositor y GPU son servicios accesibles por
capabilities y BMO Channel. No agregan nuevas entradas privilegiadas.

### ABI v1 legacy (0x100..=0x1FF)

Esta tabla se conserva solamente para ejecutar y migrar BEF/BEX ABI 1.0. Los
productores nuevos deben emitir exclusivamente las tres primitivas v2.

Estado de migracion:

- `bmo-rt` emite identidad, yield y exit mediante `BMO_INVOKE`.
- El backend COBOL convierte los nombres de tarea v1 a `BMO_INVOKE`; no
  incorpora sus numeros legacy en BEF nuevos.
- Ring 0 conserva un adaptador temporal para artefactos ABI 1.0, pero dirige
  esas operaciones al mismo dispatcher v2 para evitar dos implementaciones.
- El resto de la tabla permanece aislado hasta que cada servicio tenga una
  capability y un protocolo BMO Channel definidos.

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
| 0x190..0x197 | Memory + BEFCore |
| 0x1A0..0x1A3 | IPC |
| 0x1C0..0x1C2 | Surface mapping |
| 0x1F0..0x1F3 | Debug / diagnostics |

Ver `syscalls/mod.rs` para la lista completa.

### Convención de syscall (x86_64)

```
RAX = syscall number
RDI = arg0
RSI = arg1
RDX = arg2
R10 = arg3
R8  = arg4
R9  = arg5
RAX = status code (0 = OK)
RDX = value (handle, contador, etc.)
```

Wrappers: `syscall0()` .. `syscall6()` en `syscalls/` (inline asm, `no_std`).

---

## 4. BEF (Binary Executable Format)

```
┌──────────────────────────────────────┐
│ BefHeader (48 bytes, align 16)       │
│   magic: "BEF1" (u32 LE)             │
│   version: (1, 0)                    │
│   flags: BefFlags (EXECUTABLE, PIE)  │
│   arch: X86_64                       │
│   entry_offset: u64                  │
│   section_table_offset: u64          │
│   section_count: u32                 │
│   total_size: u32                    │
├──────────────────────────────────────┤
│ Section table (entries × 48 B)       │
│   10 tipos: Code, RoData, Data, Bss, │
│   Imports, Exports, Relocs, Symbols, │
│   Manifest, Tls, Signature           │
├──────────────────────────────────────┤
│ Section data (alin. a 8..4096)       │
├──────────────────────────────────────┤
│ Signature trailer (BLAKE3 hashes)    │
└──────────────────────────────────────┘
```

- **Header fijo de 48 B** (vs 64+ ELF, 264+ PE).
- **21 tipos de sección** planeados, 10 implementados.
- **3 tipos de relocación**: Abs64, Rel32, Got64.
- **Hashing**: BLAKE3 256-bit por sección.
- **Firma**: Ed25519 (infraestructura lista).
- **Multiboot**: detecta PE (`MZ`) y ELF (`\x7FELF`) vía `BefMagic::detect()`.
- **Devour**: PE/ELF → BEF (traducción nativa).

### Writer, Validator, Loader

| Componente | Archivo | Función |
|------------|---------|---------|
| Writer | `bef/writer.rs` | `BefBuilder` + `BefSection` → produce `Vec<u8>` BEF |
| Validator | `bef/validator.rs` | `validate()` — comprueba magic, bounds, duplicados, firma |
| Loader | `bef/loader.rs` | `load()` — parsea, asigna memoria, resuelve imports, aplica relocs, TLS |

---

## 5. Handles

`BmoHandle` es un `u64` opaco con tres campos internos:

```
bits  0..47  = index (48-bit object table index)
bits 48..60  = generation (13-bit, detecta use-after-close)
bit  61      = tag (1 = kernel, 0 = user)
bits 62..63  = reserved
```

- `0` = `BmoHandle::NULL`, `0xFFFF_FFFF_FFFF_FFFF` = `BmoHandle::INVALID`.
- El **kind** se almacena en una tabla global del kernel.
- `HandleKind` tiene **34 variantes**: Window, File, Dir, Pipe, Socket, Port,
  Timer, Thread, Process, Semaphore, SharedMem, Surface, GpuBuffer, etc.

---

## 6. Errores

`BmoStatus` (16 B, repr(C)) es el return value universal:

```
[0..3]  code:   u32  — 0 = OK, >0 = error
[4..7]  flags:  u32  — StatusFlags (partial, retry, truncated, etc.)
[8..15] value:  u64  — handle, contador, offset, etc.
```

`BmoError` (16 B) es análogo pero con semántica de error:

```
[0..3]  code:    u32  — error_code::* (21 códigos)
[4..7]  flags:   u32  — StatusFlags
[8..15] context: u64  — payload contextual
```

Todos los `BmoStatus` ↔ `BmoError` ↔ `ErrorCode` tienen conversiones
bidireccionales en `fundamentals/convert/`.

---

## 7. Convención de llamada (BMO Call)

```
Argument registers (7 GPRs): RDI, RSI, RDX, RCX, R8, R9, R10
Return: RAX:RDX (hasta 128 bits)
Stack alignment: 64 B (vs 16 B SysV)
Red zone: 256 B (vs 128 B SysV)
Shadow space: 0 B (vs 32 B Win64)
Scratch registers: RAX, RCX, RDX, RSI, RDI, R8-R11
```

---

## 8. Punto de entrada

Todo programa BMO ABI exporta un símbolo `_bmo_start`:

```rust
#[no_mangle]
pub extern "sysv64" fn _bmo_start(argc: u64, argv: *const *const u8) -> !;
```

El loader de BEF salta a `_bmo_start` después de:
1. Mapear secciones en memoria
2. Aplicar relocalizaciones
3. Resolver imports
4. Inicializar TLS
5. Configurar stack con red zone de 256 B

---

## 9. Garantías

1. **ABI v2 estable**: los tres números core no cambian dentro de v2.x.
2. **Migración gradual**: el kernel v2 acepta BEX ABI 1.0 temporalmente;
   nuevos productores escriben ABI 2.0.
3. **Handles son procesos-locales**: un handle de un proceso no es válido
   en otro (salvo vía IPC explícito).
4. **Strings son UTF-8 válido obligatorio**. Una función que recibe un
   `BmoStr` puede asumir UTF-8 válido.
5. **Time es monotónico**: `BmoInstant::now()` usa RDTSC, no retrocede.
6. **Todos los tipos repr(C) tienen static_assert!** que verifica su tamaño
   en compilación. Si el layout cambia, el build falla.

---

## 10. Glosario

- **ABI**: Application Binary Interface.
- **AOT**: Ahead-Of-Time (compilación antes de ejecutar).
- **BEF**: BMO Executable Format.
- **BEFCore**: protocolo de mensajes app ↔ BMO CORE.
- **BMO**: plataforma nativa y contrato binario de BMO.
- **BLAKE3**: hash criptográfico rápido (sección hashing de BEF).
- **Devour**: traducción de PE/ELF a BEF nativo.
- **Handle**: referencia opaca a un objeto del kernel.
- **Ring 0**: kernel (CPU privilege level 0).
- **Ring 3**: userland (CPU privilege level 3).
- **Runtime**: código que un lenguaje necesita para ejecutarse.
- **Syscall**: llamada al kernel mediante instrucción `syscall`.
- **TypeRegistry**: registro fijo de 256 TypeMeta para reflexión.
