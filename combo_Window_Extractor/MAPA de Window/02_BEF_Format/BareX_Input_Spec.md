# BareX Input (`bx_input`) Specification v1.0

**Capa:** L3 (subsistema de input de FastOS)
**Hardware Target:** USB 2.0/3.x HID (xHCI controller del chipset 500-series), Bluetooth LE 5.x opcional
**Filosofía:** Polling de hardware sub-milisegundo, sin colas asíncronas opacas, soporte directo de **todo** lo que un PC gamer usa, sin la fragmentación histórica de Windows (DirectInput → XInput → Raw Input → WGI → GameInput).

> **Objetivo:** Latencia hardware → app **< 0.5 ms** (vs 4–8 ms típicos en Windows con XInput, 1–2 ms con Raw Input).

---

## 1. Qué heredamos y qué descartamos

| Fuente | Heredamos | Descartamos |
|---|---|---|
| **GameInput** (la API moderna que MS quiere que reemplace todo) | Modelo de "reading" snapshot, soporte universal de devices | Acoplamiento a Windows.Gaming.Input WinRT |
| **Raw Input** (Win32) | Acceso de baja latencia a HID raw | API Win32 con `WM_INPUT` mensajes |
| **XInput 1.4** | Mapeo Xbox-style para gamepads | Limitación a 4 gamepads, sin presión analógica de gatillos en algunos modelos |
| **Steam Input** | Re-mapping universal, gestión por-juego | Dependencia del cliente Steam |
| **OpenXR** | Modelo de tracking VR estandarizado | — |
| **libinput / evdev (Linux)** | Modelo HID directo | XKB byzantino |

**Eliminado por basura legacy de Windows:**
- ❌ DirectInput 8 (DirectX 8-era, 25 años obsoleto pero aún usado por flight sims)
- ❌ MMSystem `joyGetPosEx` (Windows 95-era)
- ❌ WM_KEYDOWN / WM_CHAR / WM_MOUSEMOVE (cola del bucle de mensajes Win32 con coalescing impredecible)
- ❌ Cursor del SO mezclándose con cursor del juego (causa stutter)
- ❌ "Enhance pointer precision" (aceleración no-lineal oculta)
- ❌ Filtros del SO (sticky keys, filter keys, etc.) en modo gaming

---

## 2. Devices soportados

| Categoría | Soporte | Notas |
|---|---|---|
| **Teclado** USB HID | ✅ Nativo | NKRO completo, anti-ghosting, scan rate hasta 8 kHz (Razer/Logitech) |
| **Ratón** USB HID | ✅ Nativo | Polling 1000–8000 Hz, alta resolución (16-bit deltas) |
| **Gamepad Xbox** (One/Series/360) | ✅ XInput protocol nativo | Wired y wireless dongle |
| **Gamepad PlayStation** (DualShock 4, DualSense) | ✅ HID + extensiones | Touchpad, gyro, haptic feedback, adaptive triggers (DualSense) |
| **Gamepad Switch Pro** | ✅ HID custom | |
| **Volantes** (Logitech G29/G923, Thrustmaster, Fanatec) | ✅ Force feedback completo | FFB constante, spring, damper, periodic effects |
| **Pedales independientes** | ✅ HID | |
| **Flight stick / HOTAS** (Thrustmaster, VKB, Virpil) | ✅ HID extendido | Hasta 128 botones, 8 ejes |
| **Joystick analógico arcade** | ✅ HID | |
| **Trackball** | ✅ HID | |
| **Tablet Wacom / drawing pen** | ✅ HID + presión | |
| **VR Controllers** (Oculus Touch, Index, Vive) | ✅ Vía OpenXR | 6DoF + haptics |
| **HMD tracking** | ✅ Vía OpenXR runtime | Quest 3, Index, Vive Pro 2 |
| **Steam Deck Controls** | ✅ HID | |
| **MIDI controllers** | (en `bx_audio::midi`) | Separado intencionalmente |

---

## 3. API núcleo

### 3.1 Modelo "reading" (recomendado, snapshot por frame)

```rust
use barex::input::*;

let inp = bx_input::system();

// Polling al inicio del frame
let reading = inp.poll();

if reading.keyboard().is_pressed(Key::W) {
    player.move_forward();
}

let mouse_delta = reading.mouse().delta();   // (dx, dy) raw, sin aceleración
camera.rotate(mouse_delta.0, mouse_delta.1);

if let Some(gp) = reading.gamepads().first() {
    let lx = gp.axis(Axis::LeftStickX);  // [-1.0, 1.0]
    let trigger = gp.axis(Axis::RightTrigger); // [0.0, 1.0]
    if gp.is_pressed(Button::A) {
        player.jump();
    }
}
```

### 3.2 Modelo "event-driven" (para texto, UI)

```rust
inp.on_event(|ev| match ev {
    InputEvent::Key { key, modifiers, action: KeyAction::Press } => { ... }
    InputEvent::Text(s) => { ui.insert_text(&s); }
    InputEvent::GamepadConnected(gp) => { ui.show_controller_icon(gp.kind()); }
    _ => {}
});
```

### 3.3 Force feedback / haptics

```rust
gp.haptics().rumble(low_freq: 0.6, high_freq: 0.3, duration_ms: 200);

// DualSense adaptive trigger
gp.adaptive_trigger(Trigger::Right, AdaptiveMode::Resistance {
    start_position: 0.3,
    force: 0.8,
});

// Wheel FFB
wheel.ffb().set_constant(force: -0.5);
wheel.ffb().play_periodic(Periodic::Sine { magnitude: 0.4, period_ms: 100 });
```

---

## 4. Stack de hardware

```diagram
╭──────────────────────────────────────────────────╮
│  App BEF                                         │
│   bx_input::system()                             │
╰──────────────────┬───────────────────────────────╯
                   ▼
╭──────────────────────────────────────────────────╮
│  bx_input runtime (Ring 3 user library)          │
│   - HID report parser                            │
│   - Device-specific extensions (DualSense, etc.) │
│   - Mapping, dead zones, response curves         │
│   - OpenXR runtime para VR                       │
╰──────────────────┬───────────────────────────────╯
                   ▼  ioctl FastOS
╭──────────────────────────────────────────────────╮
│  HID Service (Ring 3 privilegiado, único)        │
│   - Multiplexa múltiples apps al mismo device    │
│   - Cola circular zero-copy con timestamps HW    │
╰──────────────────┬───────────────────────────────╯
                   ▼
╭──────────────────────────────────────────────────╮
│  Drivers FastOS (Ring 0)                         │
│   - xHCI (USB 3.x)                               │
│   - Bluetooth HCI                                │
│   - PS/2 controller (legacy fallback teclados)   │
│   - Polling MSI-X event-driven, sin polling CPU  │
╰──────────────────────────────────────────────────╯
```

---

## 5. Latencia y polling

| Componente | Latencia |
|---|---|
| Hardware USB poll @ 1000 Hz | 0.5–1.0 ms |
| Hardware USB poll @ 8000 Hz (Razer/Logitech) | 0.125 ms |
| xHCI driver → HID service (kernel→user) | < 50 µs (zero-copy mmap) |
| HID service → app reading | < 100 µs |
| **Total polling 1 kHz** | **< 1.2 ms** |
| **Total polling 8 kHz** | **< 0.3 ms** |

Comparativa Windows GameInput (mejor caso): ~2–4 ms. XInput: 4–8 ms.

---

## 6. Texto y composición (IME)

- **UTF-8 nativo** internamente (sin UTF-16 / wchar legacy).
- IME para asiáticos (CJK) vía servicio opt-in `bx_ime` (no forzado a la app).
- Sin "Region & Language" panel maze. Layout de teclado se selecciona en sesión.
- **Dead keys** y **compose key** soportados (modelo X11 simplificado).
- Layouts: QWERTY (US, ES, LATAM, UK, FR, DE, IT), AZERTY, QWERTZ, Dvorak, Colemak.

---

## 7. Cursor

- App declara `CursorMode::{Visible, Hidden, Captured, Confined}`.
- En modo `Captured` (FPS games), el cursor desaparece y solo recibes deltas raw.
- **Sin** aceleración del SO. La app aplica su propia sensibilidad (DPI scaling sólo si lo pide).
- Compositor renderiza el cursor solo si la app lo deja visible.

---

## 8. Re-mapping y perfiles

```rust
let profile = InputProfile::new()
    .map(Key::Caps, KeyAction::ToCtrl)     // Caps Lock → Ctrl
    .map_gamepad(Button::A, Button::B)     // intercambio
    .axis_curve(Axis::LeftStickX, Curve::Cubic)
    .dead_zone(Axis::LeftStickX, 0.08);

inp.apply_profile(profile);
```

Perfiles serializables a `.bxinput` (TOML) — el SO puede tener perfiles globales por usuario y por juego (tipo Steam Input pero nativo).

---

## 9. Multi-app y foco

- Cuando una app pierde foco (compositor le quita el primer plano), automáticamente:
  - El input HID se silencia para ella (no recibe más eventos).
  - El cursor se libera si estaba captured.
- Solo la app con foco recibe eventos exclusivos.
- Excepción: hotkeys globales registradas explícitamente (`bx_input::register_global_hotkey`).

---

## 10. VR (OpenXR runtime nativo)

- BareX provee un OpenXR runtime mínimo para HMDs PCVR (Quest Link, Index, Vive).
- Tracking vía USB/Wi-Fi (Quest) o Lighthouse (Index/Vive).
- Integrado con `bx_input` para controllers y con `bx_swapchain` para render estereoscópico.
- Runtime de SteamVR no necesario.

---

## 11. Compatibilidad con el shim L4

Fake DLLs heredadas para juegos Windows:
- `xinput1_4.dll` → wrapper a gamepads BareX.
- `dinput8.dll` → wrapper limitado (suficiente para flight sims).
- `hid.dll` → wrapper a HID service.
- `user32.dll` (subset) → traducción de input a `WM_*` para apps que solo manejan win32 messages.
- `windows.gaming.input` (WinRT) → stub de WGI.

---

## 12. Anti-cheat e inyección

- BareX **no** permite que una app inyecte input en otra (a diferencia de SendInput de Windows que sí lo hace).
- Esto rompe macros tipo AutoHotkey por diseño, pero a cambio elimina toda una clase de cheats client-side.
- Apps tipo "remote control" deben usar el servicio `bx_remote` con permiso explícito del usuario.

---

## 13. Métricas de éxito

| Métrica | Objetivo |
|---|---|
| Latencia ratón 8 kHz | < 0.3 ms |
| Latencia gamepad XInput | < 1.5 ms |
| Latencia VR controller | < 5 ms (motion-to-photon) |
| Reconocimiento de devices al hot-plug | < 200 ms |
| Soporte de un device HID nuevo desconocido | ✅ (parser HID generic) |
| Drift en sticks analógicos | Calibración automática + dead zone tunable |

---

## 14. Archivos relacionados

- `BareX_Audio_Spec.md` (head tracking para spatial audio)
- `BareX_API_Spec.md` (cursor visible solo si compositor lo permite)
- `FastOS_Scheduler_Spec.md` (prioridad alta para HID service)
- `FastOS_App_Sandbox.md` (aislamiento de input entre procesos)
