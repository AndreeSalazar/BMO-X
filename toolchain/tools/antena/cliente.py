#!/usr/bin/env python3
"""cliente -- hace de BMO-X para probar la antena ANTES de que BMO-X sepa TCP.

S3 de docs/plan/PLAN_CLOUD_LOCAL.md: "desde Windows, un cliente de prueba recibe
un .mpg que se reproduce". Habla ANTENA/1 igual que lo hara BMO-X:

    python cliente.py <IP de la antena>              saluda y lista
    python cliente.py <IP de la antena> v1 -s 20     guarda 20 s de v1 en prueba.mpg
    python cliente.py --lamina pagina.lamina         juzga una LAMINA (L0)

Si `prueba.mpg` se abre en cualquier reproductor, la antena ya sirve lo que
pl_mpeg va a necesitar.

`--lamina` es L0 de la seccion 11: `lamina.js` corre en el navegador de la
antena y escribe una pagina ya maquetada; esto la lee con las MISMAS reglas que
`platform/shared/bmo-antena/src/lamina.rs` (el juez de verdad es ese banco) y
dice cuantas cajas, tiras e imagenes trae, cuanto pesa, y si algo se sale.
"""
import argparse
import socket
import sys
import time

VERSION = "ANTENA/1"
PUERTO = 7117


def linea(f):
    texto = f.readline(258)
    if not texto:
        raise SystemExit("cliente: la antena cerro la conexion")
    return texto.decode("ascii", "replace").rstrip("\r\n")


# -- L0: juzgar una lamina con las reglas del lector de BMO-X -----------------

ANCHO_MAX, ALTO_LAMINA_MAX, ELEMENTOS_MAX, ESCALA_MAX = 1280, 32768, 4096, 4
LETRA_ANCHO, LETRA_ALTO, LINEA_MAX = 8, 16, 256


def _numero(s, tope):
    if not s.isdigit() or int(s) > tope:
        raise ValueError("un numero imposible: %r" % s)
    return int(s)


def _id(s):
    import re
    if not re.fullmatch(r"[a-z0-9_-]{1,32}", s):
        raise ValueError("un id fuera de [a-z0-9_-]{1,32}: %r" % s)
    return s


def _color(s):
    import re
    if not re.fullmatch(r"[0-9a-fA-F]{6}", s):
        raise ValueError("un color que no es rrggbb: %r" % s)
    return s


def juzgar_lamina(ruta):
    """Lee una lamina y devuelve (cabecera, cuenta) o levanta ValueError con el
    numero de linea y el motivo, con los nombres de `Rechazo`."""
    datos = open(ruta, "rb").read()
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
            verbo = p[0].decode("ascii")
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


def main():
    ap = argparse.ArgumentParser(description="Cliente de prueba ANTENA/1")
    ap.add_argument("antena", nargs="?")
    ap.add_argument("--lamina", help="juzga una lamina escrita por lamina.js (L0)")
    ap.add_argument("id", nargs="?")
    ap.add_argument("-s", "--segundos", type=int, default=20)
    ap.add_argument("-o", "--salida", default="prueba.mpg")
    ap.add_argument("--puerto", type=int, default=PUERTO)
    args = ap.parse_args()

    if args.lamina:
        try:
            (W, H, n, peso), cuenta = juzgar_lamina(args.lamina)
        except ValueError as e:
            print("cliente: lamina RECHAZADA -- %s" % e)
            return 1
        print("cliente: lamina %dx%d, %d lineas, %d bytes (%.1f s por la LAN de 10 Mbit)" % (W, H, n, peso, peso * 8 / 10e6))
        print("         " + ", ".join("%d %s" % (v, k.lower()) for k, v in cuenta.items()))
        return 0
    if not args.antena:
        ap.error("hace falta la IP de la antena, o --lamina")

    c = socket.create_connection((args.antena, args.puerto), timeout=10)
    f = c.makefile("rb")
    c.sendall(("HOLA %s\n" % VERSION).encode("ascii"))
    print(linea(f))
    c.sendall(b"LISTA\n")
    cabecera = linea(f)
    print(cabecera)
    for _ in range(int(cabecera.split()[1]) if cabecera.startswith("LISTA ") else 0):
        print("  " + linea(f))
    if not args.id:
        return 0

    c.sendall(("PIDE %s\n" % args.id).encode("ascii"))
    respuesta = linea(f)
    print(respuesta)
    if not respuesta.startswith("VIDEO "):
        return 1
    fin = time.time() + args.segundos
    total = 0
    with open(args.salida, "wb") as salida:
        while time.time() < fin:
            trozo = f.read1(16384)
            if not trozo:
                break
            salida.write(trozo)
            total += len(trozo)
    c.close()
    print("cliente: %d bytes en %s (%.2f Mbit/s)" % (total, args.salida, total * 8 / 1e6 / args.segundos))
    return 0


if __name__ == "__main__":
    sys.exit(main())
