# BareX — Work Log

Tracker del trabajo en `kernel/src/barex/` y subsistemas relacionados.
Sirve para no perder hilo entre sesiones y para que cualquier agente nuevo entienda el estado actual.

---

## ✅ Hecho (orden cronológico)

### Sesión 1 — Mapa & Specs
- Creadas 7 specs en `MAPA de Window/02_BEF_Format/`:
  `BareX_API_Spec`, `BareX_Shader_Pipeline`, `BareX_Compat_Shim_Spec`,
  `DX12_to_BareX_Mapping`, `BareX_Audio_Spec`, `BareX_Input_Spec`, `BareX_Network_Spec`.

### Sesión 2 — Esqueletos kernel
- `kernel/src/barex/{graphics,audio,input,net,shader,compat}/mod.rs`
- `kernel/src/{bef,sched,syscall,sandbox}/mod.rs`
- `cargo build` ✅ sin tocar `drivers/gpu/fastgpu/`.

### Sesión 3 — BMO ABI (cimiento)
- Creada `MAPA/02_BEF_Format/BMO_ABI_Spec.md` ⭐.
- Creada `kernel/src/barex/abi.rs` (single file).
- Stack USB local: `kernel/src/drivers/usb/{xhci,hid,audio_class,descriptors}.rs`
  para teclado + ratón + headset Redragon (VID 0x0C45).
- Renombrado todas las menciones "C ABI" → "BMO ABI" en specs.

### Sesión 4 — BMO ABI multi-carpeta (esta sesión)
- `kernel/src/barex/abi/` reorganizada en **9 sub-carpetas**:
  | Carpeta | Reemplaza |
  |---|---|
  | `primitives/` | `<stdint.h>`, `<stddef.h>`, `<stdbool.h>` |
  | `memory/` | `void*`, `size_t`, alignment helpers |
  | `string/` | `char*`, `wchar_t*`, `<string.h>` |
  | `handle/` | `HANDLE`, `fd`, `IUnknown*` |
  | `status/` | `HRESULT`, `errno`, `GetLastError` |
  | `calling/` | convención de llamada (registros + stack) |
  | `async_io/` | `OVERLAPPED`, IOCP, callbacks |
  | `time/` | `time_t`, `timespec`, `GetTickCount` |
  | `compat/` | thunks Win64 / SysV ↔ BMO ABI |
- 22 archivos `.rs` nuevos + `_README.md` + este `_WORK_LOG.md`.

### Sesión 5 — BEF devour PE/ELF + BMO ABI cleanup
- `kernel/src/bef/` expandido a **9 archivos** + carpeta `loader/` con 4 archivos:
  | Archivo | Rol |
  |---|---|
  | `header.rs` | BefHeader 48 B + BefMagic detector + BefFlags + BefArch |
  | `sections.rs` | SectionKind (15 tipos) + SectionTable parser |
  | `imports.rs` | ImportTable + ImportFlags (lazy/eager/weak) |
  | `exports.rs` | ExportTable con búsqueda por hash BLAKE3-32 |
  | `relocations.rs` | RelocationKind (3 tipos vs 38 ELF / 16 PE) + apply() |
  | `symbols.rs` | Symbol + binding + visibility |
  | `manifest.rs` | Manifest TOML + Provenance (Native/PeDevoured/ElfDevoured) |
  | `signing.rs` | SectionHash BLAKE3 256-bit + verify |
  | `tls.rs` | TlsTemplate (un solo blob) |
  | `loader/mod.rs` | Image + LoadError + dispatcher universal |
  | `loader/native.rs` | BEF nativo |
  | `loader/pe.rs` | ⭐ **DEVOUR PE** (DOS + COFF + Optional64 + section headers + fake-DLL map) |
  | `loader/elf.rs` | ⭐ **DEVOUR ELF** (Ehdr + Phdr + reloc translation x86_64 → BEF) |

- BMO ABI extendido con **3 carpetas más** (total 12):
  | Carpeta | Reemplaza |
  |---|---|
  | `sync/` | `<stdatomic.h>`, `<threads.h>`, Interlocked*, pthread_mutex |
  | `option/` | layout C-FFI estable para `Option<T>` |
  | `result/` | layout C-FFI estable para `Result<T,E>` |

- `sync/` contiene: `atomic.rs` (BmoAtomicU32/U64/Bool + MemOrder), `futex.rs` (BmoFutex wait/wake), `mutex.rs` (BmoMutex futex-backed lock-free fast path).

---

## 🔜 Por hacer (prioridad)

1. **Bridge BMO/GSP** (lo lleva el usuario en `drivers/gpu/fastgpu/`).
2. Conectar `barex::graphics::BxDevice::primary()` al bridge cuando esté listo.
3. Implementar `arch::x86_64::tsc::read_ns()` para dar vida a `BmoInstant::now()`.
4. Pipeline completo del loader BEF nativo (relocs + imports + tls + sandbox).
5. Devour PE: parsear `IMAGE_DIRECTORY_ENTRY_IMPORT` + IAT real.
6. Devour ELF: iterar Phdr, procesar `PT_DYNAMIC` + `DT_NEEDED`.
7. Implementar BLAKE3 real (vs FNV-1a placeholder en `bef::signing`).
8. `xhci::probe()` real para detectar el host controller del chipset 500-series.
9. Loop de poll HID que rellene la cola de `barex::input`.
10. Stream isoch OUT al headset Redragon vía `usb::audio_class::submit_pcm`.
11. Dispatcher real en `syscall::dispatch` con BMO ABI extendido.

---

## 🛡️ Reglas de la casa

- **NO tocar** `drivers/gpu/fastgpu/` (bridge BMO/GSP en obra del usuario).
- Cada módulo nuevo: `#![allow(dead_code)]` mientras está en stub.
- Toda función no implementada devuelve `BxError::NotImplemented` (nunca panic).
- Antes de pushear: `cargo build` debe terminar `Finished`.
- Especs y código se sincronizan en ambos paths del MAPA (FastOS y SigDead).
