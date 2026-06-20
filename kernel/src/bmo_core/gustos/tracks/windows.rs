//! Windows-inspired tracks (sin samples, puro FM synthesis).
//!
//! v1.5.0: recrea el CARÁCTER de los sonidos icónicos de Windows
//! sin copiar samples propietarios.

use crate::bmo_core::gustos::synth::fm::{play, Adsr, FmParams};

/// Windows XP-style startup: sweep ascendente major chord.
pub fn startup() {
    let params = FmParams {
        carrier: 261.63,  // C4
        modulator_ratio: 1.5,
        index: 1.2,
        envelope: Adsr {
            attack: 0.04,
            decay: 0.6,
            sustain: 0.4,
            release: 0.3,
        },
        duration_ms: 800,
        volume: 0.4,
        sweep_to: Some(523.25),  // Sube a C5
    };
    play(params);
}

/// Windows Error sound: tritono descendente.
pub fn error() {
    let params = FmParams {
        carrier: 880.0,  // A5
        modulator_ratio: 1.19,
        index: 2.0,
        envelope: Adsr {
            attack: 0.01,
            decay: 0.3,
            sustain: 0.3,
            release: 0.2,
        },
        duration_ms: 600,
        volume: 0.5,
        sweep_to: Some(440.0),  // Baja a A4
    };
    play(params);
}

/// Windows Critical Stop: square wave grave.
pub fn critical_stop() {
    let params = FmParams {
        carrier: 220.0,  // A3
        modulator_ratio: 1.0,
        index: 0.0,  // Sine puro (no FM, no FM bell)
        envelope: Adsr {
            attack: 0.001,
            decay: 0.0,
            sustain: 1.0,
            release: 0.4,
        },
        duration_ms: 800,
        volume: 0.5,
        sweep_to: None,
    };
    play(params);
}

/// Windows Balloon: bell corto.
pub fn balloon() {
    let params = FmParams {
        carrier: 1318.51,  // E6
        modulator_ratio: 2.5,  // Deciem
        index: 1.5,
        envelope: Adsr {
            attack: 0.005,
            decay: 0.2,
            sustain: 0.0,
            release: 0.05,
        },
        duration_ms: 250,
        volume: 0.3,
        sweep_to: None,
    };
    play(params);
}

/// Windows Exclamation: tritono.
pub fn exclamation() {
    let params = FmParams {
        carrier: 880.0,
        modulator_ratio: 1.25,  // Tritono
        index: 1.0,
        envelope: Adsr {
            attack: 0.001,
            decay: 0.0,
            sustain: 0.5,
            release: 0.15,
        },
        duration_ms: 250,
        volume: 0.4,
        sweep_to: None,
    };
    play(params);
}

/// Windows Logon: sweep A4 → A5.
pub fn logon() {
    let params = FmParams {
        carrier: 440.0,
        modulator_ratio: 1.5,
        index: 1.0,
        envelope: Adsr {
            attack: 0.02,
            decay: 0.4,
            sustain: 0.4,
            release: 0.3,
        },
        duration_ms: 600,
        volume: 0.4,
        sweep_to: Some(880.0),
    };
    play(params);
}

/// Windows Unlock: arpeggio C5 E5 G5 (sweep).
pub fn unlock() {
    // Toca C5, E5, G5 como arpeggio
    let notes = [523.25, 659.26, 783.99];
    for &note in &notes {
        let params = FmParams {
            carrier: note,
            modulator_ratio: 1.5,
            index: 1.0,
            envelope: Adsr {
                attack: 0.005,
                decay: 0.15,
                sustain: 0.0,
                release: 0.05,
            },
            duration_ms: 130,
            volume: 0.4,
            sweep_to: None,
        };
        play(params);
    }
}
