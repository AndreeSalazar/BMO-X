"""perfil-campos -- el guardian que compara CADA perfil con el codigo, campo a campo.

Por que existe
==============

`perfil` comprueba que cada perfil diga a quien expone y que ese alguien exista.
Eso es la FLECHA. Esto es el CONTENIDO: que lo que el perfil AFIRMA sea lo que el
codigo dice.

`perfil-placa` ya lo hacia para la placa. Este cubre los otros cinco -- y lo
primero que hizo falta al escribirlo fue mirar, uno por uno, QUE SE PUEDE
COMPARAR DE VERDAD. La respuesta no fue "todo".

Lo que se puede comparar, y lo que NO
======================================

    CPU     fabricante y microarquitectura SI: son constantes del perfil
            ** nucleos, hilos, ccx y ccd NO: el codigo los MIDE en el arranque
               (`t.total_cores`), no los declara. Y eso esta BIEN -- un numero
               medido no necesita que un fichero de texto lo confirme; lo que
               necesita es que nadie lo suponga. Aqui se dice para que la
               ausencia de comprobacion no se lea como un descuido

    GPU     `pci_vendor` y que `pci_devices` siga VACIO. Lo segundo importa mas
            que lo primero: en cuanto alguien meta una SKU ahi sin tarjeta
            delante, `claims()` dira que si a algo que nadie ha visto

    RED     `pci_vendor` SI, contra `VENDOR_REALTEK` del driver
            ** `pci_device` NO: el driver no lo declara como constante. Se
               comprueba contra el PCI en el arranque, no aqui

    DISCO   ** LOS TRES CIERRES DE `discos.ps1`, y es la comprobacion mas
            importante de todo el repo. No compara un numero: comprueba que
            siguen ESTANDO. Si alguien quita uno, el build para

    RAM     NADA. No hay un solo campo suyo que el codigo declare -- su unico
            numero de verdad se MIDE con la orden `banda`, y no se ha medido.
            Se cuenta como profile sin comprobar, y se DICE

** Un guardian que solo informa de lo que comprueba deja creer que comprueba
todo. Por eso el renglon limpio de este dice las dos cifras: cuantos campos se
compararon y cuantos perfiles no tienen ni uno que comparar.
"""

import argparse
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
PERFIL = os.path.join(RAIZ, "PERFIL")
K = os.path.join(RAIZ, "Ultra_kernel_x86-64", "kernel", "src", "ring0")

# (campo del perfil, fichero de codigo, regex que saca el valor, como comparar)
COMPARABLES = {
    "CPU.txt": [
        ("fabricante", os.path.join(K, "cpu_vendor", "ryzen_5_5600x", "mod.rs"),
         r'vendor:\s*"([^"]*)"', "texto"),
        ("microarquitectura", os.path.join(K, "cpu_vendor", "ryzen_5_5600x", "mod.rs"),
         r'microarch:\s*"([^"]*)"', "texto"),
    ],
    "GPU.txt": [
        ("pci_vendor", os.path.join(RAIZ, "platform", "drivers", "gpu", "rdna4", "src", "lib.rs"),
         r"pci_vendor:\s*(0x[0-9A-Fa-f]+)", "numero"),
    ],
    "RED.txt": [
        ("pci_vendor", os.path.join(K, "dev", "net", "mod.rs"),
         r"const VENDOR_REALTEK:\s*u16\s*=\s*(0x[0-9A-Fa-f]+)", "numero"),
    ],
}

# Perfiles que no tienen NI UN campo comparable, y por que. Estar aqui es una
# respuesta escrita, no un olvido.
SIN_COMPARAR = {
    "RAM.txt": "su unico numero de verdad se MIDE con `banda`, y no se ha medido",
    # DISCO no se comprueba por campos: se comprueba por CIERRES, abajo. Y esa
    # comprobacion es la mas importante de todo el repo.
    "DISCO.txt": "no se comprueba por campos sino por los tres CIERRES de discos.ps1",
}

# El de la placa tiene guardian propio.
SUYO = {"PERFIL.txt"}

# *** LOS TRES CIERRES DEL DESPLIEGUE. No es un campo: es que sigan existiendo.
DISCOS = os.path.join(RAIZ, "Ultra_kernel_x86-64", "build", "discos.ps1")
CIERRES = [
    ("rechaza la unidad del sistema", r"\$env:SystemDrive"),
    ("exige FAT o FAT32", r"'FAT32'"),
    ("exige que la unidad exista", r"Test-Path"),
]


def campo(texto, nombre):
    # Los campos van indentados en los perfiles: `  fabricante: AMD`.
    m = re.search(r"^\s*%s:\s*(.+?)\s*$" % re.escape(nombre), texto, re.M)
    return m.group(1) if m else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    if not os.path.isdir(PERFIL):
        print("la carpeta NO EXISTE: falta PERFIL/")
        return 1 if args.check else 0

    quejas = []
    comparados = 0
    sin_comparar = 0

    for dirpath, _, files in os.walk(PERFIL):
        for f in sorted(files):
            if not f.endswith(".txt") or f in SUYO:
                continue
            ruta = os.path.join(dirpath, f)
            with open(ruta, "r", encoding="utf-8", errors="replace") as fh:
                texto = fh.read()

            if f in SIN_COMPARAR:
                sin_comparar += 1
                continue

            reglas = COMPARABLES.get(f)
            if reglas is None:
                # Un perfil nuevo sin reglas ni excusa escrita. Se dice: la
                # alternativa es que aparezcan perfiles que nadie comprueba y
                # que nadie sepa que nadie los comprueba.
                quejas.append(
                    "%s no tiene ni reglas de comparacion ni una linea en "
                    "SIN_COMPARAR -- decide cual de las dos es" % f)
                continue

            for nombre, fichero, patron, modo in reglas:
                dice = campo(texto, nombre)
                if dice is None:
                    quejas.append("%s no declara `%s`" % (f, nombre))
                    continue
                if not os.path.exists(fichero):
                    quejas.append("%s: %s no existe" % (f, os.path.relpath(fichero, RAIZ)))
                    continue
                with open(fichero, "r", encoding="utf-8", errors="replace") as fh:
                    m = re.search(patron, fh.read())
                if not m:
                    quejas.append(
                        "%s: en %s ya no esta el valor de `%s` -- o se renombro, "
                        "o se quito" % (f, os.path.relpath(fichero, RAIZ), nombre))
                    continue
                codigo = m.group(1)
                igual = (int(dice, 16) == int(codigo, 16)) if modo == "numero" \
                    else (dice == codigo)
                if not igual:
                    quejas.append("%s: `%s` dice '%s' y el codigo dice '%s'"
                                  % (f, nombre, dice, codigo))
                else:
                    comparados += 1

    # -- GPU: que `pci_devices` siga VACIO ---------------------------------
    #
    # ** Esta no es una comparacion de campo, es un CIERRE. Una SKU escrita ahi
    # sin tarjeta delante hace que `claims()` diga que si a algo que nadie ha
    # visto, y eso es una mentira que compila.
    gpu = os.path.join(RAIZ, "platform", "drivers", "gpu", "rdna4", "src", "lib.rs")
    if os.path.exists(gpu):
        with open(gpu, "r", encoding="utf-8", errors="replace") as fh:
            if not re.search(r"pci_devices:\s*&\[\s*\]", fh.read()):
                quejas.append(
                    "GPU: `pci_devices` ya NO esta vacio. Si hay tarjeta en el "
                    "banco, actualiza PERFIL/GPU.txt y quita este cierre; si no "
                    "la hay, `claims()` esta reclamando hardware que nadie ha visto")
            else:
                comparados += 1

    # -- DISCO: los tres cierres, y es lo mas importante de aqui -----------
    if not os.path.exists(DISCOS):
        quejas.append("DISCO: no existe build/discos.ps1 -- el despliegue no "
                      "tiene los tres cierres porque no tiene fichero")
    else:
        with open(DISCOS, "r", encoding="utf-8", errors="replace") as fh:
            d = fh.read()
        for que, patron in CIERRES:
            if re.search(patron, d):
                comparados += 1
            else:
                quejas.append(
                    "** DISCO: FALTA EL CIERRE '%s' en build/discos.ps1. Ese "
                    "fichero es el unico del repo que puede escribir en el disco "
                    "del dueno -- ver PERFIL/DISCO.txt" % que)

    if quejas:
        print("los perfiles y el codigo NO dicen lo mismo:")
        for q in quejas:
            print("  " + q)
        print("")
        print("  cada perfil esta en PERFIL/ y dice a quien expone. Lo que este")
        print("  guardian puede y NO puede comparar esta en su cabecera.")
        return 1 if args.check else 0

    print("clean: %d campo(s) y cierre(s) comparados con el codigo, y %d perfil(es) "
          "sin nada que comparar (dicho, no olvidado)" % (comparados, sin_comparar))
    return 0


if __name__ == "__main__":
    sys.exit(main())
