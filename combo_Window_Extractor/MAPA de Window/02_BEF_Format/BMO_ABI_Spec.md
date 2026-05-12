# BMO ABI Specification v1.0

**Capa:** Cimiento de toda la superficie BareX (L2/L3/L4) y de la tabla de syscalls FastOS.
**Hardware Target:** AMD Ryzen 5 5600X (Zen 3) — extensible a Zen 4/5 sin romper ABI.
**Filosofía:** Reemplaza el C ABI clásico (cdecl/stdcall/Win64/SysV AMD64). Optimizado para llamadas BareX (muchos handles `u64`, parámetros pequeños, 0–2 floats), con cero deuda histórica de C/Win32.

> **El BMO ABI es para FastOS lo que el C ABI fue para Unix.** Es la convención de llamada universal de Ring 3 ↔ Ring 0 y de Ring 3 ↔ Ring 3 dentro de FastOS.

---

## 1. Por qué un nuevo ABI

| Problema del C ABI clásico | Solución BMO ABI |
|---|---|
| Solo 4–6 GPRs para args int → spills frecuentes en llamadas con muchos handles | **7 GPRs** para args int |
| Shadow space 32 B (MS x64) → desperdicio de stack en cada call | **0 bytes** de shadow space |
| Stack alignment 16 B → no aprovecha cache lines de 64 B | **64 B** alignment (cache line) |
| `HRESULT` de 32 bits, separado de la salida útil → 2 retornos | **`bx_status` 16 B en RAX:RDX** (code + flags + value) |
| `errno` global → no thread-safe sin TLS | **Sin globales** — error viaja por valor |
| Strings `\0`-terminated → strlen O(n) y vector de bugs | **UTF-8 + length explícita** |
| Handles tipados a punteros → UAF silencioso | **Handles 64-bit con generación** → UAF detectado |
| Async = OVERLAPPED + APC + IOCP (Win) o callback hell (Unix) | **SQ/CQ rings io_uring-style** |
| `wchar_t` 16-bit en Win32 vs 32-bit en Unix → conversión perpetua | **UTF-8 puro** en todas partes |

---

## 2. Calling convention

### 2.1 Registros de paso de argumentos (entrada)

| Posición | GPR | XMM (float/vec) | Notas |
|---|---|---|---|
| 1 | **RDI** | XMM0 | igual a SysV |
| 2 | **RSI** | XMM1 | |
| 3 | **RDX** | XMM2 | |
| 4 | **R10** | XMM3 | (no RCX — RCX queda para shifts) |
| 5 | **R8**  | XMM4 | |
| 6 | **R9**  | XMM5 | |
| 7 | **RAX_extra** | XMM6 | ⭐ args extra que MS x64 y SysV no tienen |
| 8+ | stack 64-B-aligned | XMM7+ stack | |

### 2.2 Registros de retorno

| Tipo de retorno | Registros |
|---|---|
| `void` / `()` | — |
| Entero ≤ 64 bits | RAX |
| Estructura ≤ 128 bits (incluye `bx_status`) | **RAX:RDX** |
| Float / `f64` | XMM0 |
| Vec2/3/4 `f32` o `f64` | XMM0 (empaquetado) |
| Más grande | puntero out en RDI (sret), retorno en RAX |
| Flags auxiliares | **R11** (1 bit por flag, hasta 64 flags) |

### 2.3 Caller-saved (volátiles)

```
RAX  R10  R11
XMM8..XMM15
```

### 2.4 Callee-saved (preservados)

```
RBX  RBP  RSP  R12  R13  R14  R15
XMM0..XMM7   ⚠️ (también usados como args; las funciones que reciben floats deben preservarlos al spillar)
```

> Esto invierte la convención de Win64/SysV donde XMM0–7 son volátiles. En el BMO ABI los XMM bajos son estables porque las funciones BareX gráficas operan con vectores que sobreviven a llamadas anidadas (`bx_cmdlist::set_viewport`, `bx_cmdlist::draw`, etc.).

### 2.5 Stack

- **Alignment al `call`:** 64 bytes (cache line completa Zen 3).
- **Shadow space:** 0 bytes.
- **Red zone:** 256 bytes bajo RSP — uso libre por el callee sin reservar.
- **Argumentos por stack:** crecen hacia arriba desde `RSP+0`.
- **Frame pointer (RBP):** opcional. Sin obligación, salvo en debug builds.

---

## 3. Tipos canónicos

### 3.1 `bx_status` (resultado universal)

```rust
#[repr(C)]
pub struct BmoStatus {
    pub code: u32,    // 0 = OK; >0 = BxError as u32
    pub flags: u32,   // partial, retry, truncated, etc.
    pub value: u64,   // handle, contador, lo que aplique
}   // 16 bytes — viaja en RAX:RDX
```

### 3.2 `bx_handle` (puntero opaco con generación)

```text
bit 63        : tag (0 = recurso, 1 = canal/cola activo)
bits 62..56   : kind (7 bits — 128 tipos)
bits 55..40   : generación (16 bits — invalida UAF)
bits 39..0    : índice (40 bits — 1 trillón de slots)
```

Cuando un slot se libera y se reasigna, su generación se incrementa. Cualquier handle viejo apuntando a ese slot devolverá `BxError::BadHandle`. Esto **elimina por construcción** la clase entera de bugs UAF que en C es la #1 fuente de CVEs.

### 3.3 `bx_slice` (string/buffer con longitud)

```rust
#[repr(C)]
pub struct BmoSlice { pub ptr: *const u8, pub len: u64 }
```

Encaja en dos GPRs consecutivos del ABI (ej. RDI:RSI). Reemplaza:
- C strings `\0`-terminated → adiós `strlen` O(n) por llamada.
- Win32 `LPCWSTR` → adiós conversión UTF-16.
- Pares (ptr, len) sueltos en SysV → adiós errores de orden.

### 3.4 Layout de structs `#[repr(bmo)]`

- **Sin padding final** (`sizeof` no se redondea).
- **Campos pequeños primero** para empacar mejor (compilador reordena).
- **Alignment por campo** respetado individualmente.
- **Endianness:** little-endian (x86-64 puro).

---

## 4. Wire format de syscalls

Los syscalls FastOS usan `syscall`/`sysret` con BMO ABI extendido:

| Registro | Uso |
|---|---|
| RAX | Número de syscall (entrada) → `bx_status.code` (salida) |
| RDX | (salida) `bx_status.value` |
| R11 | Flags de salida + RFLAGS guardado |
| RDI RSI RDX R10 R8 R9 | Args 1–6 (igual a Linux x86-64 syscall ABI por compatibilidad de registros) |
| RAX_extra | Si se necesita un 7° arg, se pasa por el slot RAX antes de overwrite |

Detalle completo de la tabla en `FastOS_Syscall_Table_Spec.md`.

---

## 5. Async: SQ/CQ rings

Para I/O y operaciones asíncronas (DirectStorage, sockets, audio submit), el BMO ABI **no usa callbacks ni APCs**. Usa **submission queues** y **completion queues** estilo io_uring:

```diagram
  App BEF                           Kernel FastOS
  ┌──────────┐  push SQE            ┌──────────┐
  │ SQ ring  │ ──────────────────▶  │ Worker   │
  │ (mmap)   │                      │ thread   │
  └──────────┘                      └────┬─────┘
                                         │ ejecuta op
  ┌──────────┐  CQE listo               ▼
  │ CQ ring  │  ◀────────────────  ┌──────────┐
  │ (mmap)   │                     │ Hardware │
  └──────────┘                     └──────────┘
```

- Cero syscalls por op en hot path (solo al rellenar/drenar el ring).
- Cero copias entre user/kernel (rings son mmap compartidos).
- Cero callbacks → la app drena CQ cuando le conviene.

---

## 6. Versionado y estabilidad

- **BMO ABI 1.0** congelado para todo el ciclo de vida de FastOS Ryzen 5000 + RTX 30/40.
- **Rompimientos solo en `2.0`**, paralelos a `1.x` durante 2 años.
- Apps BEF declaran su `abi_version` en el manifest. El kernel rechaza versiones incompatibles.
- Los handles binarios entre `1.x` y `2.0` **no se comparten**.

---

## 7. Implementación en este kernel

Los tipos del BMO ABI viven en:
- `kernel/src/barex/abi.rs` — `BmoStatus`, `BmoHandle`, `HandleKind`, `BmoSlice`, constantes.
- `kernel/src/barex/mod.rs` — `BxError::to_status()`, re-export de versión.
- `kernel/src/syscall/mod.rs` — `Syscall` enum + dispatcher (BMO ABI extendido).

---

## 8. Migración desde C ABI (para apps que vienen de Windows/Linux)

Si una app está escrita en C esperando cdecl/MS x64:
1. Recompilar con un frontend que emita BMO ABI calling sequence (gcc/clang custom backend o trampolines automáticos).
2. **O** vivir bajo el shim L4 (`BareX_Compat_Shim_Spec.md`), que envuelve cada llamada COM/Win32 con un thunk MS x64 → BMO ABI.

El shim L4 paga ~5 ns por llamada por el thunk. Apps **nativas BMO ABI** no pagan nada.

---

## 9. Archivos relacionados

- `BareX_API_Spec.md` — superficie BareX que usa BMO ABI.
- `BareX_Compat_Shim_Spec.md` — thunks Win64 → BMO ABI.
- `FastOS_Syscall_Table_Spec.md` — tabla de syscalls con esta convención.
- `BEF_Executable_Format_Spec.md` — el manifest declara `abi_version`.
- (Implementación) `kernel/src/barex/abi.rs`.
