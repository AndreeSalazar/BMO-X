# c-gen — la fábrica Python que le toma la medida a BMO C

Python **en tu PC** que interroga a BMO C con **sondas**, lo contrasta con lo
que dice ISO y con lo que digan GCC/LLVM/MSVC si están instalados, y escribe
`toolchain/lang/c/BRECHA.md`.

> **Soberanía**: Python es una herramienta de desarrollo. **Nunca entra a BMO**
> — no es dependencia de runtime, no viaja en el `.bex`, no corre en el kernel.
> Es la misma regla que `cobol-gen`, y por el mismo motivo.

## Uso

```bash
py toolchain/tools/c-gen/generate.py
```

## ★ Por qué SONDAS y no leer las fuentes

Un extractor que lee `lexer.rs` diría qué **palabras** reconoce BMO C. Eso no es
lo que hace falta saber: `static` está en el lexer de cualquier compilador de
juguete que luego no sabe qué hacer con ella — y de hecho **eso es exactamente
lo que pasaba aquí**. `union` también estaba en el lexer.

Una sonda es un programa de C mínimo que se le da al compilador de verdad. La
pregunta es la única que decide: **¿compila?** Y la respuesta trae el error
exacto, que es lo que dice por dónde empezar.

Es el mismo criterio que el banco de pruebas de BMO C —que **ejecuta** los
programas en vez de mirar volcados de bytes— y el mismo que `VERDAD.md` aplica
al hardware. Un informe deducido de las fuentes envejece el día que alguien toca
las fuentes; uno que compila **se actualiza solo**.

### Y una sonda tiene que preguntar UNA cosa

En la primera pasada, dos sondas mintieron:

| Sonda | Decía | Era |
|---|---|---|
| `union` con un `char c[4]` dentro | "no hay uniones" | las uniones **funcionan**; lo que falla son los arrays dentro |
| `varargs` con `#include <stdarg.h>` | "no hay varargs" | no existe la cabecera; la sintaxis `...` ni se llegó a probar |

Las dos se partieron en sondas mínimas. Una sonda que mezcla dos cosas no
contesta ninguna, y encima manda a arreglar lo que no está roto.

## Archivos

| Archivo | Rol |
|---|---|
| `defs/estandar.py` | Palabras clave por era ISO (C89/99/11/23) y **una sonda por característica**, con *para qué sirve*. |
| `defs/libc.py` | La superficie de libc por cabecera, con **destinatario**: `ESENCIA` / `DOOM` / `FUERA` y el motivo. |
| `defs/vendor.py` | Extensiones de GCC/LLVM/MSVC: lo que **no** entra, y **la salida** para cada una. |
| `sondas.py` | Compila el frontend una vez y le pasa las sondas. |
| `extraer.py` | Habla con GCC/Clang/MSVC **si están**. Si no, lo dice. |
| `informe.py` | Escribe `BRECHA.md`. |
| `generate.py` | El orquestador. Punto de entrada. |

## La columna que decide

En `defs/libc.py` la columna no es *"existe en el estándar"*, es **para qué**.
Una lista de libc sin motivo al lado es una invitación a implementarla entera —
que es exactamente el fallo que la hoja de ruta descarta con nombre propio
(*"prometer compatibilidad que no existe"*). Por eso hay una sección de
**FUERA** con su motivo: `setlocale` no falta, **sobra**.

## Los testigos

GCC, LLVM y MSVC no se copian: se **contrastan**. Si los tres dicen que `int`
mide 4 y BMO dice 4, el tema está cerrado. Si BMO dijera otra cosa, el que se
equivoca es BMO y hay que enterarse aquí y no en el Ryzen.

Si no están instalados, el informe **lo dice y sigue**. Un extractor que
rellena huecos cuando no encuentra la fuente es peor que uno que no encuentra
nada: el segundo te manda a instalar un compilador, el primero te manda a
depurar una mentira.

Para llenar ese apartado: `winget install LLVM.LLVM`.
