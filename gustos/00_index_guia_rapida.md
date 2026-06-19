# 📑 00 — Índice Rápido

> ¿No sabes por dónde empezar? Esta guía te lleva al lugar correcto.

## 🎯 Quiero...

### **Oír un sonido ya hecho** (track)
→ [`tracks/001_hola_mundo/synth.md`](tracks/001_hola_mundo/synth.md) — primer track
→ [`tracks/002_uhi_boot/synth.md`](tracks/002_uhi_boot/synth.md) — boot UHI

### **Entender la teoría**
→ [`01_fm_synth_basics.md`](01_fm_synth_basics.md) — qué es FM synthesis
→ [`03_icosahedral_resonance.md`](03_icosahedral_resonance.md) — por qué suenan bien los tonos

### **Diseñar mi propio chime**
→ [`04_pleasant_chimes.md`](04_pleasant_chimes.md) — 5 reglas de oro + plantillas
→ [`05_uhi_chimes.md`](05_uhi_chimes.md) — usar el estándar UHI japonés

### **Emitir un beep en mi PC**
→ [`02_beep_pc_speaker.md`](02_beep_pc_speaker.md) — código PIT + speaker

### **Implementar audio en el kernel**
→ [`README_implementacion.md`](README_implementacion.md) — arquitectura, API, hooks

### **Entender el panorama completo**
→ [`README.md`](README.md) — overview de todo

## 🔊 Por tipo de sonido

| Tipo | Frecuencia | Archivo |
|------|-----------|---------|
| Beep simple | 200–1000 Hz | `02_beep_pc_speaker.md` |
| FM bell | 880 Hz, ratio 1.19 | `01_fm_synth_basics.md` |
| Major chord (éxito) | 523+659+784 Hz | `04_pleasant_chimes.md` |
| Tritono (alerta) | 880+1100 Hz | `05_uhi_chimes.md` |
| Sweep (boot) | 880→1320 Hz | `tracks/002_uhi_boot/` |
| Inarmónico (campana) | ratios icosaédricos | `03_icosahedral_resonance.md` |

## ⏱️ Por duración

| Duración | Uso | Archivo |
|----------|-----|---------|
| 50–100 ms | Click en UI | `04_pleasant_chimes.md` |
| 100–200 ms | Notificación | `04_pleasant_chimes.md` |
| 200–500 ms | Success/Error | `05_uhi_chimes.md` |
| 1–2 s | Boot/celebration | `03_icosahedral_resonance.md` |
| 3+ s | Ambient/drones | (futuro) |

## 🎓 Por nivel de complejidad

### Principiante
1. Lee `02_beep_pc_speaker.md` y emite un beep
2. Copia `tracks/001_hola_mundo/synth.md` y reprodúcelo
3. Experimenta con frecuencias

### Intermedio
1. Lee `01_fm_synth_basics.md` para entender FM
2. Diseña tu propio chime siguiendo `04_pleasant_chimes.md`
3. Implementa `uhi_ok()` siguiendo `05_uhi_chimes.md`

### Avanzado
1. Lee `03_icosahedral_resonance.md` para timbres ricos
2. Diseña un chime con 6 parciales inarmónicos
3. Integra con la DSP chain (`drivers/audio/dsp/`)

## 📊 Mapa del proyecto

```
gustos/                              (este directorio, raíz)
├── README.md                        ← empieza aquí
├── 00_index_guia_rapida.md          ← este archivo
├── 01-05_*.md                       ← teoría y práctica
├── README_implementacion.md         ← cómo integrar
└── tracks/                          ← tracks específicos
    ├── 001_hola_mundo/
    └── 002_uhi_boot/

kernel/src/gustos/                   (NUEVO en v1.5.0)
├── mod.rs
├── synth/
└── tracks/
```

## ❓ FAQ

**P: ¿Qué sample rate uso?**
R: 48000 Hz (estándar USB audio, también funciona en PC speaker con downsampling).

**P: ¿Funciona con PC speaker o solo USB audio?**
R: Funciona con PC speaker vía PIT, pero el PC speaker solo emite square wave. Para FM synthesis completo, necesitas USB audio.

**P: ¿Cómo descargo samples de internet?**
R: Esta carpeta no descarga samples — todo se genera con FM synthesis. Si quieres samples WAV/MP3, necesitarías un decoder (futuro).

**P: ¿Hay copyright?**
R: FM synthesis patent expiró en 1995. UHI es estándar JEITA (uso libre). Todo el código es MIT, docs CC-BY-SA.

---

**¿No encuentras lo que buscas?** Revisa el [`README.md`](README.md) principal.
