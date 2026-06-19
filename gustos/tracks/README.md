# 🎼 Tracks disponibles

Catálogo de sonidos para FastOS. Cada track es **autocontenido** y
puede reproducirse de forma independiente.

## Índice

| ID | Nombre | Tipo | Cuándo suena | Spec |
|----|--------|------|--------------|------|
| 001 | [hola_mundo](001_hola_mundo/synth.md) | FM bell | Inicio | [→](001_hola_mundo/synth.md) |
| 002 | [uhi_boot](002_uhi_boot/synth.md) | Sweep sine | Phase 5 | [→](002_uhi_boot/synth.md) |

## Cómo usar

```rust
use crate::gustos::tracks;

fn play_hello() {
    tracks::play_001_hola_mundo();
}

fn play_boot_sound() {
    tracks::play_002_uhi_boot();
}
```

## Estado

| ID | Estado | Testeado en |
|----|--------|-------------|
| 001 | 📝 Spec lista, código pendiente | — |
| 002 | 📝 Spec lista, código pendiente | — |
| 003+ | 🔮 Por diseñar | — |

## Cómo añadir un track

1. Crear carpeta `tracks/00X_nombre/`
2. Escribir `synth.md` con especificación matemática
3. Implementar `src/tracks/track_00X.rs` con el código
4. Añadir `play_00X_nombre()` a `tracks/mod.rs`
5. Actualizar este README

## Snippets listos para copiar

Ver `gustos/01-05_*.md` para snippets de FM bell, chimes UHI, etc.
