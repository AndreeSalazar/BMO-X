# 🏯 05 — UHI Chimes (Universal Home Interface)

> **v1.5.0**: Sonidos de notificación estándar de la **Universal Home
> Interface** (sistema japonés de audio en el hogar y dispositivos).
> Se usan en ascensores, hoteles, y hogares inteligentes.

## ¿Qué es UHI?

La **Universal Home Interface** (ユニバーサルホームインターフェース) es un
estándar japonés para señales auditivas en el hogar. Establecido por
JEITA (Japan Electronics and Information Technology Industries
Association) en 2004.

### Características

- **6 sonidos canónicos** para acciones del sistema
- Cada sonido tiene una **frecuencia fundamental fija**
- Diseñados para ser **reconocibles pero no molestos**
- Usados en ascensores, inodoros inteligentes, lavadoras

## Los 6 sonidos UHI

| # | Acción | Nombre japonés | Frecuencia base |
|---|--------|----------------|-----------------|
| 1 | Inicio | 起動音 (kidōon) | 880 Hz |
| 2 | Operación OK | 操作音 (sōsaon) | 1318 Hz (E6) |
| 3 | Operación completada | 完了音 (kanryōon) | 1568 Hz (G6) |
| 4 | Warning | 注意音 (chūiion) | 880 Hz + 1100 Hz (tritono) |
| 5 | Error | 異常音 (ijōon) | 220 Hz (grave) |
| 6 | Apagado | 終了音 (shūryōon) | 660 Hz |

## Especificación exacta (de JEITA RC-5241)

### 1. 起動音 (Inicio)
- **Frecuencia**: 880 Hz (A5) → 1320 Hz (E6) — sweep ascendente
- **Duración**: 200 ms
- **Forma**: Sine
- **Envelope**: attack 10 ms, decay 200 ms (sin sustain, sin release)

```rust
pub fn uhi_startup() {
    use crate::drivers::audio::dsp::math::dsp_sin;
    const SR: f32 = 48000.0;
    const DUR: f32 = 0.2;
    
    for n in 0..(DUR * SR) as u32 {
        let t = n as f32 / SR;
        // Sweep de 880 a 1320 Hz
        let f = 880.0 + (1320.0 - 880.0) * (t / DUR);
        let envelope = (-(t / DUR) * 5.0).exp();
        let sample = dsp_sin(t * f) * envelope * 0.4;
        emit_sample(sample);
    }
}
```

### 2. 操作音 (Operación OK)
- **Frecuencia**: 1318 Hz (E6)
- **Duración**: 80 ms
- **Forma**: Sine
- **Envelope**: attack 5 ms, decay 80 ms

```rust
pub fn uhi_ok() {
    use crate::drivers::audio::dsp::math::dsp_sin;
    const SR: f32 = 48000.0;
    const DUR: f32 = 0.08;
    
    for n in 0..(DUR * SR) as u32 {
        let t = n as f32 / SR;
        let envelope = (-(t / DUR) * 6.0).exp();
        let sample = dsp_sin(t * 1318.0) * envelope * 0.35;
        emit_sample(sample);
    }
}
```

### 3. 完了音 (Operación completada)
- **Frecuencia**: 1568 Hz (G6)
- **Duración**: 120 ms
- **Forma**: Sine
- **Envelope**: attack 5 ms, decay 120 ms

```rust
pub fn uhi_complete() {
    use crate::drivers::audio::dsp::math::dsp_sin;
    const SR: f32 = 48000.0;
    const DUR: f32 = 0.12;
    
    for n in 0..(DUR * SR) as u32 {
        let t = n as f32 / SR;
        let envelope = (-(t / DUR) * 5.0).exp();
        let sample = dsp_sin(t * 1568.0) * envelope * 0.35;
        emit_sample(sample);
    }
}
```

### 4. 注意音 (Warning) — el más distintivo
- **Frecuencia**: 880 Hz + 1100 Hz (tritono, "diabolus in musica")
- **Duración**: 200 ms
- **Forma**: dos sines simultáneos
- **Patrón**: pulso (50% on, 50% off, 3 veces)

```rust
pub fn uhi_warning() {
    use crate::drivers::audio::dsp::math::dsp_sin;
    const SR: f32 = 48000.0;
    const DUR: f32 = 0.2;
    const PULSES: u32 = 3;
    
    for n in 0..(DUR * SR) as u32 {
        let t = n as f32 / SR;
        // Pulso cuadrado lento
        let pulse_t = (t * 6.0) % 1.0;  // 3 Hz
        let pulse = if pulse_t < 0.5 { 1.0 } else { 0.0 };
        // Tritono: dos sunes
        let sample = (dsp_sin(t * 880.0) + dsp_sin(t * 1100.0)) * 0.5;
        let output = sample * pulse * 0.4;
        emit_sample(output);
    }
}
```

### 5. 異常音 (Error) — grave
- **Frecuencia**: 220 Hz (A3)
- **Duración**: 500 ms
- **Forma**: square wave
- **Patrón**: pulso 2 Hz (1 segundo total, on/off)

```rust
pub fn uhi_error() {
    const SR: f32 = 48000.0;
    const DUR: f32 = 1.0;  // 1 segundo total
    const PULSES: u32 = 2;  // 2 pulsos
    
    for n in 0..(DUR * SR) as u32 {
        let t = n as f32 / SR;
        // Pulso lento
        let pulse_t = (t * 2.0) % 1.0;
        let pulse = if pulse_t < 0.5 { 1.0 } else { 0.0 };
        // Square wave de 220 Hz
        let phase = t * 220.0;
        let sample = if (phase.floor() as i32) % 2 == 0 { 0.4 } else { -0.4 };
        emit_sample(sample * pulse);
    }
}
```

### 6. 終了音 (Apagado)
- **Frecuencia**: 660 Hz (E5)
- **Duración**: 300 ms
- **Forma**: Sine
- **Envelope**: attack 50 ms, decay 300 ms (sin attack rápido)

```rust
pub fn uhi_shutdown() {
    use crate::drivers::audio::dsp::math::dsp_sin;
    const SR: f32 = 48000.0;
    const DUR: f32 = 0.3;
    
    for n in 0..(DUR * SR) as u32 {
        let t = n as f32 / SR;
        let envelope = if t < 0.05 { t / 0.05 } else { (-((t - 0.05) / 0.3)).exp() };
        let sample = dsp_sin(t * 660.0) * envelope * 0.35;
        emit_sample(sample);
    }
}
```

## Tabla de uso en FastOS

| Evento del sistema | Sonido UHI | Cuándo |
|--------------------|------------|--------|
| Kernel arranca | uhi_startup() | Después de Phase 5 |
| Usuario hace click OK | uhi_ok() | En cada acción válida |
| Ventana se abre | uhi_complete() | Welcome → desktop |
| Warning del sistema | uhi_warning() | Memoria baja, etc. |
| Triple fault | uhi_error() | Antes del halt |
| Kernel shutdown | uhi_shutdown() | Reboot planificado |

## Mapeo a tonos MIDI

| UHI | Frecuencia | Nota MIDI |
|-----|-----------|-----------|
| uhi_startup | 880 Hz | A5 (note 69) |
| uhi_startup (final) | 1320 Hz | E6 (note 76) |
| uhi_ok | 1318 Hz | E6 (note 76) |
| uhi_complete | 1568 Hz | G6 (note 79) |
| uhi_shutdown | 660 Hz | E5 (note 64) |
| uhi_error | 220 Hz | A3 (note 45) |

## Por qué UHI funciona

1. **Frecuencias separadas**: cada sonido es distinguible
2. **Duraciones cortas**: no saturan la atención
3. **Tritono en warning**: universalmente reconocido como "alerta"
4. **A5 → E6 en startup**: sweep ascendente = "inicio"
5. **A3 en error**: graves suenan "serios"

## Variantes internacionales

| Cultura | Estándar | Frecuencias |
|---------|----------|-------------|
| 🇯🇵 Japón | JEITA UHI | 880/1318/1568 Hz |
| 🇺🇸 USA | Microsoft Sound | 1000/2000/3000 Hz |
| 🇪🇺 EU | IEC 60118 | 500/1000/1500 Hz |
| 🇨🇳 China | GB/T 31002 | 700/1400/2100 Hz |

## Referencias

- JEITA RC-5241: "Universal Home Interface"
- Nakamura, K. (2004). "Standardization of home audio notifications".
- UHI 仕様書 v2.1 (2004)

---

**Siguiente**: Implementación → `README_implementacion.md`
