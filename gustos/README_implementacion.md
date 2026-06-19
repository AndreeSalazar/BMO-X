# 🛠️ README de Implementación

> Cómo integrar `gustos/` en BMO/FastOS. **v1.5.0**.

## Arquitectura

```
kernel/src/gustos/                 (NUEVO en v1.5.0)
├── mod.rs                          # public API
├── synth/
│   ├── mod.rs
│   ├── fm.rs                       # FM synthesis core
│   ├── envelope.rs                 # ADSR
│   └── pcm.rs                      # emit_to_pcm()
└── tracks/
    ├── mod.rs
    ├── track_001_hola_mundo.rs
    ├── track_002_uhi_boot.rs
    └── ...

gustos/                             (este directorio, raíz)
├── README.md                       # Overview
├── 01_fm_synth_basics.md           # Teoría FM
├── 02_beep_pc_speaker.md           # Hardware speaker
├── 03_icosahedral_resonance.md     # Por qué suenan bien las campanas
├── 04_pleasant_chimes.md           # Reglas de oro
├── 05_uhi_chimes.md                # Estándar japonés
├── README_implementacion.md        # este archivo
└── tracks/                         # Tracks específicos
```

## API pública

```rust
// En kernel/src/gustos/mod.rs
pub mod synth;
pub mod tracks;

/// Reproduce un track por ID.
pub fn play(track_id: TrackId);

/// Reproduce un chime UHI.
pub fn play_uhi(chime: UhiChime);
```

## Integración con drivers

Para que los tracks se **escuchen** realmente, necesitamos:

1. **PC speaker** (ya funciona) — `drivers/audio/beep.rs` (futuro)
2. **USB audio** (driver listo, falta integración) — `drivers/usb/audio/`
3. **DSP chain** (ya existe) — `drivers/audio/dsp/`

### Paso 1: emitir samples a un buffer PCM

```rust
// En kernel/src/gustos/synth/pcm.rs
static mut PCM_BUFFER: [i16; 48000] = [0; 48000];
static mut PCM_LEN: usize = 0;

pub fn emit_to_pcm(sample: f32) {
    unsafe {
        let i = PCM_LEN;
        if i < PCM_BUFFER.len() {
            PCM_BUFFER[i] = (sample * i16::MAX as f32) as i16;
            PCM_LEN = i + 1;
        }
    }
}

pub fn drain_pcm() -> &'static [i16] {
    unsafe {
        let slice = &PCM_BUFFER[..PCM_LEN];
        PCM_LEN = 0;
        slice
    }
}
```

### Paso 2: conectar al USB audio driver

```rust
// En drivers/usb/audio/mod.rs
pub fn play_pcm(samples: &[i16]) {
    // 1. Configurar endpoint de salida
    // 2. DMA los samples al ring buffer
    // 3. Iniciar transmisión
}
```

### Paso 3: hooks en welcome screen

```rust
// En kernel/src/desktop/welcome.rs
fn show_welcome() {
    crate::gustos::play(crate::gustos::TrackId::HolaMundo);
    // ... render welcome
}
```

## Roadmap

| Versión | Feature |
|---------|---------|
| v1.5.0 | PCM buffer + USB audio integration |
| v1.5.0 | track_001, track_002 implementados |
| v1.6.0 | uhi_startup/ok/complete/error/shutdown |
| v1.6.0 | Reverb + limiter en DSP chain |
| v1.7.0 | Sound effects context-aware (qué suena depende de qué se hizo) |

## Testing sin hardware

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn track_001_produces_samples() {
        crate::gustos::synth::pcm::clear();
        crate::gustos::tracks::play_001_hola_mundo();
        let samples = crate::gustos::synth::pcm::drain_pcm();
        assert!(!samples.is_empty());
        assert!(samples.len() > 1000);  // ~25 ms mínimo
    }
}
```

## Limitaciones actuales

- **PC speaker**: solo square wave, freq fija
- **USB audio**: driver listo pero no conectado a `gustos::synth::pcm::emit_to_pcm`
- **Sin reverb espacial**: solo el dry signal

## Referencias

- Ver `gustos/01-05_*.md` para teoría detallada
- Ver `gustos/tracks/*/synth.md` para specs de tracks
