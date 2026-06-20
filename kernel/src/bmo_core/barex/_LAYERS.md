# BareX — Mapa de capas dentro del kernel

Esta carpeta implementa la API moderna **BareX** sobre el **BMO ABI** (que reemplaza al C ABI clásico). Las specs maestras viven en
`combo_Window_Extractor/MAPA de Window/02_BEF_Format/`.

---

## 🧱 Estratificación

```
┌─────────────────────────────────────────────────────────────┐
│ L4   barex/compat        →  PE loader + thunks DX/COM/Win32 │
│      (ejecuta binarios Windows existentes en sandbox)       │
├─────────────────────────────────────────────────────────────┤
│ L3   barex/graphics      →  API gráfica (12 objetos núcleo) │
│      barex/audio         →  bx_audio (USB + HDA + HDMI)     │
│      barex/input         →  bx_input (USB HID directo)      │
│      barex/net           →  bx_net (TCP/UDP/QUIC)           │
├─────────────────────────────────────────────────────────────┤
│ L2   barex/shader        →  DXIL/DXBC/SPIR-V → SASS loader  │
├─────────────────────────────────────────────────────────────┤
│      barex/abi           →  BMO ABI (la base de TODO arriba)│
├─────────────────────────────────────────────────────────────┤
│ L1   drivers/gpu/fastgpu →  Bridge BMO/GSP (NO TOCAR)       │
│      drivers/usb/*       →  xHCI + HID + USB Audio Class    │
│      drivers/{nvme,ahci,pci,serial}                         │
└─────────────────────────────────────────────────────────────┘
```

---

## 📦 Archivos por módulo

| Módulo | Archivo | Spec | Estado |
|---|---|---|---|
| BMO ABI | `abi.rs` | `BareX_API_Spec.md` §3 | 🟢 Tipos definidos |
| Graphics | `graphics/mod.rs` | `BareX_API_Spec.md` | 🟡 12 objetos esqueleto |
| Audio | `audio/mod.rs` | `BareX_Audio_Spec.md` | 🟡 Engine + USB AC support |
| Input | `input/mod.rs` | `BareX_Input_Spec.md` | 🟡 HID kbd/mouse/headset |
| Network | `net/mod.rs` | `BareX_Network_Spec.md` | 🔴 Solo tipos |
| Shader | `shader/mod.rs` | `BareX_Shader_Pipeline.md` | 🟡 Loader stub |
| Compat | `compat/mod.rs` | `BareX_Compat_Shim_Spec.md` | 🔴 Solo detección |

🟢 listo · 🟡 en progreso · 🔴 stub

---

## 🖥️ Hardware soportado *en este equipo* (target dev local)

| Componente | Driver | Capa BareX |
|---|---|---|
| Teclado USB | `drivers::usb::hid` | `barex::input::keyboard` |
| Ratón USB | `drivers::usb::hid` | `barex::input::mouse` |
| Headset Redragon (USB) | `drivers::usb::audio_class` + `drivers::usb::hid` | `barex::audio` (out) + `barex::input::headset_buttons` |
| GPU NVIDIA RTX 3060 | `drivers::gpu::fastgpu` (en progreso) | `barex::graphics` |
| NVMe SSD | `drivers::nvme` | `bef::load` + `barex::audio::stream_from_file` |

Otras categorías (gamepads, VR, volantes, NIC) están **declaradas en la spec
pero no instaladas en este equipo** — sus stubs existen pero no se inicializarán.
