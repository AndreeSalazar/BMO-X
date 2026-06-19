# 🎹 Generador de Música Procedural

> **v1.5.0**: Genera música **algorítmicamente** sin samples. Basado en
> teoría musical + FM synthesis. Produce armonías, melodías, y ritmos.

## Composición

### Estructura musical

```
Intro (4 compases) → Tema A (8) → Puente (4) → Tema B (8) → Outro (4)
```

Cada compás = 4 beats. Tempo = 120 BPM = 2 beats/segundo = 0.5s/beat.

### Generadores

```rust
// kernel/src/gustos/synth/procedural.rs
use crate::gustos::synth::fm::FmParams;

/// Genera una progresión de acordes (I-V-vi-IV en C mayor).
pub fn chord_progression_cmajor() -> [(f32, f32); 4] {
    [
        (261.63, 130.81),  // C major (root + fifth)
        (392.00, 196.00),  // G major
        (220.00, 329.63),  // A minor
        (349.23, 174.61),  // F major
    ]
}

/// Genera una melodía sobre una progresión de acordes.
/// Algoritmo: cada nota es 1 beat, derivada del acorde actual.
pub fn melody() -> [f32; 16] {
    let progression = chord_progression_cmajor();
    let mut melody = [0.0; 16];
    for (i, beat) in melody.iter_mut().enumerate() {
        let chord = &progression[i / 4 % 4];
        // Toca la tónica, tercera, o quinta del acorde
        let note_in_chord = i % 3;
        *beat = match note_in_chord {
            0 => chord.0,        // Tónica
            1 => chord.0 * 1.26, // Tercera mayor
            2 => chord.1,        // Quinta
            _ => 0.0,
        };
    }
    melody
}

/// Genera un bajo que sigue la progresión (root note + octave below).
pub fn bass_line() -> [f32; 16] {
    let progression = chord_progression_cmajor();
    let mut bass = [0.0; 16];
    for (i, beat) in bass.iter_mut().enumerate() {
        let chord = &progression[i / 4 % 4];
        *beat = chord.0 / 2.0;  // Octava abajo
    }
    bass
}

/// Genera un patrón rítmico (kick + snare + hi-hat).
/// Cada bit representa un 1/16th note (4 bits por beat, 16 beats total).
pub fn rhythm() -> [u8; 16] {
    let mut pattern = [0u8; 16];
    // Kick: beats 0, 4, 8, 12
    for &beat in &[0, 4, 8, 12] { pattern[beat] |= 0b001; }
    // Snare: beats 4, 12
    for &beat in &[4, 12] { pattern[beat] |= 0b010; }
    // Hi-hat: cada 1/16
    for i in 0..16 { pattern[i] |= 0b100; }
    pattern
}
```

## Cómo se reproduce

```rust
pub fn play_procedural_track() {
    let melody = melody();
    let bass = bass_line();
    let rhythm = rhythm();
    
    const BPM: f32 = 120.0;
    const BEAT_SEC: f32 = 60.0 / BPM / 2.0;  // 0.25s
    const SAMPLE_RATE: f32 = 48000.0;
    
    for (i, &note) in melody.iter().enumerate() {
        let duration = BEAT_SEC;
        let samples = (duration * SAMPLE_RATE) as u32;
        
        // Melodía: FM bell
        let params = FmParams {
            carrier: note,
            modulator_ratio: 1.19,
            index: 1.5,
            envelope: Envelope {
                attack: 0.005,
                decay: 0.2,
                sustain: 0.0,
                release: 0.05,
            },
            duration_ms: (duration * 1000.0) as u32,
            volume: 0.3,
            sweep_to: None,
        };
        crate::gustos::synth::fm::play(params);
        
        // Bajo: FM con index alto (más percusivo)
        if i % 4 == 0 {
            let bass_params = FmParams {
                carrier: bass[i],
                modulator_ratio: 1.0,
                index: 2.0,
                envelope: Envelope {
                    attack: 0.001,
                    decay: 0.15,
                    sustain: 0.0,
                    release: 0.05,
                },
                duration_ms: (BEAT_SEC * 4.0 * 1000.0) as u32,
                volume: 0.35,
                sweep_to: None,
            };
            crate::gustos::synth::fm::play(bass_params);
        }
    }
}
```

## Tracks procedurales disponibles

| ID | Nombre | Tempo | Compases | Carácter |
|----|--------|-------|----------|----------|
| 011 | startup_music | 120 BPM | 16 | Major, optimista |
| 012 | ambient_pad | 80 BPM | 32 | Slow, meditative |
| 013 | error_loop | 100 BPM | 8 | Minor, tenso |
| 014 | idle_drone | 60 BPM | 64 | Largo, calmado |

## Uso

```rust
// En welcome.rs
fn show_welcome() {
    crate::gustos::tracks::play_011_startup_music();
    // ... render welcome
}
```

## Limitaciones

- v1.5.0: solo el **esqueleto procedural** (marcador en código)
- v1.6.0: reproducción real con PCM buffer
- v1.7.0: variación aleatoria de melodía (mismo "feeling", distinta nota)

## Referencias

- "Algorithmic Composition" — David Cope (2000)
- "The Structure of Musical Harmony" — Howard Hanson (1939)
- "Music Theory" — Michael Miller (2016)
