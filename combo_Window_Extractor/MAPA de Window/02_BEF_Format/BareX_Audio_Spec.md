# BareX Audio (`bx_audio`) Specification v1.0

**Capa:** L3 (subsistema de audio de FastOS, hermano de `bx_graphics`/BareX)
**Hardware Target:** Realtek ALC1220 / ALC4080 (chipsets B550/X570 típicos del Ryzen 5600X) + USB Audio Class 2/3 + HDMI Audio (RTX 3060)
**Filosofía:** Latencia de hardware sin compromiso. Heredamos lo bueno de XAudio2 / WASAPI Exclusive y descartamos toda la basura legacy (DirectSound, MMSystem, MIDI legacy, ACM, kmixer histórico).

> **Objetivo:** **< 1.5 ms round-trip** en modo exclusivo (vs 3–10 ms típicos en Windows WASAPI Shared, 30–50 ms en Linux PulseAudio default).

---

## 1. Qué heredamos y qué descartamos

| Fuente | Heredamos | Descartamos |
|---|---|---|
| **XAudio2 9** | Voice graph, submix, DSP chain, 3D positional helper | COM, IXAudio2Voice vtables |
| **WASAPI Exclusive** | Acceso directo al endpoint, formato nativo, event-driven buffer | El modelo "Shared" (mezclador kernel), MMDevice enumeration |
| **Steam Audio / Phonon** | HRTF, oclusión, reverb por raytracing | — |
| **Dolby Atmos for Headphones** | Renderizado spatial 7.1.4 | Licencia opcional (driver certificado MS) |
| **OpenAL Soft** | API de audio 3D portable | Drivers vendor-specific |
| **JACK (Linux pro audio)** | Modelo de routing graph low-latency | XML config, dependencias |

**Eliminado por basura legacy de Windows:**
- ❌ DirectSound (DS3D, EAX) — muerto desde Vista
- ❌ MMSystem (`waveOut*`, `mciSendCommand`) — Windows 3.1
- ❌ Audio Compression Manager (ACM)
- ❌ MIDI legacy (`midiOut*`) — usamos USB MIDI 2.0 directo
- ❌ Kmixer / APO chain de Windows (overhead invisible que añade 5–15 ms)
- ❌ "Audio Enhancements" del panel de control que destrozan calidad

---

## 2. Arquitectura

```diagram
╭─────────────────────────────────────────────────────────╮
│  App BEF                                                │
│  ┌───────────────────────────────────────────────────┐  │
│  │  bx_audio_engine                                  │  │
│  │   - Voices (audio sources)                        │  │
│  │   - Submixes                                      │  │
│  │   - DSP nodes (reverb, EQ, comp, limiter)         │  │
│  │   - 3D Spatializer (HRTF / Atmos)                 │  │
│  └─────────────────┬─────────────────────────────────┘  │
╰────────────────────┼─────────────────────────────────────╯
                     ▼
╭─────────────────────────────────────────────────────────╮
│  Audio Driver FastOS (Ring 0)                           │
│  - HDA controller driver (Realtek ALC*)                 │
│  - USB Audio Class 2/3 driver                           │
│  - HDMI Audio (vía GSP RTX 3060)                        │
│  - Single mixer point (si hay múltiples apps)           │
│  - Buffer DMA directo, polling MSI-X                    │
╰────────────────────┬─────────────────────────────────────╯
                     ▼
              Hardware Codec / DAC
```

---

## 3. API núcleo

```rust
use barex::audio::*;

// 1. Abre engine en modo exclusivo si solo hay una app activa
let engine = bx_audio::engine(EngineMode::ExclusiveOrShared {
    sample_rate: 48_000,           // o 96000, 192000 si el codec lo soporta
    format: SampleFormat::F32,     // 32-bit float interno siempre
    channels: ChannelLayout::Stereo,
    buffer_frames: 64,             // 64 frames @ 48 kHz = 1.33 ms
})?;

// 2. Crea una voice y carga PCM/Vorbis/Opus
let voice = engine.create_voice(VoiceDesc {
    source: AudioSource::Pcm(samples),
    pitch: 1.0,
    volume: 0.8,
})?;

// 3. Cadena DSP opcional
voice.attach_dsp(&[
    Dsp::Eq(EqDesc::three_band(low_db, mid_db, high_db)),
    Dsp::Reverb(ReverbPreset::Hall),
    Dsp::Limiter { threshold_db: -1.0 },
]);

// 4. 3D positional con HRTF
let spat = engine.create_spatializer(SpatializerDesc::HrtfBuiltin);
spat.set_listener(Vec3::ZERO, Quat::IDENTITY);
spat.attach(&voice, Vec3::new(2.0, 0.0, -3.0));

voice.play();
```

---

## 4. Formatos soportados

| Formato | Decodificador | Hardware accelerated |
|---|---|---|
| PCM 16/24/32, Float32 | Nativo | — |
| Vorbis (`.ogg`) | `lewton` Rust | CPU |
| Opus (`.opus`) | `audiopus` Rust | CPU (muy rápido) |
| FLAC | `claxon` Rust | CPU |
| **WAV / AIFF** | Nativo | — |
| MP3 | `symphonia` Rust | CPU |
| AAC LC / HE | `symphonia` | CPU |
| AC3 / E-AC3 / DTS passthrough | Bypass directo a HDMI | ✅ HDMI Audio GSP |
| Dolby Atmos / DTS:X | Render spatial interno o passthrough | Opcional |

**Sin** Windows Media Audio (WMA), sin RealAudio, sin G.722 telecom.

---

## 5. Modos de latencia

| Modo | Buffer | Latencia round-trip | Caso de uso |
|---|---|---|---|
| `Realtime` | 32 frames @ 48 kHz | **0.67 ms** | Música profesional, VR |
| `LowLatency` | 64 frames | **1.33 ms** | Juegos competitivos |
| `Balanced` (default) | 128 frames | **2.67 ms** | Juegos, multimedia |
| `Power` | 512 frames | **10.67 ms** | Reproducción música/video |

WASAPI Exclusive en Windows 11 oficial baseline: ~3 ms. WASAPI Shared: 10–20 ms. PulseAudio default Linux: 30–50 ms. **BareX Realtime gana incluso a ASIO en codecs USB de buena calidad.**

---

## 6. Spatial Audio nativo

| Tecnología | Estado | Backend |
|---|---|---|
| HRTF stereo (auriculares) | ✅ Built-in | Datasets MIT KEMAR + IRCAM |
| Binaural con head tracking (VR) | ✅ | Quaternion del HMD vía `bx_input` |
| 5.1 / 7.1 surround | ✅ | Channel mapping nativo |
| 7.1.4 con elevación (Atmos-like) | ✅ | Render objeto-basado interno |
| Dolby Atmos certificado | 🟡 | Requiere licencia comercial Dolby |
| DTS:X | 🟡 | Requiere licencia DTS |
| Sony 360 Reality Audio | ❌ | No prioritario |

---

## 7. DSP integrados (cero plugins externos necesarios)

- **EQ** paramétrico (10 bandas) y gráfico (3 bandas).
- **Reverb** algorítmico (Hall, Room, Plate, Spring) + convolución con IR custom.
- **Compresor / Limitador** (con sidechain).
- **Gate** con threshold y lookahead.
- **Pitch shift** PSOLA.
- **Time stretch** (rubberband-rs).
- **Resampler** SoX-quality.
- **Mezclador 3D** con doppler y oclusión.

Todos corren en SIMD AVX2/AVX-512 (el 5600X tiene AVX2 completo).

---

## 8. MIDI

- USB MIDI 2.0 nativo (UMP packets).
- USB MIDI 1.0 compatible.
- API `bx_midi::input/output` con timestamps de hardware.
- **Sin** Windows MIDI Synth de mesa (GS Wavetable).
- Soporte SoundFont 2 y SFZ vía sintetizador interno (`bx_midi_synth`).

---

## 9. Captura (input audio)

- Micrófono integrado, headset, USB, line-in.
- **Echo cancellation** + **noise suppression** opt-in vía RNNoise (red neuronal compacta).
- **Voice activity detection** integrado.
- Loopback de captura del propio output (para streaming/grabación) sin "stereo mix" hacks.

---

## 10. Mezclador del sistema (cuando hay múltiples apps)

Cuando dos apps BEF quieren audio simultáneo, el driver pasa de exclusive a shared con un mezclador **mínimo**:
- Sample rate único negociado al arranque (default 48 kHz).
- Sin SRC por app (las apps deben adaptarse — barato en CPU moderna).
- Volumen per-app + master, sin "ducking" automático.
- Latencia adicional del mezclador: < 0.3 ms.

Esto es radicalmente más simple que el AudioEngine de Windows (que tiene formato float interno, multiples APO, ducking, comunicaciones, etc.).

---

## 11. Compatibilidad para el shim L4

El `BareX_Compat_Shim_Spec.md` provee fake DLLs:
- `xaudio2_9.dll` → wrapper a `bx_audio`.
- `dsound.dll` → mapeo limitado (suficiente para juegos viejos).
- `mmdevapi.dll` + `audioses.dll` (WASAPI) → wrapper.
- `winmm.dll` → solo `waveOut*` mínimo.

---

## 12. Métricas de éxito

| Métrica | Objetivo |
|---|---|
| Latencia round-trip Realtime | < 1.5 ms |
| Jitter de buffer | < 50 µs |
| CPU overhead engine vacío | < 0.1% en un core 5600X |
| CPU overhead 32 voices con 3D + reverb | < 3% |
| Bit-perfect passthrough HDMI Atmos | ✅ |
| Click/pop bajo carga | 0 (vs frecuente en Windows con drivers genéricos) |

---

## 13. Archivos relacionados

- `BareX_API_Spec.md` (gráficos, hermano)
- `BareX_Input_Spec.md` (input para HMD tracking en spatial audio)
- `FastOS_Scheduler_Spec.md` (prioridad realtime para el thread de audio)
- `BMO_Graphics_Layer_Spec.md` (HDMI audio vía GSP)
