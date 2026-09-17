#!/usr/bin/env python3
"""cliente -- hace de BMO-X para probar la antena ANTES de que BMO-X sepa TCP.

S3 de docs/plan/PLAN_CLOUD_LOCAL.md: "desde Windows, un cliente de prueba recibe
un .mpg que se reproduce". Habla ANTENA/1 igual que lo hara BMO-X:

    python cliente.py <IP de la antena>              saluda y lista
    python cliente.py <IP de la antena> v1 -s 20     guarda 20 s de v1 en prueba.mpg
    python cliente.py <IP de la antena> p1           guarda la pagina p1 en pagina.lamina
    python cliente.py <IP de la antena> --pagina https://example.com   la antena NAVEGA
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


# -- L0: el juez vive en lamina_juez.py, el mismo que usa la antena -----------
from lamina_juez import juzgar_lamina  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description="Cliente de prueba ANTENA/1")
    ap.add_argument("antena", nargs="?")
    ap.add_argument("--lamina", help="juzga una lamina escrita por lamina.js (L0)")
    ap.add_argument("--pagina", help="PAGINA <url>: la antena navega sola y manda la lamina")
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
    if not args.id and not args.pagina:
        return 0

    if args.pagina:
        c.sendall(("PAGINA %s\n" % args.pagina).encode("ascii"))
    else:
        c.sendall(("PIDE %s\n" % args.id).encode("ascii"))
    respuesta = linea(f)
    print(respuesta)
    if respuesta.startswith("LAMINA "):
        # Una pagina: la cabecera y sus n lineas, tal cual, a un fichero; el
        # juez dice si la antena mando lo que BMO-X va a aceptar.
        n = int(respuesta.split()[3])
        datos = (respuesta + "\n").encode("ascii")
        for _ in range(n):
            cruda = f.readline(258)
            if not cruda:
                raise SystemExit("cliente: la antena corto la lamina a medias")
            datos += cruda.rstrip(b"\r\n") + b"\n"
        salida = args.salida if args.salida != "prueba.mpg" else "pagina.lamina"
        with open(salida, "wb") as fichero:
            fichero.write(datos)
        c.close()
        try:
            (W, H, n, peso), cuenta = juzgar_lamina(salida)
        except ValueError as e:
            print("cliente: lamina RECHAZADA -- %s" % e)
            return 1
        print("cliente: lamina %dx%d, %d lineas, %d bytes en %s" % (W, H, n, peso, salida))
        print("         " + ", ".join("%d %s" % (v, k.lower()) for k, v in cuenta.items()))
        return 0
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
