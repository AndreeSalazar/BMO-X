"""Los TESTIGOS: GCC, LLVM y MSVC, si estan instalados.

No se les copia: se les **contrasta**. Si los tres dicen que `int` mide 4 y BMO
dice 4, el tema esta cerrado. Si BMO dijera otra cosa, el que se equivoca es
BMO y hay que enterarse aqui y no en el Ryzen.

★ Y si no estan instalados, **se dice y ya**. Un extractor que inventa datos
cuando no encuentra la fuente es peor que uno que no encuentra nada: el segundo
te manda a instalar un compilador, el primero te manda a depurar una mentira.
Es la misma regla que el emulador de BMO tiene escrita en `VERDAD.md`.
"""

import shutil
import subprocess


def _hay(cmd):
    return shutil.which(cmd) is not None


def macros_de(cmd, estilo):
    """Las macros predefinidas de un compilador. `{}` si no esta."""
    if not _hay(cmd):
        return None
    try:
        if estilo == "gnu":
            # `-dM -E` sobre la nada: el compilador escupe todo lo que se define
            # a si mismo antes de ver una linea de codigo.
            r = subprocess.run([cmd, "-dM", "-E", "-x", "c", "-"],
                               input="", capture_output=True, text=True, timeout=30)
        else:
            r = subprocess.run([cmd, "/Bx"], capture_output=True, text=True, timeout=30)
        if r.returncode != 0:
            return {}
        fuera = {}
        for linea in r.stdout.splitlines():
            if linea.startswith("#define "):
                resto = linea[len("#define "):]
                if " " in resto:
                    k, v = resto.split(" ", 1)
                else:
                    k, v = resto, ""
                if "(" not in k:
                    fuera[k] = v.strip()
        return fuera
    except Exception:
        return {}


def censo():
    """Que testigos hay en esta maquina."""
    testigos = []
    for cmd, nombre, estilo in (("gcc", "GCC", "gnu"),
                                ("clang", "LLVM/Clang", "gnu"),
                                ("cl", "MSVC", "msvc")):
        presente = _hay(cmd)
        version = ""
        if presente:
            try:
                r = subprocess.run([cmd, "--version"], capture_output=True,
                                   text=True, timeout=15)
                version = (r.stdout or r.stderr).splitlines()[0].strip()
            except Exception:
                version = "(no contesta a --version)"
        testigos.append({
            "cmd": cmd, "nombre": nombre, "estilo": estilo,
            "presente": presente, "version": version,
            "macros": macros_de(cmd, estilo) if presente else None,
        })
    return testigos
