# 🪟 Windows-Inspired Sounds for FastOS

> **v1.5.0**: Recrea los sonidos icónicos de Windows XP/7/10 usando
> FM synthesis puro. No usa samples — todo se genera matemáticamente.

## Sonidos icónicos recreados

| ID | Inspirado en | Cuándo suena | Carácter |
|----|--------------|--------------|----------|
| 003 | Windows XP Startup | Kernel reaches Phase 5 | Major chord swell ascendente |
| 004 | Windows XP Shutdown | Usuario elige "Shutdown" | Major chord descendente |
| 005 | Windows Error | Triple fault o kernel panic | Tritono descendente |
| 006 | Windows Critical Stop | #DF (double fault) | Square wave grave |
| 007 | Windows Balloon | Notificación | Bell corto ascendente |
| 008 | Windows Exclamation | Warning | Tritono "diabolus" |
| 009 | Windows Logon | Welcome screen | Sweep A4→E5→A5 |
| 010 | Windows Unlock | "Run" command | Major arpeggio |

## Filosofía de recreación

**No copiamos** los sonidos. **Capturamos el carácter**:

- **Windows XP Startup**: "asciende" — sweep de grave a agudo
- **Windows Error**: "desciende y se quiebra" — tritono
- **Windows Balloon**: "amigable" — bell corto
- **Windows Critical Stop**: "alerta máxima" — square grave

## Implementación

```rust
// kernel/src/gustos/synth/windows.rs

use crate::gustos::synth::fm::{FmVoice, FmParams};
use crate::gustos::synth::envelope::{Adsr, Envelope};

/// Crea los parámetros FM para cada sonido Windows-inspired.
pub mod win {
    use super::*;

    /// Windows XP-style startup
    /// Carácter: Major chord que asciende (C4+E4+G4 → C5+E5+G5)
    pub fn startup() -> FmParams {
        FmParams {
            carrier: 261.63,  // C4
            modulator_ratio: 1.5,  // Quint
            index: 1.2,
            envelope: Envelope {
                attack: 0.04,
                decay: 0.8,
                sustain: 0.5,
                release: 0.3,
            },
            duration_ms: 800,
            volume: 0.4,
            sweep_to: Some(523.25),  // Sube a C5
        }
    }

    /// Windows Error sound (descending minor)
    pub fn error() -> FmParams {
        FmParams {
            carrier: 440.0,  // A4
            modulator_ratio: 1.19,  // Tierce
            index: 2.0,
            envelope: Envelope {
                attack: 0.01,
                decay: 0.4,
                sustain: 0.3,
                release: 0.2,
            },
            duration_ms: 600,
            volume: 0.5,
            sweep_to: Some(220.0),  // Baja a A3
        }
    }

    /// Windows Critical Stop (square grave)
    pub fn critical_stop() -> FmParams {
        FmParams {
            carrier: 110.0,  // A2 (muy grave)
            modulator_ratio: 1.0,
            index: 0.0,  // Sine puro (sin FM)
            envelope: Envelope {
                attack: 0.001,
                decay: 0.0,
                sustain: 1.0,
                release: 0.3,
            },
            duration_ms: 800,
            volume: 0.6,
            sweep_to: None,
        }
    }

    /// Windows Balloon (notification bell)
    pub fn balloon() -> FmParams {
        FmParams {
            carrier: 1318.51,  // E6
            modulator_ratio: 2.5,  // Deciem
            index: 1.5,
            envelope: Envelope {
                attack: 0.005,
                decay: 0.2,
                sustain: 0.0,
                release: 0.05,
            },
            duration_ms: 250,
            volume: 0.35,
            sweep_to: None,
        }
    }

    /// Windows Exclamation (warning tritono)
    pub fn exclamation() -> FmParams {
        FmParams {
            carrier: 880.0,  // A5
            modulator_ratio: 1.25,  // Tritono (5:4)
            index: 1.0,
            envelope: Envelope {
                attack: 0.001,
                decay: 0.0,
                sustain: 0.5,
                release: 0.1,
            },
            duration_ms: 200,
            volume: 0.5,
            sweep_to: None,
        }
    }

    /// Windows Logon (sweep A4 → E5 → A5)
    pub fn logon() -> FmParams {
        FmParams {
            carrier: 440.0,  // A4
            modulator_ratio: 1.5,
            index: 1.0,
            envelope: Envelope {
                attack: 0.02,
                decay: 0.5,
                sustain: 0.4,
                release: 0.3,
            },
            duration_ms: 600,
            volume: 0.4,
            sweep_to: Some(880.0),  // Sube a A5
        }
    }

    /// Windows Unlock (major arpeggio C5 E5 G5)
    pub fn unlock() -> FmParams {
        FmParams {
            carrier: 523.25,  // C5
            modulator_ratio: 1.5,
            index: 1.0,
            envelope: Envelope {
                attack: 0.01,
                decay: 0.3,
                sustain: 0.0,
                release: 0.05,
            },
            duration_ms: 400,
            volume: 0.45,
            sweep_to: Some(783.99),  // G5
        }
    }
}
```

## Tabla de mapeo a eventos FastOS

| Evento FastOS | Track Windows |
|---------------|---------------|
| Kernel arranca | win::startup() |
| Welcome screen aparece | win::logon() |
| Usuario presiona "Run" | win::unlock() |
| Operación exitosa | win::balloon() |
| Warning del sistema | win::exclamation() |
| #PF (page fault) | win::error() |
| #DF (double fault) | win::critical_stop() |
| Shutdown planificado | (futuro) win::shutdown() |

## Créditos

- "Windows XP Startup" — Brian Eno (1986), usado por Microsoft
  desde 1995 hasta Windows 8. Recreado con FM synthesis puro.
- "Windows Error/Critical Stop" — Microsoft (1992). Recreación por
  carácter (tritono descendente + square grave).
- Todos los demás son interpretaciones libres inspiradas en el carácter
  auditivo de Windows, no en samples propietarios.

## Licencia

- **Implementación**: MIT
- **Nombres "Windows-inspired"**: son descriptivos, no reivindican
  propiedad sobre marcas Microsoft. Microsoft® es marca registrada
  de Microsoft Corp.
