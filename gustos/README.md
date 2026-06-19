# 🎵 gustos/ — Catálogo de audio para FastOS

> **v1.5.0**: Catálogo personal de sonidos, melodías, y tonos que pueden
> reproducirse en FastOS. Cada track incluye la especificación matemática
> (FM synthesis, formas de onda, frecuencias) y un snippet en Rust para
> integrarlo en el kernel.

## 🎯 ¿Qué es esto?

FastOS tiene **audio nativo** vía:
- **Beep PC speaker** (1 canal, ~1000 Hz máximo) — ya funcional
- **USB audio** (UAC2, 7.1 surround) — driver listo, esperando `net::audio` integrado
- **DSP chain** (`drivers/audio/dsp/*`) — EQ, limiter, compressor, reverb

Pero **falta el contenido**: ¿qué suena cuando arrancas? ¿qué toca al
abrir una ventana? **Esta carpeta define eso.**

## 📂 Estructura

```
gustos/
├── README.md                  ← este archivo
├── 00_index_guia_rapida.md    ← qué track usar y cuándo
├── 01_fm_synth_basics.md      ← cómo funciona FM synthesis
├── 02_beep_pc_speaker.md      ← beep PC speaker (lo más simple)
├── 03_icosahedral_resonance.md← teoría de tonos "ricos"
├── 04_pleasant_chimes.md      ← cómo diseñar campanadas agradables
├── 05_uhi_chimes.md           ← Universal Home Interface (japonés)
├── README_implementacion.md   ← cómo meter un track al kernel
└── tracks/                    ← tracks específicos
    ├── 001_hola_mundo/        ← primer sonido
    ├── 002_chime_ok/          ← sonido de "todo bien"
    ├── 003_chime_error/       ← sonido de "algo falló"
    ├── 004_boot_startup/      ← intro del kernel
    └── 005_phase_transition/  ← entre fases del boot
```

## 🔊 Hardware de salida

| Dispositivo | Estado | Calidad |
|-------------|--------|---------|
| PC speaker | ✅ beep funcional | 1-bit / mono / 0-1kHz |
| USB audio (UAC2) | ✅ driver listo, falta integración | 24-bit / 7.1 / 48kHz |
| HDMI audio | ❌ pendiente (sin GPU) | — |

Para un sistema **sin GPU** (FastOS), USB audio es la opción.

## 🎼 Tracks disponibles

| ID | Nombre | Cuándo suena | Sintaxis |
|----|--------|--------------|----------|
| 001 | hola_mundo | Boot inicial | Sine 440Hz + 880Hz, 200ms |
| 002 | chime_ok | Operación exitosa | FM bell-like |
| 003 | chime_error | Fallo detectado | Square descendente |
| 004 | boot_startup | Inicio del kernel | Major chord swell |
| 005 | phase_transition | Entre fases del boot | Short blip |

## 🚀 Quick start

Si solo quieres **oír algo** desde la welcome screen:

```rust
use crate::gustos::tracks::track_001_hola_mundo;

fn play_demo() {
    track_001_hola_mundo::play();
}
```

## 📚 Documentos de fondo

1. **`01_fm_synth_basics.md`** — qué es FM synthesis
2. **`02_beep_pc_speaker.md`** — el speaker más simple
3. **`03_icosahedral_resonance.md`** — por qué algunos tonos suenan "ricos"
4. **`04_pleasant_chimes.md`** — reglas de oro para sonidos agradables
5. **`05_uhi_chimes.md`** — Universal Home Interface (estándar japonés)

## 🛠️ Cómo añadir un track

1. Crear carpeta `tracks/00X_nombre/`
2. Escribir `synth.md` con especificación matemática
3. Escribir `source.txt` con créditos/URL de origen
4. Implementar `rust_snippet.rs` que reproduce el track
5. Registrar en `tracks/README.md`
