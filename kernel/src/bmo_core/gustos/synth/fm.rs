//! FM Synthesis engine for FastOS.
//!
//! v1.5.0: implementa la fórmula FM estándar (Chowning 1973, patent
//! expirado 1995).
//!
//! ## Fórmula
//!
//! y(t) = A * sin(2π fc * t + I * sin(2π fm * t))
//!
//! - `fc` = carrier frequency
//! - `fm` = modulator frequency (fm = fc * ratio)
//! - `I` = modulation index
//! - `A` = amplitude (modulated by envelope)
//!
//! ## Limitaciones v1.5.0
//!
//! - Solo 2 operadores (carrier + 1 modulator)
//! - Sin feedback
//! - Sin envelopes complejos (solo ADSR lineal)
//! - Sin unison/detune

use crate::dev::audio::dsp_sin;

/// Parámetros de un track FM.
#[derive(Debug, Clone, Copy)]
pub struct FmParams {
    /// Frecuencia de la portadora (Hz).
    pub carrier: f32,
    /// Ratio entre modulador y carrier (e.g. 1.0, 1.5, 2.0).
    pub modulator_ratio: f32,
    /// Índice de modulación (qué tan "rico" suena).
    pub index: f32,
    /// Envolvente ADSR.
    pub envelope: Adsr,
    /// Duración total en milisegundos.
    pub duration_ms: u32,
    /// Volumen (0.0–1.0).
    pub volume: f32,
    /// Si se especifica, la portadora hace sweep desde `carrier` hasta
    /// este valor durante la duración del track.
    pub sweep_to: Option<f32>,
}

/// ADSR (Attack, Decay, Sustain, Release) en segundos.
#[derive(Debug, Clone, Copy)]
pub struct Adsr {
    pub attack: f32,
    pub decay: f32,
    /// Nivel de sustain (0.0–1.0).
    pub sustain: f32,
    pub release: f32,
}

impl Adsr {
    /// Aplica el ADSR en tiempo `t` (segundos) y retorna amplitud (0.0–1.0).
    pub fn amplitude_at(&self, t: f32, duration: f32) -> f32 {
        let release_start = duration - self.release;
        if t < self.attack {
            // Attack
            t / self.attack
        } else if t < self.attack + self.decay {
            // Decay: interpolamos de 1.0 a sustain
            let dt = (t - self.attack) / self.decay;
            1.0 - dt * (1.0 - self.sustain)
        } else if t < release_start {
            // Sustain
            self.sustain
        } else {
            // Release
            let dt = (t - release_start) / self.release;
            self.sustain * (1.0 - dt)
        }
    }
}

/// Genera un sample FM en `t` con los parámetros dados.
/// El sample es un f32 normalizado a [-1.0, 1.0].
pub fn fm_sample(params: &FmParams, t: f32, duration: f32) -> f32 {
    // Sweep de carrier si está habilitado
    let fc = if let Some(sweep_to) = params.sweep_to {
        let progress = t / duration;
        params.carrier + (sweep_to - params.carrier) * progress
    } else {
        params.carrier
    };
    let fm = fc * params.modulator_ratio;

    // Modulator
    let modulator = dsp_sin(t * fm * core::f32::consts::TAU);

    // Carrier con modulación de fase
    let phase = t * fc * core::f32::consts::TAU + params.index * modulator;
    let carrier = dsp_sin(phase);

    // Aplicar envolvente
    let envelope = params.envelope.amplitude_at(t, duration);

    carrier * envelope * params.volume
}

/// Reproduce un track FM usando el destino PCM actual.
/// El destino se setea con `pcm::set_destination`.
pub fn play(params: FmParams) {
    use crate::bmo_core::gustos::synth::pcm;
    const SAMPLE_RATE: f32 = 48000.0;
    let duration = params.duration_ms as f32 / 1000.0;
    let total_samples = (duration * SAMPLE_RATE) as u32;

    for n in 0..total_samples {
        let t = n as f32 / SAMPLE_RATE;
        let sample = fm_sample(&params, t, duration);
        pcm::emit(sample);
    }
}
