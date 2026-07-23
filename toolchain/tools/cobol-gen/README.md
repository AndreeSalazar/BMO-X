# cobol-gen — la fábrica Python del COBOL gigante

Python **en tu PC** que genera Rust verboso a partir de una definición
compacta de COBOL. Para tablas enormes y mecánicas (palabras reservadas —
cientos —, verbos, funciones intrínsecas), escribir Rust a mano es inviable;
aquí describes COBOL corto y Python escribe el Rust largo.

> **Soberanía**: Python es una herramienta de desarrollo. El Rust generado se
> **commitea**. **Python NUNCA entra a BMO** — no es dependencia de runtime,
> no ships, no corre en el kernel. BMO sigue 100% Rust propio.

## Uso

Requiere Python 3 (instalado con `winget install Python.Python.3.13`).

```bash
py toolchain/tools/cobol-gen/generate.py
```

(Desde una terminal nueva `py` ya está en PATH. Si no, usa la ruta:
`%LOCALAPPDATA%\Programs\Python\Python313\python.exe`.)

## Archivos

| Archivo | Rol |
|---|---|
| `definition.py` | **La fuente**: listas/dicts compactos de COBOL (reservadas, verbos…). **Aquí creces COBOL.** |
| `generate.py` | La fábrica: lee la definición, escribe el Rust. |
| → `lang/cobol/src/generated/words.rs` | **Salida generada** (Rust, commiteada, NO editar a mano). |

## Flujo

```
definition.py (compacto)  ──py generate.py──►  lang/cobol/src/generated/*.rs
   "cientos de palabras"                          (tablas Rust + búsqueda binaria)
                                                   → compila, se commitea
```

Para crecer COBOL hacia el estándar 2023 completo: amplía las listas en
`definition.py`, corre `generate.py`, commitea el Rust. El parser crece solo.

## Regla

Lo generado lleva cabecera `AUTO-GENERADO … NO editar a mano`. Si necesitas
cambiarlo, cambia `definition.py` (o `generate.py`) y regenera — nunca el
`.rs` a mano, o el próximo `generate.py` lo pisa.
