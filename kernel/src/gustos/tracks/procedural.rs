//! Música procedural (álgebraica, sin samples).
//!
//! v1.5.0: genera melodías y progresiones de acordes con teoría musical
//! básica. Ver `gustos/tracks/procedural.md` para teoría.

use crate::gustos::synth::fm::{play, Adsr, FmParams};

/// Reproduce una melodía sobre la progresión I-V-vi-IV en C mayor.
pub fn cmajor_progression() {
    let chord_progression = [
        (261.63, 130.81),  // C major
        (392.00, 196.00),  // G major
        (220.00, 329.63),  // A minor
        (349.23, 174.61),  // F major
    ];
    let melody_notes = [0, 2, 4, 2, 0, 4, 5, 7];  // Indices en la escala

    for (i, &note_idx) in melody_notes.iter().enumerate() {
        let chord = chord_progression[i / 2 % 4];
        let base = chord.0;
        // Octavas: 0, 1, 2 según índice
        let octave_mult = match (note_idx as i32) / 3 {
            0 => 1.0,
            1 => 2.0,
            2 => 4.0,
            _ => 1.0,
        };
        let freq = base * octave_mult;

        let params = FmParams {
            carrier: freq,
            modulator_ratio: 1.5,
            index: 1.0,
            envelope: Adsr {
                attack: 0.005,
                decay: 0.2,
                sustain: 0.0,
                release: 0.05,
            },
            duration_ms: 200,
            volume: 0.3,
            sweep_to: None,
        };
        play(params);
    }
}

/// Bajo simple que sigue la tónica de cada acorde.
pub fn simple_bass() {
    let roots = [130.81, 196.00, 110.00, 174.61];  // C2, G2, A2, F2

    for (_i, &root) in roots.iter().enumerate() {
        let params = FmParams {
            carrier: root,
            modulator_ratio: 1.0,
            index: 1.5,
            envelope: Adsr {
                attack: 0.001,
                decay: 0.15,
                sustain: 0.0,
                release: 0.05,
            },
            duration_ms: 800,
            volume: 0.35,
            sweep_to: None,
        };
        play(params);
    }
}

/// Track procedural completo: melodía + bajo.
pub fn play_procedural_track() {
    cmajor_progression();
    simple_bass();
}
