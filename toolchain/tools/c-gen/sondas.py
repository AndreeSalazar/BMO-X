"""Las SONDAS: se le pregunta a BMO C compilando, no leyendo.

★ Por que asi y no mirando el lexer.

Leer `lexer.rs` diria que PALABRAS reconoce. Eso no es lo que hace falta saber:
`static` esta en el lexer de casi cualquier compilador de juguete y luego el
parser no sabe que hacer con ella. Lo que hace falta saber es si un programa que
la usa **compila**, y eso solo lo contesta el compilador.

Es el mismo criterio que el banco de pruebas de BMO C —que EJECUTA los
programas en vez de mirar el volcado de bytes— y el mismo que `VERDAD.md`
aplica al hardware. Un informe que se deduce de las fuentes se queda viejo el
dia que alguien toca las fuentes; uno que compila se actualiza solo.
"""

import subprocess
import tempfile
import pathlib

RAIZ = pathlib.Path(__file__).resolve().parents[3]
CRATE = RAIZ / "toolchain" / "lang" / "c"


class Compilador:
    """BMO C, listo para que se le pregunten cosas."""

    def __init__(self, verbose=False):
        self.verbose = verbose
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="c-gen-"))
        self.exe = None

    def preparar(self):
        """Compila el frontend UNA vez. Sin esto, cada sonda pagaria el build."""
        print("  compilando BMO C (una sola vez)...")
        r = subprocess.run(
            ["cargo", "build", "--quiet", "--release", "-p", "bmo-c-front"],
            cwd=str(CRATE), capture_output=True, text=True,
        )
        if r.returncode != 0:
            print("  !! no se pudo compilar bmo-c-front:")
            print(r.stderr[-800:])
            return False
        exe = RAIZ / "target" / "release" / "bmo-c-front.exe"
        if not exe.exists():
            exe = RAIZ / "target" / "release" / "bmo-c-front"
        if not exe.exists():
            print(f"  !! no encuentro el binario en {exe}")
            return False
        self.exe = exe
        return True

    def probar(self, fuente: str, nombre: str = "sonda"):
        """Compila `fuente`. Devuelve (ok, primera_linea_del_error).

        No se ejecuta el .bex a proposito: aqui se pregunta si el COMPILADOR
        acepta la construccion. Que el programa haga lo correcto es trabajo del
        banco de pruebas de Rust, que ya lo ejecuta.
        """
        seguro = "".join(ch if ch.isalnum() else "_" for ch in nombre)[:40]
        c = self.tmp / f"{seguro}.c"
        bex = self.tmp / f"{seguro}.bex"
        c.write_text(fuente, encoding="utf-8")
        r = subprocess.run(
            [str(self.exe), str(c), "-o", str(bex)],
            capture_output=True, text=True,
        )
        if r.returncode == 0 and bex.exists():
            return True, ""
        salida = (r.stderr or "") + (r.stdout or "")
        motivo = ""
        for linea in salida.splitlines():
            linea = linea.strip()
            if linea.startswith("error"):
                motivo = linea
                break
        if not motivo:
            motivo = salida.strip().splitlines()[-1] if salida.strip() else "sin mensaje"
        return False, motivo[:150]

    def probar_dos_ficheros(self, cabecera: str, fuente: str, nombre: str):
        """Igual, pero con un `#include` de verdad a un fichero de al lado.

        Hace falta una sonda propia porque `#include` no se puede probar con un
        solo fichero, y es justo lo que separa "compila un programa" de "compila
        un PROGRAMA de cincuenta ficheros" — que es el caso de DOOM.
        """
        seguro = "".join(ch if ch.isalnum() else "_" for ch in nombre)[:40]
        h = self.tmp / f"{seguro}.h"
        h.write_text(cabecera, encoding="utf-8")
        return self.probar(fuente.replace("@H@", h.name), nombre)
