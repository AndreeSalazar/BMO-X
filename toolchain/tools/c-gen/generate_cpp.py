"""Escribe `toolchain/lang/cpp/BRECHA.md`: el alcance de BMO C++.

Hermano de `generate.py`, y a propósito **sin sondas**: el frontend de C++ de
hoy no emite un solo byte para NINGUNA entrada, ni siquiera un fichero vacío
(ver el diagnóstico medido en la cabecera del documento). Sondearlo daría una
tabla de "NO" sin información.

Lo que sí aporta hoy es el **alcance**: qué entra, qué no, y por qué. Cuando el
frontend emita bytes, este guion crece con sondas como el de C.

Uso:
    py toolchain/tools/c-gen/generate_cpp.py
"""

import sys
import pathlib
import datetime

AQUI = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI))
from defs import censo_cpp   # noqa: E402

RAIZ = AQUI.parents[2]
DESTINO = RAIZ / "toolchain" / "lang" / "cpp" / "BRECHA.md"

CABECERA = """# BRECHA — el alcance de BMO C++

> **AUTO-GENERADO** por `toolchain/tools/c-gen/generate_cpp.py`. No editar a
> mano: se regenera con `py toolchain/tools/c-gen/generate_cpp.py`.

## De dónde se parte, dicho sin adornos

★ **Diagnóstico corregido el 2026-08-02.** Antes aquí ponía que el frontend
"desborda la pila con una clase con dos métodos", lo que apuntaba al parser.
**No es el parser.** Medido:

```
$ bmo-cpp-front vacio.cpp        # cero bytes de entrada
thread 'main' has overflowed its stack
```

Desborda con un fichero **vacío**, con `class P{};` y con
`int main(){return 1;}`. La causa está medida y es otra:

| | bytes |
|---|---|
| `IrStmt` | 24 |
| `IrBlock` (256 sentencias en array fijo) | 6 152 |
| `IrFunction` (32 bloques) | 198 480 |
| **`IrModule` (64 funciones)** | **12 711 184 = 12,12 MB** |

`Emitter::new()` construye **12 MB en la pila**, por valor, antes de mirar el
AST. Son arrays de tamaño fijo diseñados para `no_std` en Ring 0, instanciados
en una herramienta que corre en el anfitrión.

★ **Y arreglarlo no serviría de nada, que es lo grave**: `IrModule` **no tiene
un solo consumidor en todo el repo**. Nada lo convierte en bytes. En C y COBOL
`compile_to_ir` es vestigial —su camino real es `codegen::compile_to_bef_bytes`—
pero en C++ es el **único** camino. Se ve en el manifiesto: `cpp/Cargo.toml`
depende sólo de `bmo-abi`, ni de `bmo-sem-asm` ni de `bmo-lower`. **No hay
emisor porque no se enchufó ninguno.**

Estado real: **1 099 líneas** (C: 10 115), **0 tests** (C: 216), **0 ficheros
`.cpp`** en el repo, **0 bytes emitidos jamás**. El parser además es carácter a
carácter sin lexer, sin precedencia de operadores, y `parse_body` **se salta en
silencio** todo lo que no reconoce — lo que viola de frente la regla de BMO:
*nada que compile y no haga lo que dice*.

Eso no es un defecto que ocultar: es el punto de partida honesto. Y explica por
qué este documento **no lleva sondas** todavía, al revés que el de C — sondear
un frontend que no emite bytes daría una tabla de "NO" sin información. Cuando
emita, este guion crece con sondas.

El estudio de cómo resuelven esto Cfront, GCC, LLVM y MSVC está en
[`MAESTROS.md`](MAESTROS.md); el contrato con BMO C, en
[`HERENCIA.md`](HERENCIA.md).

## La pregunta que decide cada fila

> **¿Esto me deja abstraer SIN PAGAR?**

Es el principio de coste cero, y no es una frase bonita: es el motivo por el
que C++ existe en vez de ser "C con clases". Bjarne lo formuló así — *no pagas
por lo que no usas, y lo que usas no lo podrías haber escrito mejor a mano*.

★ Y corta en sitios que sorprenden: **las excepciones y los iostreams son C++
y fallan la prueba**. No por gusto, sino por lo que arrastran — y eso está
explicado fila a fila.

"""

CIERRE = """
## Lo que esto significa en la práctica

Un C++ **sin excepciones y sin RTTI** no es una rareza de este proyecto: es
exactamente lo que compila todo el mundo que escribe C++ para sistemas
empotrados (`-fno-exceptions`, `-fno-rtti`), y por el mismo motivo. La
diferencia es que aquí está escrito por qué en vez de ser una opción heredada
del Makefile de alguien.

Y una honestidad sobre el navegador, que es la razón por la que C++ interesa:

- **Un navegador propio no necesita C++.** NetSurf —motor propio, ~200k
  líneas— está escrito en **C**, y C ya está en 32 de 32 sondas.
- C++ hace falta para **portar** algo que ya existe en C++, y para escribir
  sistemas grandes sin pagar la abstracción.

O sea que C++ no es el camino al navegador: es el camino a **escribir cosas
grandes sin que se hagan ingobernables**. Que es otra cosa, y también vale.

★ El censo completo de **qué aplicación desbloquea qué pieza del sistema** —con
la superficie de BMO-X medida, y por qué las palancas que más desbloquean no
piden C++— está en [`docs/QUE_DESBLOQUEA.md`](../../../docs/QUE_DESBLOQUEA.md).

## El orden

Empieza en **0**, y el 0 no es el que estaba escrito aquí antes. "Que compile
una clase" no puede ser el primer paso de algo que no emite bytes para un
fichero vacío.

0. ✅ **HECHO — que emita un byte.** Tirados `ir_emit.rs` y `IrModule`; la
   salida va a `bmo_c_front::ast::Program` → `codegen::compile_to_bef_bytes`.
   El test que lo sostiene: **el BEF de C++ es byte a byte idéntico al de BMO
   C** para la misma fuente. Si divergen, o dejó de heredar o se combinaron.
1. ✅ **HECHO (la mitad) — lexer y parser de verdad.** Tokens con línea real,
   la escalera completa de precedencia, ámbitos anidados, y **ninguna rama que
   descarte tokens**. La decisión cara quedó tomada: **el parser y la tabla de
   símbolos se hablan** — sin eso `a<b>(c)` no se desambigua (ver
   `MAESTROS.md`), y el punto de decisión ya existe aunque el conjunto de
   plantillas esté vacío hasta el paso 6.
   ⏳ **Falta el preprocesador** (`#include`, `#define`): se rechaza con motivo.
2. ✅ **HECHO — clase con métodos.** El desazucarado de Cfront: clase →
   `struct`, método → función libre con `this` de primer parámetro
   (`P.doble(P* this)`, y el punto es ilegal en C, así que no choca). Corren
   campos públicos y privados, métodos con argumentos, `this` explícito e
   implícito, métodos `const`, acceso por puntero (`p->x`, `p->f()`), un
   método llamando a otro, y un campo usado antes de declararse — que es legal
   en C++ y obliga a parsear la clase en **dos vueltas**.
   ★ Aquí apareció la primera ambigüedad de verdad: **`P *q` es una
   declaración o una multiplicación, y sólo la tabla de símbolos lo sabe**. Es
   el hermano pequeño de `a<b>(c)`, y llegó sin necesidad de plantillas.
3. **Constructor y destructor (RAII)**, que es la razón de existir del lenguaje:
   una lista de limpieza por ámbito, recorrida al revés en **cada** salida.
4. **Mangling**, en cuanto haya sobrecarga. Y el ABI se escribe **el mismo día**
   — la lección de MSVC.
5. **Virtuales y vtable** — con la herencia virtual y múltiple descartadas, es
   una tabla de punteros a función y un `vptr` en el offset 0.
6. **Plantillas básicas** por monomorfización, que es donde C++ deja de ser C
   con azúcar.

Y desde el paso 0, **una matriz de conformidad de C++** sobre `bmo_lower::emu`,
con la misma regla que la de C: *al añadir una característica al codegen, se le
añade su fila*. Si no ejecuta lo que dice soportar, no lo soporta.

`new`/`delete` esperan a la **capability de memoria**, igual que `malloc`. Y
devolver objetos por valor espera al `sret`, que es deuda de **C**, no de C++.
"""


def escribir():
    hoy = datetime.date.today().isoformat()
    p = [CABECERA, f"Escrito el **{hoy}**.\n"]

    total = len(censo_cpp.CENSO_CPP)
    esencia = len(censo_cpp.por_veredicto("ESENCIA"))
    util = len(censo_cpp.por_veredicto("UTIL"))
    fuera = len(censo_cpp.por_veredicto("DESCARTAR"))
    pct = (fuera * 100) // total if total else 0

    p.append(f"## El número\n")
    p.append(f"**{total} elementos** en el censo de C++:\n")
    p.append("| Veredicto | Cuántos | Qué significa |")
    p.append("|---|---|---|")
    p.append(f"| **ESENCIA** | {esencia} | sin esto, C++ no aporta nada sobre C |")
    p.append(f"| **UTIL** | {util} | aporta de verdad. Entra cuando toque |")
    p.append(f"| **DESCARTAR** | {fuera} | existe en C++ y **no entra**, con su motivo |")
    p.append("")
    p.append(f"**{pct} de cada 100 elementos de C++ se quedan fuera** — contra 27 de")
    p.append("cada 100 en C. La diferencia no es capricho: C++ acumuló treinta años de")
    p.append("características encima de un lenguaje que ya estaba completo.\n")

    for cat in censo_cpp.categorias():
        filas = [f for f in censo_cpp.CENSO_CPP if f[0] == cat]
        p.append(f"### {cat}\n")
        p.append("| Elemento | Veredicto | Motivo |")
        p.append("|---|---|---|")
        for _c, elem, ver, motivo in filas:
            marca = {"ESENCIA": "**ESENCIA**", "UTIL": "UTIL", "DESCARTAR": "~~FUERA~~"}[ver]
            p.append(f"| {elem} | {marca} | {motivo} |")
        p.append("")

    p.append(CIERRE)
    DESTINO.parent.mkdir(parents=True, exist_ok=True)
    DESTINO.write_text("\n".join(p) + "\n", encoding="utf-8")
    return DESTINO


if __name__ == "__main__":
    destino = escribir()
    print(f"== escrito: {destino} ==")
    t = len(censo_cpp.CENSO_CPP)
    f = len(censo_cpp.por_veredicto("DESCARTAR"))
    print(f"   {t} elementos, {f} descartados ({(f*100)//t} de cada 100)")
