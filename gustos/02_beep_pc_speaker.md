# 🔊 02 — Beep PC Speaker

> **v1.5.0**: Cómo emitir un beep real en FastOS usando el PC speaker.
> Es el hardware de audio más simple y siempre disponible.

## Hardware: PIT (Programmable Interval Timer) + Speaker

El PC speaker está conectado al **PIT channel 2** (I/O port 0x42) y a
un bit en el **keyboard controller** (port 0x61).

### Diagrama

```
PIT channel 2 ──► Square wave ──┐
                                ├──► PC speaker (transductor piezo)
Keyboard ctrl bit 0-1 ─────────┘    (gated by bit 0)
```

## Pasos para hacer beep

1. **Programar PIT channel 2** con la frecuencia deseada
2. **Activar el speaker** escribiendo en port 0x61
3. **Esperar N milisegundos** (usando PIT o TSC)
4. **Apagar el speaker**

## Frecuencia del PIT

El PIT tiene una base de **1.193182 MHz**. Para generar una onda
cuadrada de N Hz, escribimos el divisor:

```rust
const PIT_FREQUENCY: u32 = 1_193_182;

fn beep(freq_hz: u32, duration_ms: u32) {
    let divisor = PIT_FREQUENCY / freq_hz;
    unsafe {
        // 1. Configurar PIT channel 2 en modo 3 (square wave)
        outb(0x43, 0b10110110);  // 0xB6 = channel 2, lobyte/hibyte, mode 3, binary
        // 2. Escribir divisor (lobyte primero, luego hibyte)
        outb(0x42, (divisor & 0xFF) as u8);
        outb(0x42, ((divisor >> 8) & 0xFF) as u8);
        // 3. Activar speaker (bit 0 = speaker gate, bit 1 = timer gate)
        let prev = inb(0x61);
        if (prev & 0x03) == 0 {
            outb(0x61, prev | 0x03);
        }
    }
    // 4. Esperar
    crate::arch::cpu::tsc::busy_wait_ms(duration_ms, crate::arch::cpu::tsc_per_sec());
    // 5. Apagar
    unsafe {
        let prev = inb(0x61);
        outb(0x61, prev & !0x03);
    }
}
```

## Limitaciones

| Característica | Límite |
|----------------|--------|
| Rango de freq | 18 Hz – 1.19 MHz (real: 20–8000 Hz audible) |
| Canales | 1 (mono) |
| Sample rate | Variable según PIT (no fijo) |
| Timbre | Solo square wave (no FM, no sine) |
| Volumen | Fijo (no controlable por software) |

## Notas musicales

- **Frecuencias de notas**: la4 = 440 Hz
- **Una octava arriba**: 880 Hz
- **Frecuencias útiles**: 100 Hz – 4000 Hz

```rust
const NOTE_C4: u32 = 262;  // Do central
const NOTE_E4: u32 = 330;  // Mi
const NOTE_G4: u32 = 392;  // Sol
const NOTE_A4: u32 = 440;  // La (A4 estándar)
const NOTE_C5: u32 = 523;  // Do una octava arriba
```

## Beeps útiles en el sistema

| Evento | Frecuencia | Duración | Patrón |
|--------|------------|----------|--------|
| Bienvenida | 880 Hz | 100 ms | único |
| OK | 1320 Hz | 50 ms | corto |
| Error | 200 Hz | 300 ms | grave, largo |
| Alerta | 1000 Hz | 100 ms × 3 | triple |
| Boot | 523→659→784 Hz | 100 ms cada | arpegio |

## Integración con FM synthesis

Para timbres más ricos (FM bell, FM bass), necesitaríamos **USB audio**
o **HDMI audio**, no PC speaker. El PC speaker solo emite square wave.

## Función `beep_sequence`

```rust
/// Reproduce una secuencia de notas con timing preciso.
pub fn beep_sequence(notes: &[(u32, u32)]) {
    // notes = [(freq_hz, duration_ms), ...]
    for &(freq, dur) in notes {
        beep(freq, dur);
        // Gap entre notas (50ms)
        busy_wait_ms(50, tsc_per_sec());
    }
}

// Ejemplo: intro boot
beep_sequence(&[
    (NOTE_C4, 100),
    (NOTE_E4, 100),
    (NOTE_G4, 100),
    (NOTE_C5, 200),
]);
```

## Referencias

- OSDev Wiki: "PC Speaker" — https://wiki.osdev.org/PC_Speaker
- OSDev Wiki: "Programmable Interval Timer"
- Intel 8253/8254 PIT datasheet

---

**Siguiente**: `03_icosahedral_resonance.md` — por qué algunos tonos suenan más "ricos".
