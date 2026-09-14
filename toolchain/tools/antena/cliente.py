#!/usr/bin/env python3
"""cliente -- hace de BMO-X para probar la antena ANTES de que BMO-X sepa TCP.

S3 de docs/plan/PLAN_CLOUD_LOCAL.md: "desde Windows, un cliente de prueba recibe
un .mpg que se reproduce". Habla ANTENA/1 igual que lo hara BMO-X:

    python cliente.py <IP de la antena>              saluda y lista
    python cliente.py <IP de la antena> v1 -s 20     guarda 20 s de v1 en prueba.mpg

Si `prueba.mpg` se abre en cualquier reproductor, la antena ya sirve lo que
pl_mpeg va a necesitar.
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


def main():
    ap = argparse.ArgumentParser(description="Cliente de prueba ANTENA/1")
    ap.add_argument("antena")
    ap.add_argument("id", nargs="?")
    ap.add_argument("-s", "--segundos", type=int, default=20)
    ap.add_argument("-o", "--salida", default="prueba.mpg")
    ap.add_argument("--puerto", type=int, default=PUERTO)
    args = ap.parse_args()

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
