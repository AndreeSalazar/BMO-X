# Track 001 — Hola Mundo

> Primer sonido del sistema. Suena al iniciar FastOS.

## Especificación

| Parámetro | Valor |
|-----------|-------|
| **ID** | 001_hola_mundo |
| **Tipo** | FM bell (2 operadores) |
| **Frecuencia base** | 880 Hz (A5) |
| **Ratio modulador** | 1.19 (tierce) |
| **Índice FM** | 1.5 |
| **Duración** | 400 ms |
| **Sample rate** | 48000 Hz |
| **Volumen** | 0.4 |
| **Envelope** | attack 5 ms, decay 400 ms |

## Fórmula

$$y(t) = 0.4 \cdot e^{-t/0.4} \cdot \sin(2\pi \cdot 880t + 1.5 \cdot \sin(2\pi \cdot 1047t))$$

Donde $1047 = 880 \times 1.19$.

## Implementación

```rust
// gustos/src/tracks/track_001.rs
use crate::drivers::audio::dsp::math::dsp_sin;

pub fn play() {
    const SR: u32 = 48000;
    const DURATION_MS: u32 = 400;
    const FC: f32 = 880.0;
    const FM: f32 = 1047.0;  // 880 * 1.19
    const INDEX: f32 = 1.5;
    const VOLUME: f32 = 0.4;
    const DECAY: f32 = 0.4;
    
    let samples = (SR * DURATION_MS) / 1000;
    
    for n in 0..samples {
        let t = n as f32 / SR as f32;
        let envelope = (-(t / DECAY)).exp();
        let modulator = dsp_sin(t * FM * core::f32::consts::TAU);
        let carrier = dsp_sin(
            t * FC * core::f32::consts::TAU
            + INDEX * modulator
        );
        let output = carrier * envelope * VOLUME;
        emit_to_pcm(output);
    }
}
```

## Test de sonido

Para verificar que el código es correcto sin hardware:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn hola_mundo_doesnt_panic() {
        // Solo verifica que la función se ejecuta sin panics
        // (no podemos probar sonido real en unit test)
        play();
    }
}
```

## Variantes

- **001b**: Variante con 3 parciales (más "rica")
- **001c**: Variante descendente (A5 → E5, 400ms)

## Créditos

- Basado en el patrón FM bell de John Chowning (1973)
- Adaptado para PC speaker de FastOS

## Licencia

- Documentación: CC-BY-SA 4.0
- Código: MIT
