# cobol-gen -- la fabrica Python del COBOL gigante

Python **en tu PC** que genera Rust verboso a partir de una definicion
compacta de COBOL. Para tablas enormes y mecanicas (palabras reservadas --
cientos --, verbos, funciones intrinsecas), escribir Rust a mano es inviable;
aqui describes COBOL corto y Python escribe el Rust largo.

> **Soberania**: Python es una herramienta de desarrollo. El Rust generado se
> **commitea**. **Python NUNCA entra a BMO** -- no es dependencia de runtime,
> no ships, no corre en el kernel. BMO sigue 100% Rust propio.

## Uso

Requiere Python 3 (instalado con `winget install Python.Python.3.13`).

```bash
py toolchain/tools/cobol-gen/generate.py
```

(Desde una terminal nueva `py` ya esta en PATH. Si no, usa la ruta:
`%LOCALAPPDATA%\Programs\Python\Python313\python.exe`.)

## Archivos

| Archivo | Rol |
|---|---|
| `defs/words.py` | Palabras reservadas: esencia por era + `RESERVED_STANDARD` + `RESERVED_VENDOR`. |
| `defs/verbs.py` | Verbos con codegen (palabra -> `CobolStatement`). |
| `defs/intrinsics.py` | Funciones intrinsecas (lista de reconocimiento). |
| `defs/grammar.py` | *(futuro)* formatos de sentencia por verbo -> dispatch del parser. |
| `defs/editmasks.py` | *(futuro)* mascaras de edicion PIC (`$$,$$9.99`, Z, CR/DB...). |
| `definition.py` | Agregador: importa de `defs/` y re-exporta. **Aqui NO se edita datos.** |
| `generate.py` | La fabrica: lee la definicion, escribe el Rust. |
| -> `lang/cobol/src/generated/words.rs` | **Salida generada** (Rust, commiteada, NO editar a mano). 556 reservadas + `is_essence`/`is_vendor`. |

**Organizado por concern**: cada archivo en `defs/` es una cosa; crecer uno
crece BMO COBOL sin tocar el resto. Python genera lo **tabular** (tablas,
dispatch, esqueletos); la **semantica/codegen** de cada verbo es logica Rust
(no generable -- Python arma el esqueleto, Rust pone la inteligencia).

## Esencia vs vendor (COBOL devorado -> BMO COBOL)

GnuCOBOL infla a 1130+ palabras porque traduce a C y trae todos los dialectos.
No es la esencia. Aqui separamos: **STANDARD** (el idioma de Grace Hopper,
primera clase) vs **VENDOR** (VAX DBMS / IBM obsoletas / pantalla -- reconocidas
pero aparte). El numero inflado de GnuCOBOL NO es el objetivo; la cobertura del
estandar si.

## Flujo

```
definition.py (compacto)  --py generate.py--►  lang/cobol/src/generated/*.rs
   "cientos de palabras"                          (tablas Rust + busqueda binaria)
                                                   -> compila, se commitea
```

Para crecer COBOL hacia el estandar 2023 completo: amplia las listas en
`definition.py`, corre `generate.py`, commitea el Rust. El parser crece solo.

## Regla

Lo generado lleva cabecera `AUTO-GENERADO ... NO editar a mano`. Si necesitas
cambiarlo, cambia `definition.py` (o `generate.py`) y regenera -- nunca el
`.rs` a mano, o el proximo `generate.py` lo pisa.
