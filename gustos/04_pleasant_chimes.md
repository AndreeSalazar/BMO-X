# 🎼 04 — Reglas de Oro para Chimes Agradables

> **v1.5.0**: Cómo diseñar sonidos que el usuario quiera escuchar,
> no que tenga que tolerar.

## Las 5 reglas

### 1. **Ataca rápido, decae lento**

```
amplitud
  │
1 ├─█
  │  ╲
  │   ╲
  │    ╲
  │     ╲╲
  │       ╲╲╲╲╲╲╲╲╲
0 └────────────────────► tiempo
  ▲    ▲              ▲
attack decay        release
(5ms) (1-2s)
```

Un ataque corto (< 15 ms) da "presencia". Un decay largo (1-2 s) da
"cuerpo". Esto imita instrumentos acústicos reales.

### 2. **Parciales inarmónicos pero consonantes**

Usa ratios icosaédricos (`01_fm_synth_basics.md`):
```
1.0, 1.19, 1.50, 2.0, 2.50, 3.0
```

Estos suenan "ricos" sin ser disonantes.

### 3. **Frecuencia entre 500–2000 Hz**

| Frecuencia | Percepción |
|------------|------------|
| < 200 Hz | "sub", rumble, opaco |
| 200–500 Hz | "warm", percusivo |
| **500–2000 Hz** | **"present", claro, audible** |
| 2000–5000 Hz | "thin", brillante |
| > 5000 Hz | "piercing", agresivo |

Para chimes del sistema, mantente en **500–2000 Hz**.

### 4. **Amplitud pico moderado**

```rust
// BIEN: pico 0.5, decay suave
let amp = 0.5 * (-t / 1.5_f32).exp();

// MAL: pico 1.0, decay abrupto
let amp = if t < 0.1 { 1.0 } else { 0.0 };
```

Saturación o volumen alto causa **fatiga auditiva**.

### 5. **Silencio entre sonidos**

Después de un chime, deja al menos **200-500 ms de silencio**.
Sin gap, los sonidos se "amontonan" y se vuelven molestos.

## Plantillas listas para usar

### Chime "OK" (operación exitosa)

```rust
pub struct Chime {
    pub fundamental: f32,
    pub decay: f32,
    pub attack: f32,
    pub volume: f32,
}

pub const CHIME_OK: Chime = Chime {
    fundamental: 880.0,  // A5
    decay: 0.8,
    attack: 0.005,
    volume: 0.4,
};
```

### Chime "Error"

```rust
pub const CHIME_ERROR: Chime = Chime {
    fundamental: 220.0,  // A3 (grave, "malo")
    decay: 0.4,
    attack: 0.001,
    volume: 0.5,
};
```

### Chime "Boot"

```rust
pub const CHIME_BOOT: Chime = Chime {
    fundamental: 440.0,  // A4
    decay: 1.5,
    attack: 0.015,
    volume: 0.45,
};
```

## Implementación canónica (FM bell)

```rust
pub fn play_chime(chime: &Chime) {
    use crate::drivers::audio::dsp::math::dsp_sin;
    const SAMPLE_RATE: u32 = 48000;
    
    let total_samples = (chime.decay * SAMPLE_RATE as f32) as u32;
    let partials = [1.0, 1.19, 1.50, 2.0, 2.50, 3.0];
    let amplitudes = [1.0, 0.6, 0.4, 0.3, 0.2, 0.1];
    
    for n in 0..total_samples {
        let t = n as f32 / SAMPLE_RATE as f32;
        let envelope = if t < chime.attack {
            t / chime.attack
        } else {
            (-(t - chime.attack) / chime.decay).exp()
        };
        
        let mut sample = 0.0;
        for (&ratio, &amp) in partials.iter().zip(amplitudes.iter()) {
            sample += amp * dsp_sin(t * chime.fundamental * ratio);
        }
        
        let output = sample * envelope * chime.volume;
        emit_sample(output);
    }
}
```

## Tabla de decisión: ¿qué chime usar?

| Situación | Frecuencia | Decay | Carácter |
|-----------|------------|-------|----------|
| Bienvenida del kernel | 440-880 Hz | 1.0–1.5 s | Major, "abierto" |
| Operación exitosa | 880-1320 Hz | 0.5–0.8 s | Bell corto |
| Warning | 330-440 Hz | 0.3–0.5 s | Minor, "alerta" |
| Error | 200-300 Hz | 0.4–0.6 s | Grave, percusivo |
| Click en UI | 1000-1500 Hz | 0.05–0.1 s | Muy corto, "tap" |
| Notificación | 660-880 Hz | 0.6–0.9 s | Suave, "ping" |
| Confirmación | 523-784-1047 Hz | 0.8 s | Major arpeggio |

## Errores comunes a evitar

### ❌ Decay demasiado corto (sonido "truncado")
```rust
let decay = 0.05;  // 50 ms — suena a "plop", no a campana
```

### ❌ Demasiados parciales
```rust
// Demasiado "ruidoso" para una notificación
let partials = [1.0, 1.05, 1.10, 1.15, 1.20, 1.25, 1.30, 1.35, 1.40];
```

### ❌ Volumen saturado
```rust
let output = sample * 1.0;  // Saturado — clipping en USB audio
```

### ❌ Frecuencia demasiado alta
```rust
let fundamental = 5000.0;  // Penetrante, molesto
```

## Referencias

- Howard, D. & Angus, J. (2017). *Acoustics and Psychoacoustics*.
- Chowning, J. (1989). "Frequency Modulation Synthesis of the
  Singing Voice".

---

**Siguiente**: `05_uhi_chimes.md` — Universal Home Interface (estándar japonés).
