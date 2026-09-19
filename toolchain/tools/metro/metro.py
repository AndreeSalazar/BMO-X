"""metro -- el envoltorio del build para `bmo-metro` (2026-09-18).

El metro del emisor de x86-64 es Rust (compila con los emisores de verdad y
ejecuta en el emulador); el build solo lanza guardianes en Python. Esto lo
lanza y devuelve su veredicto tal cual. Ver `src/main.rs`.

Antes de juzgar, demuestra que ve: si no hay `cargo`, o el metro no llega a
decir nada, es MUERTO -- no "limpio".
"""

import os
import shutil
import subprocess
import sys


def raiz():
    return os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))


def main():
    cargo = shutil.which("cargo") or os.path.join(os.path.expanduser("~"), ".cargo", "bin", "cargo.exe")
    if not os.path.exists(cargo) and not shutil.which("cargo"):
        print("guardian MUERTO: no hay `cargo` para compilar el metro")
        return 1
    modo = "--check" if "--check" in sys.argv else ""
    r = subprocess.run([cargo, "run", "-q", "-p", "bmo-metro", "--"] + ([modo] if modo else []),
                       cwd=raiz(), capture_output=True, text=True, encoding="utf-8", errors="replace")
    salida = r.stdout.strip()
    if not salida:
        print("guardian MUERTO: el metro no dijo nada")
        for l in r.stderr.strip().splitlines()[-5:]:
            print("  " + l)
        return 1
    print(salida)
    return r.returncode


if __name__ == "__main__":
    sys.exit(main())
