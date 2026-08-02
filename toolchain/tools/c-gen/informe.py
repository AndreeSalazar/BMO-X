"""Escribe `BRECHA.md`: lo que hay, lo que falta y lo que NO debe entrar.

El documento vive al lado de `VERDAD.md` y sigue su misma regla: cada fila dice
**como se comprueba**. Aqui la comprobacion es la sonda, y la sonda se acaba de
ejecutar — no es una opinion de cuando se escribio el documento.
"""

import pathlib
import datetime

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[2]
DESTINO = RAIZ / "toolchain" / "lang" / "c" / "BRECHA.md"

CABECERA = """# BRECHA — lo que le falta a BMO C, medido y no opinado

> **AUTO-GENERADO** por `toolchain/tools/c-gen/generate.py`. No editar a mano:
> se regenera con `py toolchain/tools/c-gen/generate.py`.

Cada fila de este documento sale de una **sonda**: un programa de C minimo que
se le da a BMO C. Si compila, la fila dice `si`. Si no, dice el error EXACTO que
devolvio el compilador.

Eso importa mas de lo que parece. Leer el lexer diria que *palabras* reconoce
BMO C — y `static` esta en el lexer de cualquier compilador de juguete que
luego no sabe que hacer con ella. Aqui se pregunta lo unico que decide:
**¿compila?**

Es el mismo criterio que el banco de pruebas de BMO C (que EJECUTA los
programas en vez de mirar volcados de bytes) y el mismo que `VERDAD.md` aplica
al hardware. Un informe deducido de las fuentes envejece el dia que alguien
toca las fuentes; uno que compila se actualiza solo.

"""


def _fila_bool(ok):
    if ok is None:
        return "—"
    return "**si**" if ok else "**NO**"


def escribir(lenguaje, libc_res, testigos):
    hoy = datetime.date.today().isoformat()
    p = []
    p.append(CABECERA)
    p.append(f"Medido el **{hoy}**.\n")

    # ── Resumen ──
    total = len(lenguaje)
    verdes = sum(1 for r in lenguaje if r[3])
    p.append(f"## El numero\n")
    p.append(f"**{verdes} de {total}** sondas del lenguaje compilan.\n")

    faltan = [r for r in lenguaje if not r[3]]
    if faltan:
        p.append("Lo que falta, con lo que cuesta que falte:\n")
        p.append("| Falta | Era | Por que importa | Lo que dice BMO C |")
        p.append("|---|---|---|---|")
        for nombre, era, motivo, _ok, err in faltan:
            p.append(f"| **{nombre}** | {era} | {motivo} | `{err}` |")
        p.append("")

    # ── El lenguaje, entero ──
    p.append("## El lenguaje, sonda a sonda\n")
    p.append("| Caracteristica | Era | ¿Compila? | Para que |")
    p.append("|---|---|---|---|")
    for nombre, era, motivo, ok, _err in sorted(lenguaje, key=lambda r: (r[3], r[0])):
        p.append(f"| {nombre} | {era} | {_fila_bool(ok)} | {motivo} |")
    p.append("")

    # ── libc ──
    p.append("## libc — y el destinatario de cada funcion\n")
    p.append("★ La columna que decide no es *existe en el estandar*, es **para**")
    p.append("**que**. Una lista de libc sin motivo al lado es una invitacion a")
    p.append("implementarla entera, que es exactamente el fallo que la hoja de ruta")
    p.append("descarta con nombre propio.\n")
    for dest, titulo in (("ESENCIA", "Lo que BMO necesita para lo suyo"),
                         ("DOOM", "Lo que pide el objetivo de prueba"),
                         ("FUERA", "Lo que NO entra, y por que")):
        filas = [f for f in libc_res if f[2] == dest]
        if not filas:
            continue
        p.append(f"### {titulo}\n")
        if dest == "FUERA":
            p.append("| Funcion | Cabecera | Motivo del rechazo |")
            p.append("|---|---|---|")
            for nombre, cab, _d, motivo, _ok, _err in filas:
                p.append(f"| {nombre} | `{cab}` | {motivo} |")
        else:
            p.append("| Funcion | Cabecera | ¿Compila? | Para que |")
            p.append("|---|---|---|---|")
            for nombre, cab, _d, motivo, ok, _err in filas:
                p.append(f"| {nombre} | `{cab}` | {_fila_bool(ok)} | {motivo} |")
        p.append("")

    # ── Vendor ──
    p.append("## Lo que traen GCC, LLVM y MSVC encima del estandar\n")
    p.append("Esta lista no esta para copiarla: esta para **reconocerla y**")
    p.append("**rechazarla**. Es el mismo reparto que ya hace el COBOL de BMO")
    p.append("—esencia contra `VENDOR:`— y por la misma razon: un compilador que")
    p.append("persigue las extensiones de otros tres no termina nunca.\n")
    p.append("★ Con una excepcion honesta: **DOOM se escribio para GCC en 1993**.")
    p.append("Si su codigo usa una extension, el rechazo no puede ser *no*: tiene")
    p.append("que ser *no, y esto es lo que se hace en su lugar*.\n")
    p.append("| Extension | De quien | Veredicto | La salida |")
    p.append("|---|---|---|---|")
    for ext, dequien, veredicto, salida in vendor_filas():
        p.append(f"| `{ext}` | {dequien} | **{veredicto}** | {salida} |")
    p.append("")

    # ── Testigos ──
    p.append("## Los testigos de esta maquina\n")
    hay_alguno = any(t["presente"] for t in testigos)
    p.append("| Compilador | Estado |")
    p.append("|---|---|")
    for t in testigos:
        estado = t["version"] if t["presente"] else "no esta instalado"
        p.append(f"| {t['nombre']} | {estado} |")
    p.append("")
    if not hay_alguno:
        p.append("**Ninguno de los tres esta instalado**, y el informe lo dice en vez")
        p.append("de inventarse sus datos. Un extractor que rellena huecos cuando no")
        p.append("encuentra la fuente es peor que uno que no encuentra nada: el")
        p.append("segundo te manda a instalar un compilador, el primero te manda a")
        p.append("depurar una mentira. Es la regla que `VERDAD.md` ya le aplica al")
        p.append("emulador.\n")
        p.append("Para que este apartado se llene:")
        p.append("`winget install LLVM.LLVM` (Clang trae `-dM -E`, que es lo que se usa).\n")
    else:
        for t in testigos:
            if t["presente"] and t["macros"]:
                p.append(f"### {t['nombre']}: macros que mira el codigo ajeno\n")
                p.append("| Macro | Valor |")
                p.append("|---|---|")
                for m in vendor_macros():
                    if m in t["macros"]:
                        p.append(f"| `{m}` | `{t['macros'][m]}` |")
                p.append("")

    # ── El censo entero de C ──
    from defs import censo
    p.append("## ★ El censo de C, entero — y qué se DESCARTA\n")
    p.append("Un compilador acotado no se define por lo que tiene: se define por")
    p.append("**lo que deja fuera a propósito**. Una lista de características sin")
    p.append("veredicto es una lista de deberes; con veredicto es un *alcance* — y")
    p.append("un alcance es lo que hace que esto se pueda terminar.\n")
    total = len(censo.CENSO)
    esencia = len(censo.por_veredicto("ESENCIA"))
    util = len(censo.por_veredicto("UTIL"))
    fuera = len(censo.por_veredicto("DESCARTAR"))
    p.append(f"**{total} elementos** en el censo:\n")
    p.append("| Veredicto | Cuántos | Qué significa |")
    p.append("|---|---|---|")
    p.append(f"| **ESENCIA** | {esencia} | sin esto no es C. Entra, tarde o temprano |")
    p.append(f"| **UTIL** | {util} | aporta a lo que BMO hace. Entra cuando toque |")
    p.append(f"| **DESCARTAR** | {fuera} | existe en C y **no entra**, con su motivo |")
    p.append("")
    pct = (fuera * 100) // total if total else 0
    p.append(f"O sea: **{pct} de cada 100 elementos de C se quedan fuera**, y cada")
    p.append("uno con un motivo que se puede discutir. `DESCARTAR` no es *nunca*: es")
    p.append("*no en este alcance*. El día que el motivo caduque, la fila cambia.\n")

    for cat in censo.categorias():
        filas = [f for f in censo.CENSO if f[0] == cat]
        p.append(f"### {cat}\n")
        p.append("| Elemento | Era | Veredicto | Motivo |")
        p.append("|---|---|---|---|")
        for _c, elem, era, ver, motivo in filas:
            marca = {"ESENCIA": "**ESENCIA**", "UTIL": "UTIL", "DESCARTAR": "~~FUERA~~"}[ver]
            p.append(f"| {elem} | {era} | {marca} | {motivo} |")
        p.append("")

    # ── El cierre ──
    p.append("## Lo que este documento NO dice\n")
    p.append("Que una sonda compile **no** significa que el programa haga lo")
    p.append("correcto: eso lo prueba el banco de Rust, que ejecuta. Aqui se")
    p.append("pregunta si el compilador ACEPTA la construccion, que es la primera")
    p.append("de las dos preguntas y la que decide si 35.000 lineas ajenas tienen")
    p.append("alguna posibilidad de entrar.\n")

    DESTINO.parent.mkdir(parents=True, exist_ok=True)
    DESTINO.write_text("\n".join(p) + "\n", encoding="utf-8")
    return DESTINO


def vendor_filas():
    from defs import vendor
    return vendor.EXTENSIONES


def vendor_macros():
    from defs import vendor
    return vendor.MACROS_QUE_MIRA_EL_CODIGO_AJENO
