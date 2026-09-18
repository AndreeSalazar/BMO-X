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
          "todos x86-64; ni un camino ni una carpeta para otra CPU" % (len(lista), mirados, vistos))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--check", action="store_true", help="modo build: 0 limpio, 1 si no")
    ap.parse_args()
    sys.exit(comprobar())


if __name__ == "__main__":
    main()
