# 📘 01 — FM Synthesis Basics

> **v1.5.0**: Cómo funciona FM synthesis y por qué es perfecto para FastOS.

## ¿Qué es FM Synthesis?

**FM (Frequency Modulation) Synthesis** es una técnica donde una señal
"moduladora" cambia la frecuencia de una señal "carrier" (portadora).

```
   modulator (Hz)  →  modulates frequency of  →  carrier (Hz)  →  output
       440                  frequency                  880
```

Esto produce **timbres complejos** a partir de simples ondas sinusoidales.

## ¿Por qué FM es ideal para FastOS?

1. **Sin samples**: todo se calcula con `sin()`. No necesitamos archivos
   WAV/MP3 en memoria.
2. **Poca CPU**: una sine + integer arithmetic es suficiente.
3. **Tamaños minúsculos**: un track de 5 segundos cabe en 200 bytes.
4. **Sin royalties**: algoritmo patentado expiró en 1995 (Chowning 1973).

## La fórmula

$$y(t) = A \cdot \sin(2\pi f_c t + I \cdot \sin(2\pi f_m t))$$

Donde:
- $A$ = amplitud (volumen)
- $f_c$ = frecuencia de la portadora (carrier)
- $f_m$ = frecuencia de la moduladora
- $I$ = índice de modulación (qué tan "rica" suena)

## Implementación simple en Rust

```rust
/// Genera un sample FM en f32 a partir de phase + parámetros.
pub fn fm_sample(
    carrier_phase: f32,
    modulator_phase: f32,
    index: f32,        // índice de modulación I
    amplitude: f32,    // A
) -> f32 {
    let modulator = (modulator_phase * std::f32::consts::TAU).sin();
    let carrier = (carrier_phase * std::f32::consts::TAU).sin();
    amplitude * (carrier + index * modulator).sin()
}
```

## Ratios famosos

El carácter del sonido depende del **ratio** $f_m / f_c$:

| Ratio $f_m : f_c$ | Carácter | Uso |
|-------------------|----------|-----|
| 1:1 | Tono puro con vibrato sutil | Sine clásica |
| 2:1 | Octava, sonido "agudo" | Brass |
| 3:2 | Quinta justa, sonido "redondo" | Strings |
| 4:1 | Doble octava | Bells |
| 7:6 | Triste, disonante | Lament |
| π:1 | Inarmónico, percusivo | Bells reales |

## Ejemplo: un "ding" de campana

```rust
// Bell-like FM tone (ratio 4:1, index 2.0, decay 1.5s)
let fc = 880.0;  // A5
let fm = 4.0 * fc;  // Ratio 4:1
let index = 2.0;    // Índice moderado
let amplitude = 0.5;
```

## Buenas prácticas

1. **Empezar con index bajo (0.1–0.5)**: sonido puro
2. **Subir index a 1.0–3.0**: armónicos aparecen
3. **Index > 5.0**: ruidoso, percusivo
4. **Aplicar envelope ADSR** (Attack/Decay/Sustain/Release) a la amplitud
5. **Sumar 2-4 operadores** para timbres más ricos

## Aplicación en FastOS

FastOS usará FM synthesis en `drivers/audio/dsp/fm.rs` (futuro).

Por ahora, podemos emitir un beep en PC speaker usando FM con 1 operador:

```rust
// beep_pc_speaker.rs (en drivers/)
fn play_fm_beep(freq: u32, duration_ms: u32) {
    // 1. Programar PIT channel 2 con la frecuencia
    // 2. Activar speaker
    // 3. Esperar duration_ms
    // 4. Apagar speaker
}
```

## Referencias

- Chowning, J. (1973). "The Synthesis of Complex Audio Spectra by
  Means of Frequency Modulation". *Journal of the Audio Engineering
  Society*. **Patent expirado 1995**.
- Roads, C. (2001). *Microsound*. MIT Press.

---

**Siguiente**: `02_beep_pc_speaker.md` — cómo emitir un beep real.
