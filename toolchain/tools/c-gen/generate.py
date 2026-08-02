"""c-gen — la fabrica Python que le toma la medida a BMO C.

Interroga a BMO C con SONDAS (programas de C que compila o no), lo contrasta
con lo que dice ISO y con lo que digan GCC/LLVM/MSVC si estan instalados, y
escribe `toolchain/lang/c/BRECHA.md`: lo que hay, lo que falta y **lo que no
debe entrar**.

Python es una herramienta de tu PC. **Nunca entra a BMO.**

Uso:
    py toolchain/tools/c-gen/generate.py
    py toolchain/tools/c-gen/generate.py --rapido    (sin recompilar el frontend)
"""

import sys
import pathlib

AQUI = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI))

from defs import estandar, libc, vendor   # noqa: E402
import extraer                            # noqa: E402
import sondas                             # noqa: E402
import informe                            # noqa: E402


def main():
    print("== c-gen: tomandole la medida a BMO C ==\n")

    cc = sondas.Compilador()
    if not cc.preparar():
        print("\nSin compilador no hay sondas, y sin sondas esto seria una lista")
        print("de deseos. Se para aqui a proposito.")
        return 1

    # ── 1. Las caracteristicas del lenguaje ──
    print("  sondeando el lenguaje...")
    resultados = []
    for nombre, (era, motivo, fuente) in estandar.SONDAS.items():
        if fuente is None:
            continue
        ok, err = cc.probar(fuente, nombre)
        resultados.append((nombre, era, motivo, ok, err))
        print(f"    {'ok  ' if ok else 'NO  '} {nombre}")

    # `#include` necesita dos ficheros: sonda aparte.
    ok, err = cc.probar_dos_ficheros(
        "#define UNO 1\n",
        '#include "@H@"\nint main(){return UNO;}',
        "include propio",
    )
    resultados.append(("#include propio", "C89",
                       "DOOM son ~50 ficheros con sus cabeceras", ok, err))
    print(f"    {'ok  ' if ok else 'NO  '} #include propio")

    # ── 2. La superficie de libc ──
    print("\n  sondeando libc...")
    libc_res = []
    for nombre, cabecera, dest, motivo, fuente in libc.FUNCIONES:
        if fuente is None:
            libc_res.append((nombre, cabecera, dest, motivo, None, ""))
            continue
        ok, err = cc.probar(fuente, nombre)
        libc_res.append((nombre, cabecera, dest, motivo, ok, err))
        print(f"    {'ok  ' if ok else 'NO  '} {nombre}")

    # ── 3. Los testigos ──
    print("\n  buscando testigos (GCC / LLVM / MSVC)...")
    testigos = extraer.censo()
    for t in testigos:
        estado = t["version"] if t["presente"] else "no esta en esta maquina"
        print(f"    {t['nombre']}: {estado}")

    # ── 4. El informe ──
    destino = informe.escribir(resultados, libc_res, testigos)
    print(f"\n== escrito: {destino} ==")

    faltan = [r for r in resultados if not r[3]]
    print(f"   {len(resultados) - len(faltan)} de {len(resultados)} sondas del lenguaje en verde")
    if faltan:
        print("   falta, por orden de lo que duele:")
        for nombre, _era, motivo, _ok, _err in faltan:
            print(f"     - {nombre}  ({motivo})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
