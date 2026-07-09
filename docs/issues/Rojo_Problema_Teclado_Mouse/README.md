# Rojo: Problema Teclado y Mouse — Fallo Total

**Fecha:** 2026-07-09
**Hardware:** AMD Ryzen 5 5600X, MSI A320M-A PRO MAX (MS-7C52), 16 GB RAM
**Kernel:** `60009f9c22a4fae4` | **Módulo:** `dac2fb7a8cb6d103`

---

## Estado Actual

| Componente | Estado | Detalle |
|---|---|---|
| XHCI Controller | OK | Detectado en `0xFC6A0000` (CPU SoC) |
| CTRL Init | OK | Controlador inicializa correctamente |
| PORTS | 32 detectados | 0 con CCS=1 (sin dispositivos) |
| HID USB | **FALLA** | `UHID_PTR = None`, enumeración falla |
| PS/2 Fallback | **Activo** | Teclado funciona parcialmente (letras, Enter OK) |
| ESC key | **Fix aplicado** | `ESC_LATCH` seteado en `bmo_api/input.rs:50` |
| Mouse PS/2 | Sin confirmar | `poll_direct_ps2()` soporta mouse |

---

## Bugs Encontrados y Corregidos

### 1. BootInfo Corruption (Heap → Buddy Overlap)
- **Síntoma:** `HCI: NONE` — el módulo leía `xhci_mmio = 0` de BootInfo
- **Causa:** El heap del módulo (`alloc::format!`, `bmo_core::coord::init()`) corrompía el struct BootInfo en `0x7FBF8000`
- **Fix:** Bypass vía RAM markers — capturar `xhci_mmio` en `_module_start` antes de cualquier heap allocation, guardar en `0x9_0140`/`0x9_0150`, leer desde ahí en `init_xhci()`
- **Archivos:** `modules/bmo_core/src/main.rs`

### 2. UB: `&BootInfo` → `*mut BootInfo` aliasing
- **Síntoma:** Compiler optimizaba el write a `xhci_mmio` como dead code (release, opt-level=3)
- **Fix:** Eliminar TODAS las referencias `&BootInfo` del kernel. Usar `read_volatile`/`write_volatile` con raw pointers
- **Archivos:** `boot_phase.rs`, `context.rs`, `entry.rs`, `mod_loader.rs`, `vdso.rs`

### 3. Condición `best_mmio` con 0 puertos vivos
- **Síntoma:** Controlador inicializaba pero `alive=0` → `best_mmio` nunca se seteaba → `HCI: NONE`
- **Fix:** `best_mmio == 0 || alive > best_alive` — siempre guardar el primer controlador que inicializa
- **Archivo:** `modules/bmo_core/src/main.rs:369`

### 4. ESC key no funcional en desktop BMO API
- **Síntoma:** `esc_pressed()` siempre retornaba `false`
- **Causa:** `ESC_LATCH` nunca se seteaba en `translate_scancode()`
- **Fix:** `0x01 => { ESC_LATCH.store(true, Ordering::Relaxed); 0x1B }`
- **Archivo:** `bmo_api/input.rs:50`

### 5. TRT bits invertidos (commit `2270f3ad`)
- **Síntoma:** Control transfers USB fallaban
- **Fix:** `data_in = 3, data_out = 2` (especificación XHCI)
- **Archivo:** `drivers/usb/xhci/src/lib.rs`

### 6. Slot Context sobrescrito (commit `2270f3ad`)
- **Síntoma:** Speed/route del controlador se perdía al configurar endpoints
- **Fix:** Preservar campos del Device Context existente, solo elevar Context Entries
- **Archivo:** `drivers/usb/xhci/src/lib.rs`

---

## Problema Raíz NO Resuelto: CCS=0 en todos los puertos

### Evidencia
- RAM markers confirman: kernel escribe `xhci_mmio = 0xFC6A0000` correctamente
- `port_power_on()` establece PORTSC_PP (bit 9) y espera 100k spin loops
- `port_peek()` muestra PORTSC crudo — **necesito los valores PP y CCS para diagnosticar**
- 32 puertos detectados, 0 con dispositivos (CCS=0)

### Hipótesis
1. **Teclado/mouse en chipset A320** (puertos USB 2.0 negros) — el PCI scan no encontró el segundo controlador XHCI
2. **USB PHY no inicializado** — AMD Ryzen requiere secuencia específica de init
3. **USB Legacy handoff incompleto** — BIOS no liberó los puertos correctamente
4. **Port power no aplica** — el controlador ignora el comando PORTSC_PP

### Para Opus
- **Prioridad #1:** Leer la línea `PORT: PP=X/32 CCS=Y/32` de la pantalla de diagnóstico
- **Prioridad #2:** Probar teclado/mouse en puertos USB AZULES (CPU SoC, panel trasero)
- **Prioridad #3:** Si PP=0, debuggear `port_power_on()` — verificar que el write a PORTSC tome efecto
- **Prioridad #4:** Si PP=32 pero CCS=0, buscar el segundo controlador XHCI (chipset A320) en PCI
- **Prioridad #5:** Agregar soporte para extended scancodes PS/2 en teclas especiales (ESC, arrows, F-keys)

---

## Archivos Modificados en Esta Rama

| Archivo | Cambios |
|---|---|
| `kernel/src/ring0/boot_phase.rs` | Raw pointers, write_volatile, RAM markers, dual XHCI |
| `kernel/src/ring0/dev/pcie.rs` | `find_all_xhci_mmio()`, debug logs |
| `kernel/src/ring0/vdso.rs` | `xhci_mmio_base2`, `set_xhci_mmio2()` |
| `kernel/src/ring0/context.rs` | `boot_info()` → raw pointer |
| `kernel/src/ring0/entry.rs` | Pasa raw pointer a mod_loader |
| `kernel/src/ring0/mod_loader.rs` | Recibe `*const BootInfo` |
| `crates_Personal/ring0/boot_protocol/src/lib.rs` | `xhci_mmio2: u64` |
| `crates_Personal/drivers/usb/xhci/src/lib.rs` | `port_power_on()`, `port_peek()`, `reset_ctrl()`, TRT fix, Slot Context fix |
| `crates_Personal/drivers/usb/uhid/src/lib.rs` | `port_power_on()` antes de `port_reset()`, settling delay |
| `crates_Personal/modules/bmo_core/src/main.rs` | Bypass BootInfo, dual-controller, port diagnostic, simplified init |
| `crates_Personal/desktop/bmo_core/src/desktop/input.rs` | Extended scancode tracking, USB pending buffer |
| `crates_Personal/desktop/bmo_core/src/bmo_api/input.rs` | ESC_LATCH fix |

---

## Commits Relevantes

```
14754125 xHCI: dual-controller support for AMD A320 chipset
2270f3ad teclado post Opus (TRT fix + Slot Context + USB pending)
8a9e0a01 USB HID: XHCI+UHID keyboard+mouse in mod_bmo_core
4645bee9 input: PS/2 keyboard+mouse fallback driver
```
