# 🔮 03 — Resonancia Icosaédrica y Tonos Ricos

> **v1.5.0**: Por qué algunos tonos suenan "ricos" y otros "planos".
> Fundamento teórico para diseñar chimes agradables.

## ¿Por qué las campanas reales suenan tan bien?

Una campana física no vibra en una sola frecuencia. Vibra en
**múltiples modos parciales** simultáneamente. La razón por la que
suena "rica" es que esos parciales siguen una proporción especial.

## El "modo icosaédrico"

Geómetría icosaédrica (20 caras triangulares) genera ratios específicos:

| Parcial | Ratio | Carácter |
|---------|-------|----------|
| Hum (fundamental) | 1.0 | Tono base |
| Tierce (tercera menor) | 1.19 | "sweetness" |
| Quint | 1.50 | Brillo |
| Nominal | 2.0 | Octava |
| Deciem | 2.50 | "strike" |
| Duodecim | 3.0 | Quinta + octava |
| Double octave | 4.0 | Pico agudo |

Estos ratios **no son armónicos** (no son 1, 2, 3, 4...) sino que
están **esparcidos en el espectro**. Eso es lo que crea el sonido
"campana" en lugar de "flauta".

## Aplicación a FM synthesis

En FM, para emular un modo icosaédrico:

```rust
// Bell-like FM tone con 3 operadores
let fc = 880.0;                    // Carrier: A5
let fm1 = fc * 1.19;                // Tierce
let fm2 = fc * 1.50;                // Quint
let fm3 = fc * 2.50;                // Deciem

let index = 1.8;                    // Suave
// Sumar los 3 partials con amplitudes descendentes
let sample = (fc * t).sin()
         + 0.6 * (fm1 * t).sin()
         + 0.4 * (fm2 * t).sin()
         + 0.2 * (fm3 * t).sin();
```

## El "strike" de las campanas

Un buen chime tiene:
- **Ataque rápido** (5-15 ms): un "click" inicial
- **Decay largo** (1-3 s): la cola del sonido
- **Parciales inarmónicos**: lo que distingue una campana de un beep

```rust
// Envolvente ADSR para un chime
fn chime_envelope(t: f32) -> f32 {
    let attack = 0.005;   // 5 ms
    let decay = 1.8;      // 1.8 s
    if t < attack {
        t / attack
    } else {
        (-((t - attack) / decay)).exp()
    }
}
```

## Por qué algunos tonos "duelen"

Disonancia = batidos entre partials cercanos.

- **Cents de diferencia** entre partials < 50: batidos audibles (disonante)
- **Cents de diferencia** 50-200: tensión musical
- **Cents de diferencia** > 200: consonante

Los partials icosaédricos están **lejos** entre sí, por eso suenan
"consonantes" incluso siendo inarmónicos.

## Diseño de un chime "agradable"

```rust
// Spec para un chime universal
struct ChimeSpec {
    fundamental: f32,    // Frecuencia base (e.g. 440 Hz)
    partials: [f32; 6],  // Ratios icosaédricos
    amplitudes: [f32; 6], // Pesos
    decay_sec: f32,      // Tiempo de cola
    attack_ms: f32,      // Tiempo de ataque
}

const PLEASANT_CHIME: ChimeSpec = ChimeSpec {
    fundamental: 880.0,
    partials:    [1.0, 1.19, 1.50, 2.0, 2.50, 3.0],
    amplitudes:  [1.0, 0.6, 0.4, 0.3, 0.2, 0.1],
    decay_sec:   1.8,
    attack_ms:   8.0,
};
```

## Por qué importa para FastOS

Cuando un usuario hace click en "Run" en la welcome screen, queremos
que suene algo que **diga "todo bien"** sin ser molesto.

- **Beep plano** (200 Hz, 200 ms): suena a error
- **FM bell suave** (880 Hz, ratio 1.19, decay 1s): suena a "OK"
- **Major chord** (C+E+G, 200 ms): suena a "celebration"

## Tabla de "carácter" por tipo de sonido

| Tipo | Frecuencias | Carácter | Uso |
|------|-------------|----------|-----|
| Sine puro | Single freq | Limpio, "digital" | Notificación tech |
| Major chord | 1.0, 1.26, 1.50 | "feliz" | Success |
| Minor chord | 1.0, 1.19, 1.50 | "serio" | Warning |
| Bell (icosaédrico) | 1.0, 1.19, 1.5, 2.0, 2.5 | "rico" | Confirmación |
| Square wave | Single freq | "agresivo" | Error |
| FM con index alto | Inarmónico | "alien" | Special |

## Implementación práctica

```rust
use crate::drivers::audio::dsp::math::dsp_sin;

pub fn bell_sample(t: f32, fundamental: f32) -> f32 {
    let partials = [1.0, 1.19, 1.50, 2.0, 2.50, 3.0];
    let amplitudes = [1.0, 0.6, 0.4, 0.3, 0.2, 0.1];
    
    let mut sum = 0.0;
    for (&ratio, &amp) in partials.iter().zip(amplitudes.iter()) {
        let freq = fundamental * ratio;
        sum += amp * dsp_sin(t * freq);
    }
    sum * 0.3  // Normalizar
}
```

## Referencias

- Rossing, T. (1984). "The Acoustics of Bells". *American Scientist*.
- Fletcher, N. & Rossing, T. (1998). *The Physics of Musical Instruments*.
- Paté, A. (1995). "Carillon" — ICOSAhedral bell synthesis.

---

**Siguiente**: `04_pleasant_chimes.md` — reglas prácticas.
