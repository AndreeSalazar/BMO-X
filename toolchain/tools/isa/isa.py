"""isa -- este repositorio es de UNA arquitectura: x86-64. Y nada mas.

== De donde sale, y con fecha ==

El 2026-09-18 el dueno decidio: **una arquitectura, un repositorio**. Este es
BMO-X para x86-64 -- kernel, escritorio, compiladores y todo lo que emiten. Si
algun dia hay ARM64 o RISC-V, sera OTRO repositorio (`BMO-X-aarch64`, ...)
que empieza como una copia de este y cambia lo que haga falta; los dos no se
tocan nunca.

El motivo, en sus palabras: mezclar arquitecturas en un mismo arbol trae
choques, y BMO-X no es Linux. Un arbol con `#[cfg(target_arch)]` por todas
partes tiene caminos que NINGUNA maquina del dueno ejecuta, y un camino que
nadie ejecuta compila y miente. El ejemplo lo tenia dentro: hasta el 18-09 el
validador de BEF solo AVISABA de un `.bex` de ARM, y `CallingConvention::NATIVE`
cambiaba de valor segun la CPU del que compilaba.

Los PERFILES de hardware no cuestan (LEY 24: varias placas x86-64 caben en
este repo). Las ARQUITECTURAS si.

== Alcance: lo que BMO-X EJECUTA, no donde corren las herramientas ==

Los compiladores de este repo corren en el ANFITRION (hoy un Windows x86-64)
y EMITEN x86-64. Donde corran no es asunto de esta regla; lo que emiten, si.

== Que comprueba ==

  1. Todo `target` de compilacion del repo (`.cargo/config.toml`,
     `rust-toolchain.toml`, los `--target` de los scripts, especificaciones
     `.json`) es `x86_64-*`.
  2. Ningun fuente de Rust abre un camino para otra CPU:
     `target_arch = "<otra>"` no puede aparecer.
  3. Ninguna carpeta ni fichero lleva el nombre de otra ISA: ni un
     `arch/aarch64/`, ni un `emisor-riscv64/`, ni un `Ultra_kernel_arm64/`.
     Un emisor nuevo nace en SU repositorio, no al lado de este.

Los comentarios y la documentacion pueden NOMBRAR otras arquitecturas; lo que
no pueden es tener codigo para ellas.

== Y lo UNICO agnostico: los FRONTENDS de los compiladores ==

Tambien del 2026-09-18, y tambien de Eddi: en BMO-X TODO es x86-64 -- el
kernel, la Base, `bmo-abi`, el escritorio -- MENOS los frontends de los
compiladores. Cada lenguaje se parte en dos: el frontend (lexer, analisis,
arbol) que no sabe de CPU, y su `emisor-x86_64/` que es el elemento aislado.
Asi la copia a otra arquitectura se lleva los frontends TAL CUAL y solo
reescribe los emisores.

  4. Un frontend declarado en `FRONTENDS` no nombra una maquina en su codigo
     (registros, `asm!`, `core::arch`, x86/amd64...) y no depende de nada que
     emita: solo de lo que esta en `DEPS_DE_FRONTEND`.
  5. Los que aun NO estan partidos van en `POR_PARTIR`, a la vista y contados
     en cada build. Cuando uno se parte, sale de ahi y entra en `FRONTENDS`.

== Antes de juzgar, demuestra que ve ==

Si no encuentra ni un `target` x86-64 en el repo, no ha mirado nada: MUERTO,
no "limpio".
"""

import argparse
import os
import re
import subprocess
import sys

ISA = "x86_64"

# Nombres de OTRAS arquitecturas. Una palabra, no una subcadena: `armonia` no
# es ARM. `x86` a secas NO esta: en este repo nombra a x86-64 (el emisor de
# `bmo-lower` es `x86.rs`); la de 32 bits es i386/i586/i686, y esas si.
OTRAS = ("aarch64", "arm64", "arm", "armv7", "armv8", "thumbv7", "thumbv8",
         "riscv", "riscv32", "riscv64", "risc-v", "i386", "i586", "i686",
         "wasm32", "wasm64", "mips", "mips64", "powerpc", "powerpc64",
         "ppc64", "loongarch64", "s390x", "sparc64")

# Los frontends ya partidos: carpeta -> nombre del crate. Ver la cabecera, punto 4.
FRONTENDS = {
    "toolchain/lang/inti": "bmo-inti-front",
    "toolchain/lang/ada": "bmo-ada-front",
    "toolchain/lang/cobol": "bmo-cobol-front",
}
# Lo unico de lo que un frontend puede depender: nada que emita ni que sea la Base.
# Cada uno con su motivo; y el guardian exige que ELLOS tampoco lleven `asm!` ni
# `core::arch` (una dependencia limpia de un frontend sucio no lo limpia).
DEPS_DE_FRONTEND = {
    "bmo-mods": "cargar tablas de mods: texto y rutas, ninguna maquina",
    # 2026-09-18: INTI cuenta lo que compila por la capa `Lang` de CABINA. Es el
    # FORMATO de los eventos, no el servicio. Si cabina-core se muda a la Base
    # (pendiente desde el 17-09), este permiso se revisa: la Base es x86-64.
    "cabina-core": "el formato de los eventos de CABINA (capa Lang)",
    # 2026-09-18: la regla de disposicion de un agregado. Salio de `bmo-abi`
    # justo para que C y C++ la usen sin depender del ABI de x86-64.
    "bmo-disposicion": "la regla de disposicion de struct/union: aritmetica, sin dependencias",
}
DEPS_DE_FRONTEND.update({c: "otro frontend" for c in FRONTENDS.values()})
# Lenguajes cuyo frontend y emisor de x86-64 comparten crate todavia, con por que.
POR_PARTIR = {
    "bmo-c-front": "codegen/ (unas 7.700 lineas) emite x86-64 dentro del crate",
    "bmo-cpp-front": "desciende al arbol de C y emite con el codegen de C: se parte con C",
}
# Palabras que solo significan algo dentro de una maquina (la misma lista que
# `inti/tests/agnostico.rs`), como palabra ENTERA: `conversion` no nombra `rsi`.
RE_MAQUINA = re.compile(
    r"(?<![A-Za-z0-9_])(x86|x86_64|amd64|i386|aarch64|riscv|rax|rbx|rcx|rdx|rsi|rdi|rsp|rbp|"
    r"xmm[0-9]*|sysv|modrm|sse2|avx)(?![A-Za-z0-9_])|asm!|core::arch")
# Una cadena de Rust: su CONTENIDO es dato del usuario, no codigo. INTI tiene que
# poder leer `usa x86_64` en un programa, y eso no le ata a ninguna maquina.
RE_CADENA = re.compile(r'"(?:[^"\\]|\\.)*"')
RE_DEP = re.compile(r'^\s*([A-Za-z0-9_-]+)\s*=\s*\{[^}]*path\s*=')

RE_TRIPLE = re.compile(r"\b([a-z0-9_]+)-(unknown|pc|apple|linux|none)-[a-z0-9_]+\b")
RE_TARGET_ARCH = re.compile(r'target_arch\s*=\s*"([^"]+)"')

# Ficheros donde un triple es una ORDEN de compilacion, no una mencion.
def es_config(rel):
    base = rel.rsplit("/", 1)[-1]
    return (base in ("config.toml", "config", "rust-toolchain.toml", "rust-toolchain",
                     "Cargo.toml")
            or base.endswith((".ps1", ".sh", ".json")))


def raiz():
    return os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))


def ficheros():
    r = subprocess.run(["git", "ls-files", "-z"], cwd=raiz(), capture_output=True)
    if r.returncode != 0:
        return None
    return [p for p in r.stdout.decode("utf-8", "replace").split("\0") if p]


def nombre_ajeno(rel):
    """El primer trozo de la ruta que es el nombre de otra ISA, o None."""
    for parte in rel.lower().split("/"):
        tallo = parte.rsplit(".", 1)[0] if "." in parte else parte
        # `x86_64` / `x86-64` es UNA palabra: partida por el guion daria `x86`,
        # que es la de 32 bits. La autoprueba lo cazo en la primera version.
        tallo = tallo.replace("x86_64", "x8664").replace("x86-64", "x8664")
        for trozo in re.split(r"[-_.]", tallo):
            if trozo in OTRAS:
                return parte
        if tallo in OTRAS:
            return parte
    return None


def juzgar_fichero(rel, texto):
    """(quejas, triples_x86_vistos) de UN fichero."""
    quejas, vistos = [], 0
    if es_config(rel):
        for m in RE_TRIPLE.finditer(texto):
            arch = m.group(1)
            if arch == ISA:
                vistos += 1
            elif arch in OTRAS or arch.startswith(("arm", "riscv", "thumb", "mips", "wasm")):
                quejas.append("%s: compila para `%s`" % (rel, m.group(0)))
    if rel.endswith(".rs"):
        for n, linea in enumerate(texto.splitlines(), 1):
            if linea.strip().startswith("//"):
                continue
            for m in RE_TARGET_ARCH.finditer(linea):
                if m.group(1) != ISA:
                    quejas.append('%s:%d: camino para otra CPU (target_arch = "%s")'
                                  % (rel, n, m.group(1)))
    return quejas, vistos


def juzgar_frontend(carpeta, cargo_toml, fuentes):
    """Quejas de UN frontend: `fuentes` es [(rel, texto)] de su `src/`."""
    quejas = []
    en_deps = False
    for linea in cargo_toml.splitlines():
        t = linea.strip()
        if t.startswith("["):
            en_deps = t == "[dependencies]"
            continue
        m = RE_DEP.match(linea) if en_deps else None
        if m and m.group(1) not in DEPS_DE_FRONTEND:
            quejas.append("%s: el frontend depende de `%s`, y un frontend no depende de nada "
                          "que emita" % (carpeta, m.group(1)))
    for rel, texto in fuentes:
        for n, linea in enumerate(texto.splitlines(), 1):
            if linea.strip().startswith("//"):
                continue
            m = RE_MAQUINA.search(RE_CADENA.sub('""', linea).split("//")[0])
            if m:
                quejas.append("%s:%d: el frontend nombra una maquina (`%s`)" % (rel, n, m.group(0)))
    return quejas


def autoprueba():
    """El juicio, sobre casos que se sabe como acaban. Si falla, no se juzga."""
    fallos = []

    def caso(nombre, cond):
        if not cond:
            fallos.append(nombre)

    q, v = juzgar_fichero("k/.cargo/config.toml", '[target.x86_64-unknown-none]\n')
    caso("un target x86-64 es limpio y se cuenta", q == [] and v == 1)
    q, _ = juzgar_fichero("k/.cargo/config.toml", '[target.aarch64-unknown-none]\n')
    caso("un target aarch64 se caza", len(q) == 1)
    q, _ = juzgar_fichero("b.ps1", "cargo build --target riscv64gc-unknown-none-elf")
    caso("un --target riscv en un script se caza", len(q) == 1)
    q, v = juzgar_fichero("docs/x.md", "en un Mac: aarch64-apple-darwin")
    caso("un triple en la DOCUMENTACION no es una orden", q == [] and v == 0)
    q, _ = juzgar_fichero("a/src/x.rs", '#[cfg(target_arch = "aarch64")]\nfn f() {}\n')
    caso("un cfg de aarch64 se caza", len(q) == 1)
    q, _ = juzgar_fichero("a/src/x.rs", '#[cfg(target_arch = "x86_64")]\nfn f() {}\n')
    caso("un cfg de x86_64 es limpio", q == [])
    q, _ = juzgar_fichero("a/src/x.rs", '// antes: target_arch = "aarch64"\n')
    caso("un comentario no es codigo", q == [])
    caso("arch/aarch64 se caza", nombre_ajeno("toolchain/forge/sem-asm/tables/arch/aarch64/abi.toml") == "aarch64")
    caso("emisor-riscv64 se caza", nombre_ajeno("toolchain/lang/inti/emisor-riscv64/src/lib.rs") == "emisor-riscv64")
    caso("Ultra_kernel_arm64 se caza", nombre_ajeno("Ultra_kernel_arm64/Cargo.toml") == "ultra_kernel_arm64")
    caso("arch/x86_64 es limpio", nombre_ajeno("toolchain/forge/sem-asm/tables/arch/x86_64/abi.toml") is None)
    caso("`armonia.rs` no es ARM", nombre_ajeno("src/armonia.rs") is None)
    caso("i686 se caza", nombre_ajeno("lang/c/emisor-i686/x.rs") == "emisor-i686")
    ct = '[package]\nname = "f"\n[dependencies]\nbmo-mods = { path = "../m" }\n'
    caso("un frontend limpio es limpio (el comentario no cuenta)",
         juzgar_frontend("f", ct, [("f/src/a.rs", "fn f() {}\n// habla de rax\n")]) == [])
    caso("un frontend que depende de un emisor se caza",
         len(juzgar_frontend("f", ct.replace("bmo-mods", "bmo-lower"), [])) == 1)
    caso("un frontend que nombra rax se caza",
         len(juzgar_frontend("f", ct, [("f/src/a.rs", "let r = rax;\n")])) == 1)
    caso("`conversion` no nombra `rsi`",
         juzgar_frontend("f", ct, [("f/src/a.rs", "let conversion = rsi_no;\n")]) == [])
    caso("`usa x86_64` DENTRO de una cadena es dato, no codigo",
         juzgar_frontend("f", ct, [("f/src/a.rs", 'compila("usa x86_64\\nfin");\n')]) == [])
    caso("un asm! en un frontend se caza",
         len(juzgar_frontend("f", ct, [("f/src/a.rs", 'unsafe { core::arch::asm!("nop") }\n')])) == 1)
    caso("`Ultra_kernel_x86-64` no es x86 de 32 bits", nombre_ajeno("Ultra_kernel_x86-64/build.ps1") is None)
    return fallos


def comprobar():
    fallos = autoprueba()
    if fallos:
        print("guardian MUERTO: el juicio falla en: " + "; ".join(fallos))
        return 1
    lista = ficheros()
    if lista is None:
        print("guardian MUERTO: `git ls-files` no respondio")
        return 1
    quejas, vistos, mirados = [], 0, 0
    for rel in lista:
        ajeno = nombre_ajeno(rel)
        if ajeno:
            quejas.append("%s: lleva el nombre de otra arquitectura (`%s`)" % (rel, ajeno))
        if not (es_config(rel) or rel.endswith(".rs")):
            continue
        try:
            texto = open(os.path.join(raiz(), rel), encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        mirados += 1
        q, v = juzgar_fichero(rel, texto)
        quejas += q
        vistos += v
    frentes = 0
    for carpeta in sorted(FRONTENDS):
        try:
            ct = open(os.path.join(raiz(), carpeta, "Cargo.toml"), encoding="utf-8").read()
        except OSError:
            quejas.append("%s: el frontend declarado no existe" % carpeta)
            continue
        fuentes = [(rel, open(os.path.join(raiz(), rel), encoding="utf-8", errors="replace").read())
                   for rel in lista if rel.startswith(carpeta + "/src/") and rel.endswith(".rs")]
        if not fuentes:
            quejas.append("%s: el frontend declarado no tiene ni un fuente en src/" % carpeta)
        quejas += juzgar_frontend(carpeta, ct, fuentes)
        frentes += 1
    # Y lo que un frontend enlaza tampoco puede llevar la maquina dentro.
    for dep in sorted(DEPS_DE_FRONTEND):
        if dep in FRONTENDS.values():
            continue
        carpetas = [rel.rsplit("/", 1)[0] for rel in lista if rel.endswith("/Cargo.toml")
                    and re.search(r'^name\s*=\s*"%s"' % re.escape(dep),
                                  open(os.path.join(raiz(), rel), encoding="utf-8",
                                       errors="replace").read(), re.M)]
        if not carpetas:
            quejas.append("`%s` esta permitido a los frontends y no existe" % dep)
        for c in carpetas:
            for rel in lista:
                if rel.startswith(c + "/src/") and rel.endswith(".rs"):
                    t = open(os.path.join(raiz(), rel), encoding="utf-8", errors="replace").read()
                    if re.search(r"asm!|core::arch", RE_CADENA.sub('""', t)):
                        quejas.append("%s: `%s` lo enlaza un frontend y lleva codigo de maquina"
                                      % (rel, dep))
    if vistos == 0:
        print("guardian MUERTO: %d fichero(s) mirados y ni un target x86-64 -- no ve la configuracion"
              % mirados)
        return 1
    if quejas:
        for q in quejas:
            print("  [X] " + q)
        print("isa: %d incumplimiento(s) -- este repositorio es SOLO x86-64 (regla del 2026-09-18)"
              % len(quejas))
        return 1
    print("clean: %d ruta(s) y %d fichero(s) de codigo y configuracion mirados; %d target(s), "
          "todos x86-64; ni un camino ni una carpeta para otra CPU; %d frontend(s) agnosticos "
          "y %d por partir (%s)" % (len(lista), mirados, vistos, frentes, len(POR_PARTIR),
                                    ", ".join(sorted(POR_PARTIR))))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--check", action="store_true", help="modo build: 0 limpio, 1 si no")
    ap.parse_args()
    sys.exit(comprobar())


if __name__ == "__main__":
    main()
