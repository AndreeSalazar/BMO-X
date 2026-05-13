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

---

## 🔜 Por hacer (prioridad)

1. **Bridge BMO/GSP** (lo lleva el usuario en `drivers/gpu/fastgpu/`).
2. Conectar `barex::graphics::BxDevice::primary()` al bridge cuando esté listo.
3. Implementar `arch::x86_64::tsc::read_ns()` para dar vida a `BmoInstant::now()`.
4. Parser real de header BEF en `bef::load`.
5. `xhci::probe()` real para detectar el host controller del chipset 500-series.
6. Loop de poll HID que rellene la cola de `barex::input`.
7. Stream isoch OUT al headset Redragon vía `usb::audio_class::submit_pcm`.
8. Dispatcher real en `syscall::dispatch` con BMO ABI.

---

## 🛡️ Reglas de la casa

- **NO tocar** `drivers/gpu/fastgpu/` (bridge BMO/GSP en obra del usuario).
- Cada módulo nuevo: `#![allow(dead_code)]` mientras está en stub.
- Toda función no implementada devuelve `BxError::NotImplemented` (nunca panic).
- Antes de pushear: `cargo build` debe terminar `Finished`.
- Especs y código se sincronizan en ambos paths del MAPA (FastOS y SigDead).
