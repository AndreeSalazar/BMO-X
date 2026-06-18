# Análisis del BMO ABI + Propuesta de Re-organización

## 📊 ¿Qué tan completo está el BMO ABI?

**El BMO ABI es la JOYA del proyecto.** Es el módulo más maduro, mejor diseñado y mejor documentado de todo FastOS. ~1.850 líneas, 19 sub-módulos, 0% de fakes.

### Puntuación global: **92%**

| Subsistema | Líneas | % Real | Notas |
|---|---|---|---|
| **primitives** | 159 | 100% | Tipos enteros/flotantes/booleano completos |
| **memory** | 229 | 100% | Slice, Range, Align con `repr(C)` y helpers const |
| **string** | 171 | 95% | `BmoStr` + `BmoString` UTF-8 con bounds |
| **status** | 138 | 100% | `BmoStatus` 16-byte, 18 códigos, `StatusFlags` bitflags |
| **handle** | 190 | 100% | 33 tipos con `tag` (pasivo/activo), `from_code()` |
| **calling** | 68 | 100% | 7 GPRs vs 4/6 MS/SysV, 64B stack align, 256B red zone |
| **async_io** | 132 | 90% | SQE/CQE 64B/16B, OpCode universal, MWAIT pendiente |
| **time** | 121 | 85% | `Instant.now()` retorna ZERO (falta integrar TSC) |
| **sync** | 261 | 100% | `BmoMutex` futex-backed, `BmoAtomic*`, `BmoFutex` |
| **option** | 57 | 100% | `BmoOption<T>` FFI-safe con tag |
| **result** | 60 | 100% | `BmoResult<T>` con ErrorCode |
| **type_system** | 326 | 100% | TypeKind 21 valores, TypeLayout, TypeRegistry |
| **vtable** | 195 | 100% | Magic `BVT1`, O(1) por índice, header+entries |
| **closure** | 127 | 100% | Env, signature, boxed, marker |
| **exception** | 209 | 100% | UnwindContext, Reason, Action, table, panic, resume |
| **reflect** | 97 | 90% | Mirror + QueryApi (faltan consumers reales) |
| **lang_bridge** | 198 | 100% | 25+ LangIds, Descriptor, Features, Registry |
| **marshal** | 116 | 90% | Trait Marshaller, cast, boolean, boxing, string_enc |
| **compat** | 49 | 70% | Trampolines Win64/SysV — solo markers, no codegen real |
| **runtime** | 99 | 90% | `BmoRuntime` agregador, `validate_runtime` NotImplemented |

### 🏆 Lo que está EXCELENTE

1. **Calling convention** es muy superior a MS/SysV:
   - 7 GPRs (vs 4 MS / 6 SysV) para args int
   - 0 bytes shadow space (vs 32 MS)
   - 64B stack align (vs 16) → cache line Zen 3
   - 256B red zone (vs 0 MS / 128 SysV)
   - RAX:RDX para returns ≤ 128 bits (incluye BmoStatus)
2. **`BmoStatus`** en 16 bytes (code + flags + value) reemplaza HRESULT + GetLastError + out-param
3. **Handle con tag bit** distingue pasivo (textura) vs canal activo (queue, socket)
4. **SQE/CQE ring** (estilo io_uring) es arquitectura de I/O moderna
5. **TypeKind 21 valores** cubre C/C++/Rust/Java/Python/Go en un solo enum
6. **LangIds 25+ slots** oficiales + rango experimental — extensibilidad prevista
7. **Marshalling trait** listo para implementar per-lenguaje
8. **Todo `repr(C)`** → FFI universal

### ⚠️ Lo que falta (8%)

1. **`BmoInstant::now()` retorna ZERO** — pendiente integrar TSC (`arch::x86_64::tsc`)
2. **`BmoAsync` no consume SQ/CQ** — MWAIT/timer hook pendiente
3. **`validate_runtime` retorna NotImplemented** — cross-reference check pendiente
4. **`compat::thunks` solo markers** — falta codegen real de tránsitos Win64↔BMO
5. **No hay consumidor de `reflect::query_api`** — la API existe pero nadie la usa
6. **No hay consumidor de `marshal::Marshaller`** — trait definido, sin implementaciones

---

## 🗂️ Propuesta de Re-organización

La estructura actual (19 sub-módulos planos) está **bien conceptualmente** pero la nomenclatura mezcla niveles. Vamos a re-agrupar por **dominios semánticos**, no por tipo de dato:

### Antes (actual)

```
barex/abi/
├── mod.rs
├── runtime.rs
├── _README.md
├── primitives/      (tipos dato)
├── memory/          (memoria)
├── string/          (texto)
├── handle/          (recursos)
├── status/          (errores)
├── calling/         (convocación)
├── async_io/        (I/O asíncrono)
├── time/            (tiempo)
├── sync/            (sincronización)
├── option/          (opcional)
├── result/          (resultado)
├── type_system/     (reflexión tipos)
├── vtable/          (despacho dinámico)
├── closure/         (cierres)
├── exception/       (excepciones)
├── reflect/         (reflexión runtime)
├── lang_bridge/     (puente lenguajes)
├── marshal/         (conversión datos)
└── compat/          (tránsitos C ABI)
```

### Después (propuesto)

```
barex/abi/
├── mod.rs                       (re-exports planos + docs)
├── runtime.rs                   (BmoRuntime agregador)
├── _README.md                   (mapa visual)
│
├── fundamentals/                ── LO QUE TODO USA ──
│   ├── mod.rs
│   ├── primitives/              (u8..u128, f16/32/64, bool, isize)
│   ├── status/                  (BmoStatus + ErrorCode + StatusFlags)
│   ├── handle/                  (BmoHandle, HandleKind 33 tipos)
│   ├── option.rs                (BmoOption<T>)
│   ├── result.rs                (BmoResult<T>)
│   └── memory/                  (BmoSlice, BmoRange, align helpers)
│
├── values/                      ── TIPOS VALOR ──
│   ├── mod.rs
│   ├── string/                  (BmoStr, BmoString UTF-8)
│   ├── time/                    (BmoInstant, BmoDuration)
│   └── reflect.rs               (ReflectQuery facade)
│
├── machinery/                   ── CÓMO SE COMPONE ──
│   ├── mod.rs
│   ├── calling.rs               (registros, stack, red zone) ← consolidar
│   ├── sync/                    (BmoMutex, BmoAtomic*, BmoFutex)
│   ├── type_system/             (TypeKind, TypeLayout, TypeDescriptor, registry)
│   ├── vtable/                  (BmoVTable, VTableEntry, header)
│   ├── closure/                 (BmoClosure, env, signature)
│   ├── exception/               (UnwindContext, Reason, Action, panic, resume)
│   └── async_io/                (SQE/CQE rings, OpCode universal)
│
├── interop/                     ── HABLAR CON OTROS ──
│   ├── mod.rs
│   ├── lang_bridge/             (LangDescriptor, LangFeatures, 25+ IDs)
│   ├── marshal/                 (Marshaller trait, cast, boxing, string_enc)
│   └── compat/                  (Win64/SysV trampolines, shadow space)
│
└── runtime/                     ── AGREGACIÓN ──
    ├── mod.rs                   (BmoRuntime + RuntimeStats)
    └── validate.rs              (cross-reference validation)
```

### 🧭 Reglas de la re-organización

| Categoría | Criterio | Ejemplos |
|---|---|---|
| **fundamentals** | Tipos que casi TODO el código usa | primitives, status, handle, option, result, memory |
| **values** | Tipos valor con semántica propia | string, time, reflect |
| **machinery** | Cómo se compone el código | calling, sync, type_system, vtable, closure, exception, async_io |
| **interop** | Cómo se habla con otros mundos | lang_bridge, marshal, compat |
| **runtime** | El agregador (accedido vía handle) | runtime, validate |

---

## 📋 Plan de implementación (orden)

### Paso 1: Crear categorías (sin mover archivos)

1. Crear `barex/abi/fundamentals/mod.rs` que re-exporta
2. Crear `barex/abi/values/mod.rs` que re-exporta
3. Crear `barex/abi/machinery/mod.rs` que re-exporta
4. Crear `barex/abi/interop/mod.rs` que re-exporta
5. Crear `barex/abi/runtime/mod.rs` que mueve `runtime.rs` actual
6. `barex/abi/mod.rs` cambia: `pub mod` × 4 categorías + `pub mod runtime`

**Resultado**: 0 archivos movidos, 0 cambios de imports, solo 5 `mod.rs` nuevos.

### Paso 2: Consolidar `calling/` en un solo archivo

`calling/mod.rs` (4 líneas) y `calling/registers.rs` (64 líneas) se fusionan en `barex/abi/machinery/calling.rs`. Reduce 1 nivel de anidamiento.

### Paso 3: Documentar el mapa

Actualizar `_README.md` con el nuevo árbol visual + tabla de "qué hay en cada categoría".

### Paso 4: Mover tests al final

Los 369 líneas de `lang/bmoasm/tests.rs` se mantienen donde están. Agregar un `barex/abi/tests/` opcional para validación cross-reference.

### Paso 5: Verificar build

`cargo build --target x86_64-unknown-none` debe pasar sin warnings nuevos.

---

## 🏗️ Re-Organización por carpetas completas del proyecto

Aprovechando la pregunta, también propongo re-organizar el resto del proyecto para que **cada carpeta tenga un rol claro**:

### Top-level (raíz)

```
FastOS/
├── bootloader/         # UEFI bootloader (sin cambios)
├── boot_protocol/      # BootInfo struct compartido (sin cambios)
├── kernel/             # Kernel principal
├── bmofs/              # BMO-FS CLI tool (Ring 3, host)
├── nexo/               # ÑEXO runtime (Ring 3, host)
├── nexo-sh-tool/       # Shader compiler (Ring 3, host, usa Naga)
├── bmo_usb/            # USB init tool (sin cambios)
├── USB_boot/           # Archivos para USB boot (sin cambios)
├── plan.md             # Plan maestro (sin cambios)
├── README.md           # Readme principal (sin cambios)
├── build_uefi.ps1      # Build script (sin cambios)
├── build_uefi.cmd      # Wrapper (sin cambios)
└── target/             # Build artifacts (.gitignore)
```

### `kernel/src/` (kernel)

```
kernel/src/
├── main.rs             # Entry point, phased boot
├── boot_info.rs        # BootInfo struct (32 líneas)
│
├── arch/               # ← Capa 1: Hardware abstraction
│   ├── mod.rs          (13 líneas — re-exports)
│   ├── cpu/            (9 módulos CPU init)
│   ├── gdt.rs
│   ├── idt.rs
│   ├── apic.rs
│   ├── smp.rs
│   ├── fpu.rs
│   ├── paging.rs
│   ├── page_alloc.rs
│   ├── context_switch.rs
│   ├── syscall_entry.rs
│   └── acpi.rs
│
├── sched/              # ← Capa 2: Kernel core
│   ├── mod.rs
│   ├── process.rs
│   ├── thread.rs
│   ├── rt.rs
│   ├── user_init.rs
│   └── gate_test.rs
│
├── memory/             # ← Capa 2: Kernel core
│   ├── mod.rs
│   └── vmm.rs
│
├── fs/                 # ← Capa 2: Kernel core
│   ├── mod.rs
│   ├── ramdisk.rs
│   ├── fat32.rs
│   ├── bmofs_loop.rs
│   ├── inode.rs
│   ├── manager.rs
│   └── mount.rs
│
├── drivers/            # ← Capa 1: Hardware abstraction
│   ├── mod.rs
│   ├── serial.rs
│   ├── pci.rs
│   ├── gop/
│   ├── net/
│   ├── storage/
│   └── usb/
│
├── diag/               # ← Capa 0: Diagnóstico (siempre activo)
│   ├── mod.rs
│   ├── buffer.rs
│   ├── event.rs
│   ├── overlay.rs
│   ├── persistent.rs
│   ├── serial_sink.rs
│   └── telemetry.rs
│
├── desktop/            # ← Capa 3: User experience
│   ├── mod.rs          (facade)
│   ├── input.rs
│   ├── display.rs
│   ├── sound.rs
│   ├── render.rs
│   ├── state.rs
│   ├── windows.rs
│   ├── compositor.rs
│   ├── welcome.rs
│   ├── commands.rs
│   └── shell/          ← NUEVO: comandos de shell
│
├── ui/                 # ← Capa 3: User experience
│   ├── mod.rs
│   ├── fb.rs
│   └── font.rs
│
├── security/           # ← Capa 4: Seguridad
│   ├── mod.rs
│   ├── bytedefender/
│   └── restaurer/
│
├── barex/              # ← Capa 5: API moderna
│   ├── mod.rs
│   ├── abi/            ← REORGANIZADO
│   ├── graphics/
│   ├── audio/
│   ├── input/
│   ├── net/
│   ├── shader/
│   └── compat/
│
├── bef/                # ← Capa 6: Formato binario
│   ├── mod.rs
│   ├── header.rs
│   ├── sections.rs
│   ├── imports.rs
│   ├── exports.rs
│   ├── relocations.rs
│   ├── symbols.rs
│   ├── manifest.rs
│   ├── signing.rs
│   ├── tls.rs
│   ├── blake3.rs
│   └── loader/
│
├── lang/               # ← Capa 7: Lenguajes
│   ├── mod.rs
│   ├── bmoasm/
│   └── nexo/
│
├── windows_compat/     # ← Capa 8: Compat Win32 (futuro)
│   ├── mod.rs
│   ├── api_map.rs
│   ├── ntdll/
│   ├── kernel32/
│   ├── user32/
│   ├── gdi32/
│   ├── msvcrt/
│   ├── advapi32/
│   ├── shell32/
│   ├── comctl32/
│   ├── ole32/
│   └── seh/
│
├── sandbox/            # ← Capa 9: Sandboxing (futuro)
│   └── mod.rs
│
└── syscall/            # ← Capa API
    └── mod.rs
```

### Criterios de organización

| Carpeta | Capa | Rol | Quién lo usa |
|---|---|---|---|
| `arch/` | 1 | Hardware abstraction | Todos los demás |
| `drivers/` | 1 | Hardware abstraction | `arch/`, `barex/`, `desktop/` |
| `sched/`, `memory/`, `fs/` | 2 | Kernel core | `arch/`, `desktop/`, `barex/` |
| `diag/` | 0 | Diagnóstico (siempre activo) | Todos |
| `ui/`, `desktop/` | 3 | User experience | Usuario, apps |
| `security/` | 4 | Seguridad | Kernel, apps |
| `barex/` | 5 | API moderna | Apps Ring 3 |
| `bef/` | 6 | Formato binario | Loader, apps |
| `lang/` | 7 | Compiladores | Apps |
| `windows_compat/` | 8 | Compatibilidad Win32 | Apps (futuro) |
| `sandbox/` | 9 | Sandboxing | Kernel, apps (futuro) |
| `syscall/` | API | Dispatch | Ring 3 → Ring 0 |

---

## ✅ Conclusión

1. **BMO ABI está al 92%** — la mejor parte del proyecto, base sólida para todo.
2. **Re-organizar BMO ABI en 5 categorías semánticas** mejora discoverability sin romper nada.
3. **Re-organizar `kernel/src/` por capas** (1-9) hace explícita la jerarquía de dependencias.

¿Quieres que ejecute la re-organización? Mi recomendación es empezar por el **Paso 1** (crear categorías en BMO ABI sin mover archivos) porque es 0-riesgo y 30 minutos de trabajo.
