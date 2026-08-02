"""Escribe `toolchain/lang/cpp/BRECHA.md`: el alcance de BMO C++.

Hermano de `generate.py`, y a propósito **sin sondas**: el frontend de C++ de
hoy son ~900 líneas que no sobreviven a una clase con dos métodos (desborda la
pila). Sondearlo daría una tabla de "NO" sin información.

Lo que sí aporta hoy es el **alcance**: qué entra, qué no, y por qué. Cuando el
frontend aguante una clase, este guion crece con sondas como el de C.

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

El frontend de C++ de hoy son **~900 líneas** y **desborda la pila** con esto:

```cpp
class P { public: int x; int doble() { return x * 2; } };
int main() { P p; p.x = 21; return p.doble(); }
```

Eso no es un defecto que ocultar: es el punto de partida. Y explica por qué
este documento **no lleva sondas** todavía, al revés que el de C — sondear un
frontend que no compila una clase daría una tabla de "NO" sin información.
Cuando aguante una clase, este guion crece con sondas.

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

## El orden si algún día se retoma

1. **Que compile una clase con métodos** — hoy desborda la pila. Todo lo demás
   depende de esto.
2. **Constructor y destructor (RAII)**, que es la razón de existir del lenguaje.
3. **Mangling**, en cuanto haya sobrecarga: dos funciones distintas necesitan
   símbolos distintos y no hay forma de esquivarlo.
4. **Virtuales y vtable** — el AST ya las conoce.
5. **Plantillas básicas**, que es donde C++ deja de ser C con azúcar.

`new`/`delete` esperan a la **capability de memoria**, igual que `malloc`.
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
