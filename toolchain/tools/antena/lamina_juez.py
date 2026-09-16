#!/usr/bin/env python3
"""lamina_juez -- las reglas de `platform/shared/bmo-antena/src/lamina.rs`, en
Python, para los DOS lados que no son BMO-X: la antena (antes de servir una
lamina) y el cliente de prueba (al recibirla).

El juez de verdad es el banco de Rust; esto es su copia para el movil y para
Windows, y por eso vive en UN fichero que importan los dos programas: dos
copias del mismo juez son dos jueces que se separan.

Devuelve `((ancho, alto, n, bytes), cuenta)` o levanta `ValueError` con el
numero de linea y el nombre del `Rechazo`.
"""
import re

ANCHO_MAX, ALTO_LAMINA_MAX, ELEMENTOS_MAX, ESCALA_MAX = 1280, 32768, 4096, 4
LETRA_ANCHO, LETRA_ALTO, LINEA_MAX = 8, 16, 256


def _numero(s, tope):
    if not s.isdigit() or int(s) > tope:
        raise ValueError("Numero: %r" % s)
    return int(s)


def _id(s):
    if not re.fullmatch(r"[a-z0-9_-]{1,32}", s):
        raise ValueError("Id: %r" % s)
    return s


def _color(s):
    if not re.fullmatch(r"[0-9a-fA-F]{6}", s):
        raise ValueError("Color: %r" % s)
    return s


def juzgar_bytes(datos):
    """`datos`: la lamina entera, en bytes, con sus `\\n`."""
    lineas = datos.split(b"\n")
    if lineas and lineas[-1] == b"":
        lineas.pop()
    if not lineas:
        raise ValueError("vacia")
    cab = lineas[0].decode("ascii", "replace").rstrip("\r").split(" ")
    if cab[0] != "LAMINA" or len(cab) != 4:
        raise ValueError("linea 1: Verbo/Campos -- la cabecera es LAMINA <ancho> <alto> <n>")
    W, H, n = _numero(cab[1], ANCHO_MAX), _numero(cab[2], ALTO_LAMINA_MAX), _numero(cab[3], ELEMENTOS_MAX)
    if len(lineas) - 1 != n:
        raise ValueError("Orden: la cabecera anuncia %d lineas y hay %d" % (n, len(lineas) - 1))
    cuenta = {"CAJA": 0, "TEXTO": 0, "IMAGEN": 0, "ENLACE": 0, "CAMPO": 0}
    for k, cruda in enumerate(lineas[1:], start=2):
        cruda = cruda.rstrip(b"\r")
        if len(cruda) > LINEA_MAX:
            raise ValueError("linea %d: Largo (%d bytes)" % (k, len(cruda)))
        try:
            p = cruda.split(b" ")
            verbo = p[0].decode("ascii", "replace")
            if verbo == "TEXTO":
                if len(p) < 6:
                    raise ValueError("Campos")
                texto = b" ".join(p[5:])
                if not texto or any(not (0x20 <= b <= 0x7E or b >= 0xA0) for b in texto):
                    raise ValueError("NoAscii: un control en el texto")
                if any(not (0x20 <= b <= 0x7E) for b in b" ".join(p[:5])):
                    raise ValueError("NoAscii")
                x, y = _numero(p[1].decode(), ANCHO_MAX), _numero(p[2].decode(), ALTO_LAMINA_MAX)
                e = _numero(p[3].decode(), ESCALA_MAX)
                if e == 0:
                    raise ValueError("Tamano: escala 0")
                _color(p[4].decode())
                w, h = len(texto) * LETRA_ANCHO * e, LETRA_ALTO * e
            else:
                if any(not (0x20 <= b <= 0x7E) for b in cruda):
                    raise ValueError("NoAscii")
                if verbo not in cuenta:
                    raise ValueError("Verbo: %r" % verbo)
                if len(p) != 6:
                    raise ValueError("Campos")
                x, y = _numero(p[1].decode(), ANCHO_MAX), _numero(p[2].decode(), ALTO_LAMINA_MAX)
                w, h = _numero(p[3].decode(), ANCHO_MAX), _numero(p[4].decode(), ALTO_LAMINA_MAX)
                if w == 0 or h == 0:
                    raise ValueError("Tamano: sin area")
                (_color if verbo == "CAJA" else _id)(p[5].decode())
            if x + w > W or y + h > H:
                raise ValueError("Fuera: %d+%d > %d o %d+%d > %d" % (x, w, W, y, h, H))
            cuenta[verbo] += 1
        except ValueError as e:
            raise ValueError("linea %d: %s" % (k, e))
    return (W, H, n, len(datos)), cuenta


def juzgar_lamina(ruta):
    return juzgar_bytes(open(ruta, "rb").read())
