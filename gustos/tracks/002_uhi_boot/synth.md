# Track 002 — UHI Boot (起動音)

> Sonido de inicio basado en el estándar **Universal Home Interface**
> (JEITA RC-5241). Suena cuando el kernel alcanza Phase 5 (Desktop).

## Especificación

| Parámetro | Valor |
|-----------|-------|
| **ID** | 002_uhi_boot |
| **Tipo** | Sweep ascendente (sine) |
| **Frecuencia inicial** | 880 Hz (A5) |
| **Frecuencia final** | 1320 Hz (E6) |
| **Duración** | 200 ms |
| **Sample rate** | 48000 Hz |
| **Volumen** | 0.4 |
| **Forma** | Sine |
| **Envolvente** | exponential decay (tau=0.2) |

## Fórmula

$$f(t) = 880 + 440 \cdot \frac{t}{0.2}$$
$$y(t) = 0.4 \cdot e^{-t/0.2} \cdot \sin(2\pi \int_0^t f(\tau) d\tau)$$

Aproximación:$$\int_0^t f(\tau)d\tau \approx 880t + 220 \cdot \frac{t^2}{0.2}$$

## Implementación

```rust
// gustos/src/tracks/track_002.rs
use crate::drivers::audio::dsp::math::dsp_sin;

pub fn play() {
    const SR: u32 = 48000;
    const DURATION_SEC: f32 = 0.2;
    const F_START: f32 = 880.0;
    const F_END: f32 = 1320.0;
    const VOLUME: f32 = 0.4;
    const DECAY: f32 = 0.2;
    
    let samples = (SR as f32 * DURATION_SEC) as u32;
    let sweep_rate = (F_END - F_START) / DURATION_SEC;
    
    let mut phase = 0.0;
    for n in 0..samples {
        let t = n as f32 / SR as f32;
        let f = F_START + sweep_rate * t;
        phase += f / SR as f32;
        let envelope = (-(t / DECAY) * 5.0).exp();
        let sample = dsp_sin(phase * core::f32::consts::TAU) * envelope * VOLUME;
        emit_to_pcm(sample);
    }
}
```

## Origen

Estándar **JEITA Universal Home Interface** (Universal Design for
Home Audio Notifications). Establecido en Japón en 2004.

Ver `gustos/05_uhi_chimes.md` para más detalles.

## Cuándo se reproduce

- Después de Phase 5 (Desktop)
- Antes de mostrar la welcome screen
- Una sola vez por boot

## Variantes

- **002b**: 完了音 (Complete) — 1568 Hz sine, 120 ms
- **002c**: 操作音 (OK) — 1318 Hz sine, 80 ms

## Créditos

- JEITA Universal Home Interface RC-5241
- Chowning FM synthesis patent (expired 1995)

## Licencia

- Documentación: CC-BY-SA 4.0
- Código: MIT
