#!/usr/bin/env python3
"""Convierte el logo del gato de BMO-X en dos mascaras de 1 bit para el compositor.

═══ Por que dos mascaras de 1 bit y no una imagen ═══

El logo es **97% negro plano, 1,6% blanco y 0,9% cian** (medido, no supuesto).
Guardarlo como imagen seria pagar 24 bits por pixel para almacenar "negro"
cuarenta mil veces.

Y no hay alternativa razonable: una pantalla completa a 1920x1080 en BGRA son
8 MB, y `MAX_BEX` —el tope del cargador— es 1 MiB. Un decodificador JPEG en
`no_std` seria miles de lineas para dibujar tres colores.

Asi que el trazo va en un bit y los ojos en otro:

    mascara TRAZO   1 bit/px   el contorno del gato, en blanco
    mascara OJOS    1 bit/px   los dos ojos `=`, en cian

A 152x180 son 3.420 bytes cada una: **6,8 KB en total**, y el fondo negro no se
guarda porque el fondo del splash YA es negro. Dibujar es un test de bit.

═══ Como se ejecuta ═══

    pip install pillow
    python docs/arte/gato_a_mascara.py

Escribe la mascara en el compositor Y en el kernel. **La salida esta
commiteada**, asi que el build NO depende de Python — este script existe para
poder REHACERLA si el logo cambia. Un asset generado sin su generador es un
asset que nadie puede volver a hacer.
"""

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    sys.exit("hace falta Pillow:  pip install pillow")

RAIZ = Path(__file__).resolve().parents[2]
FUENTE = Path(__file__).parent / "bmo-x-gato.jpg"

# ★ DOS DESTINOS, y no es duplicación evitable.
#
# El kernel y el compositor son **binarios distintos**: cada uno necesita los
# bytes dentro de su propia imagen, así que compartir un crate no ahorraría nada
# en tiempo de ejecución. Y el kernel es `no_std` sin `alloc` y no importa nada
# de `Ultra_userspace`.
#
# Lo que sí importa es que no se puedan desincronizar, y de eso se encarga esto:
# **los dos salen de la misma corrida de este script**. No hay una copia que
# alguien edite a mano, porque ninguna se edita a mano.
# ⚠️ ESTE GENERADOR ESTABA MUERTO, y hay que decir por que para que no se repita.
#
# Apuntaba a `escena/gato.rs` (la carpeta se llama `scene/` desde hace meses) y a
# `core/gato.rs` (que hoy es un modulo, `core/gato/masks.rs`). Y ademas escribia
# **el mismo texto** en los dos destinos, cuando los dos ficheros ya habian
# divergido a mano: el del compositor usa `WIDTH/HEIGHT/STROKE/EYES` y lleva las
# dimensiones dentro; el del kernel usa `TRAZO/OJOS`, hace `use super::*` y tiene
# las dimensiones en su `mod.rs`.
#
# O sea que correrlo habria roto los dos. Que es exactamente contra lo que avisa
# la cabecera de este mismo fichero: *"un asset generado sin su generador es un
# asset que nadie puede volver a hacer"*. Lo era.
#
# Ahora cada destino trae su PLANTILLA, porque la divergencia no es un descuido:
# el kernel parte datos y prosa en dos ficheros a proposito ("un fichero que dice
# NO EDITAR en la linea uno es un fichero que nadie parchea a mano a las 3am").
DESTINOS = [
    {
        "ruta": RAIZ / "Ultra_userspace" / "services" / "gui" / "src" / "scene" / "gato.rs",
        "prelude": "",
        "dims": True,
        "nombres": ("WIDTH", "HEIGHT", "STROKE", "EYES", "KANJI"),
        "sufijos": ("WIDTH", "HEIGHT"),
    },
    {
        "ruta": RAIZ / "Ultra_kernel_x86-64" / "kernel" / "src" / "ring0" / "core" / "gato" / "masks.rs",
        "prelude": "use super::*;\n\n",
        "dims": False,
        "nombres": ("ANCHO", "ALTO", "TRAZO", "OJOS", "KANJI"),
        "sufijos": ("ANCHO", "ALTO"),
    },
]

# ★ LA CAJA DEL KANJI, MEDIDA Y NO ESTIMADA.
#
# El comentario de abajo ya decia que el kanji empieza en x=732. El resto de la
# caja se saca barriendo la imagen: es lo mismo que se hizo para el gato, y con
# el mismo criterio -- un recorte a ojo deja margenes distintos arriba y abajo, y
# eso en un caracter cuadrado se ve.
#
# Barrido con `clase()`: x >= 725 y por encima del titulo (y < 760).
KANJI_MIN_X = 725
KANJI_MAX_Y = 760
# Alto final del kanji. Es ~0,4 del alto del gato, la misma proporcion que
# guardan en el logo.
KANJI_ALTO = 72

# La caja del GATO dentro del logo, medida sobre la imagen y no a ojo: el kanji
# empieza en x=732 (hay un hueco de columnas vacias en 650..732) y el titulo
# "BMO-X" en y=783. El compositor dibuja su propio titulo con la fuente, que a
# ese tamaño sale mas nitido que un bitmap.
CAJA = (320, 268, 715, 736)

# Alto final. El ancho sale de la proporcion para no deformarlo.
ALTO = 180


def clase(p):
    """0 = fondo, 1 = trazo (blanco), 2 = ojos (cian).

    ⚠️ El cian se pide por SESGO (`b - r`) y no por umbrales sueltos.

    La primera version usaba `b > 110 and g > 110 and r < 150`, y con eso los
    pixeles del antialias del borde blanco —que en un JPEG salen ligeramente
    azulados— caian en "cian". El resultado eran motas cian salpicando todo el
    contorno del gato: en la vista previa a x3 se ven, y en la pantalla de
    verdad se leerian como ruido.

    Un cian de verdad tiene el azul MUY por encima del rojo. Un blanco sucio no.
    """
    r, g, b = p
    if b - r > 60 and g - r > 40 and b > 110:
        return 2
    if r > 120 and g > 120 and b > 120:
        return 1
    return 0


def main():
    if not FUENTE.exists():
        sys.exit(f"no encuentro {FUENTE}")
    im = Image.open(FUENTE).convert("RGB").crop(CAJA)
    ancho = max(1, round(im.width * ALTO / im.height))
    # LANCZOS y luego umbral: reducir con vecino mas cercano a esta escala
    # rompe las lineas de un pixel de grosor, que aqui son casi todo el dibujo.
    im = im.resize((ancho, ALTO), Image.LANCZOS)

    trazo = bytearray((ancho * ALTO + 7) // 8)
    ojos = bytearray((ancho * ALTO + 7) // 8)
    n_trazo = n_ojos = 0
    for y in range(ALTO):
        for x in range(ancho):
            c = clase(im.getpixel((x, y)))
            if c == 0:
                continue
            i = y * ancho + x
            if c == 2:
                ojos[i // 8] |= 1 << (i % 8)
                n_ojos += 1
            else:
                trazo[i // 8] |= 1 << (i % 8)
                n_trazo += 1

    def arr(nombre, datos):
        out = [f"pub(crate) const {nombre}: [u8; {len(datos)}] = ["]
        for i in range(0, len(datos), 16):
            out.append("    " + " ".join(f"0x{b:02X}," for b in datos[i : i + 16]))
        out.append("];")
        return "\n".join(out)

    cab = f'''//! **EL GATO** -- el logo de BMO-X, en dos mascaras de 1 bit.
//!
//! ** GENERADO. No se edita a mano: sale de `docs/arte/gato_a_mascara.py`, que
//! lee `docs/arte/bmo-x-gato.jpg`. Si el logo cambia, se vuelve a correr.
//!
//! === Por que un bitmap y no una imagen ===
//!
//! El logo es **97% negro plano, 1,6% blanco y 0,9% cian**, medido. Guardarlo
//! como imagen seria pagar 24 bits por pixel para almacenar "negro" cuarenta
//! mil veces -- y una pantalla completa en BGRA son 8 MB contra el 1 MiB de
//! `MAX_BEX`. Un decodificador JPEG en `no_std` serian miles de lineas para
//! dibujar tres colores.
//!
//! Asi que el trazo va en un bit y los ojos en otro, y **el fondo no se
//! guarda** porque el fondo del splash ya es negro. Dibujar es un test de bit.
//!
//! {ancho}x{ALTO} px - {len(trazo)} B de trazo + {len(ojos)} B de ojos =
//! **{len(trazo) + len(ojos)} bytes**. Pixeles encendidos: {n_trazo} de trazo,
//! {n_ojos} de ojos.
//!
//! === Y por que un GATO ===
//!
//! Porque un gato se cae, se rompe algo y sigue andando. Este sistema se niega
//! a arrancar un programa antes que escribir en su memoria una direccion que no
//! ha podido calcular, y cuando el escritorio muere guarda sus ultimas cuatro
//! lineas para poder decir DONDE. No presume de no fallar: presume de contarlo.

'''
    # ── EL KANJI ──────────────────────────────────────────────────────────
    #
    # 猫 = "gato". Va en el logo a la derecha del dibujo, y es **100% cian**
    # (medido: 10.131 pixeles cian, 0 blancos), asi que una sola mascara.
    #
    # Se dibuja y no se escribe por el mismo motivo que el triangulo de aviso:
    # la fuente del kernel es ASCII de 16 px. Meter un glifo CJK en ella seria
    # una tabla de simbolos entera para un caracter.
    im0 = Image.open(FUENTE).convert("RGB")
    kx0, ky0, kx1, ky1 = im0.width, im0.height, 0, 0
    for y in range(KANJI_MAX_Y):
        for x in range(KANJI_MIN_X, im0.width):
            if clase(im0.getpixel((x, y))):
                kx0, ky0 = min(kx0, x), min(ky0, y)
                kx1, ky1 = max(kx1, x), max(ky1, y)
    kim = im0.crop((kx0, ky0, kx1 + 1, ky1 + 1))
    kancho = max(1, round(kim.width * KANJI_ALTO / kim.height))
    kim = kim.resize((kancho, KANJI_ALTO), Image.LANCZOS)
    kanji = bytearray((kancho * KANJI_ALTO + 7) // 8)
    n_kanji = 0
    for y in range(KANJI_ALTO):
        for x in range(kancho):
            if clase(kim.getpixel((x, y))):
                i = y * kancho + x
                kanji[i // 8] |= 1 << (i % 8)
                n_kanji += 1

    for d in DESTINOS:
        nw, nh, ns, ne, nk = d["nombres"]
        cuerpo = ""
        if d["dims"]:
            cuerpo += f"/// Ancho de las dos mascaras del gato, en pixeles.\npub(crate) const {nw}: u32 = {ancho};\n"
            cuerpo += f"/// Alto de las dos mascaras del gato, en pixeles.\npub(crate) const {nh}: u32 = {ALTO};\n\n"
        cuerpo += f"/// Ancho de la mascara del kanji.\npub(crate) const {nk}_ANCHO: u32 = {kancho};\n"
        cuerpo += f"/// Alto de la mascara del kanji.\npub(crate) const {nk}_ALTO: u32 = {KANJI_ALTO};\n\n"
        txt = (
            cab
            + d["prelude"]
            + cuerpo
            + arr(ns, trazo)
            + "\n\n"
            + arr(ne, ojos)
            + "\n\n"
            + arr(nk, kanji)
            + "\n"
        )
        d["ruta"].write_text(txt, encoding="utf-8")
        print(f"{d['ruta'].relative_to(RAIZ)}")
    print(f"  gato  {ancho}x{ALTO}  trazo={len(trazo)} B ({n_trazo} px)  ojos={len(ojos)} B ({n_ojos} px)")
    print(f"  kanji {kancho}x{KANJI_ALTO}  {len(kanji)} B ({n_kanji} px)  caja fuente {(kx0, ky0, kx1 + 1, ky1 + 1)}")
    print(f"  total embebido: {len(trazo) + len(ojos) + len(kanji)} bytes")


if __name__ == "__main__":
    main()
