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
DESTINOS = [
    RAIZ / "Ultra_userspace" / "services" / "gui" / "src" / "escena" / "gato.rs",
    RAIZ / "Ultra_kernel_x86-64" / "kernel" / "src" / "ring0" / "core" / "gato.rs",
]

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

    cab = f'''//! **EL GATO** — el logo de BMO-X, en dos mascaras de 1 bit.
//!
//! ★ GENERADO. No se edita a mano: sale de `docs/arte/gato_a_mascara.py`, que
//! lee `docs/arte/bmo-x-gato.jpg`. Si el logo cambia, se vuelve a correr.
//!
//! ═══ Por que un bitmap y no una imagen ═══
//!
//! El logo es **97% negro plano, 1,6% blanco y 0,9% cian**, medido. Guardarlo
//! como imagen seria pagar 24 bits por pixel para almacenar "negro" cuarenta
//! mil veces — y una pantalla completa en BGRA son 8 MB contra el 1 MiB de
//! `MAX_BEX`. Un decodificador JPEG en `no_std` serian miles de lineas para
//! dibujar tres colores.
//!
//! Asi que el trazo va en un bit y los ojos en otro, y **el fondo no se
//! guarda** porque el fondo del splash ya es negro. Dibujar es un test de bit.
//!
//! {ancho}x{ALTO} px · {len(trazo)} B de trazo + {len(ojos)} B de ojos =
//! **{len(trazo) + len(ojos)} bytes**. Pixeles encendidos: {n_trazo} de trazo,
//! {n_ojos} de ojos.
//!
//! ═══ Y por que un GATO ═══
//!
//! Porque un gato se cae, se rompe algo y sigue andando. Este sistema se niega
//! a arrancar un programa antes que escribir en su memoria una direccion que no
//! ha podido calcular, y cuando el escritorio muere guarda sus ultimas cuatro
//! lineas para poder decir DONDE. No presume de no fallar: presume de contarlo.

/// Ancho de las dos mascaras, en pixeles.
pub(crate) const ANCHO: u32 = {ancho};
/// Alto de las dos mascaras, en pixeles.
pub(crate) const ALTO: u32 = {ALTO};

'''
    txt = cab + arr("TRAZO", trazo) + "\n\n" + arr("OJOS", ojos) + "\n"
    for d in DESTINOS:
        d.write_text(txt, encoding="utf-8")
        print(f"{d.relative_to(RAIZ)}")
    print(f"  {ancho}x{ALTO}  trazo={len(trazo)} B ({n_trazo} px)  ojos={len(ojos)} B ({n_ojos} px)")
    print(f"  total embebido: {len(trazo) + len(ojos)} bytes")


if __name__ == "__main__":
    main()
