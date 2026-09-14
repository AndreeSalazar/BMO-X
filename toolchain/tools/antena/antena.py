#!/usr/bin/env python3
"""antena -- la mitad MOVIL del CLOUD LOCAL (S3 de docs/plan/PLAN_CLOUD_LOCAL.md).

Corre en Termux (Android) o en cualquier maquina con Python 3 y ffmpeg. Sirve
los videos de UNA carpeta a UNA sola IP, convertidos EN VIVO a MPEG-1 + MP2
640x360, que es lo que BMO-X sabe ensenar con pl_mpeg.

    python antena.py --carpeta ~/storage/movies --permitir <IP de BMO-X>

El protocolo es ANTENA/1, y su juez esta en `platform/shared/bmo-antena`:

    BMO-X -> antena            antena -> BMO-X
    HOLA ANTENA/1              HOLA ANTENA/1 <nombre>
    LISTA                      LISTA <n>, y n lineas ENTRADA <id> <titulo>
    PIDE <id>                  VIDEO 0 mpeg1 640x360, y el flujo hasta cerrar
                               NO <motivo>

Lo que NO hace, a proposito:
  - no descarga nada de Internet: sirve ficheros que YA estan en la carpeta. De
    donde salgan es decision de quien la llena, y las plataformas de video
    tienen sus condiciones de uso (seccion 1 del plan).
  - no cifra: vale en casa y con --permitir. Una conexion de otra IP se cierra
    sin contestarle ni una palabra.
  - no escribe ninguna IP en ningun fichero, y no la imprime.
"""
import argparse
import os
import re
import socket
import subprocess
import sys

VERSION = "ANTENA/1"
PUERTO = 7117
LINEA_MAX = 256
LISTA_MAX = 64
TEXTO_MAX = 160
ANCHO, ALTO = 640, 360
EXTENSIONES = (".mp4", ".mkv", ".webm", ".mov", ".mpg", ".avi")
ID = re.compile(r"^[a-z0-9_-]{1,32}$")


def limpio(texto):
    """El titulo como lo acepta BMO-X: ASCII imprimible y 160 caracteres."""
    t = "".join(c if 32 <= ord(c) < 127 else "?" for c in texto)
    return t[:TEXTO_MAX] or "?"


def catalogo(carpeta):
    """`[(id, fichero)]`. El id es `v1`, `v2`...: un nombre de fichero no viaja."""
    nombres = sorted(f for f in os.listdir(carpeta) if f.lower().endswith(EXTENSIONES))
    return [("v%d" % (i + 1), n) for i, n in enumerate(nombres[:LISTA_MAX])]


def leer_linea(conexion):
    datos = b""
    while not datos.endswith(b"\n"):
        b = conexion.recv(1)
        if not b:
            return None
        datos += b
        if len(datos) > LINEA_MAX + 2:
            return None
    return datos.rstrip(b"\r\n").decode("ascii", "replace")


def enviar(conexion, linea):
    conexion.sendall((linea + "\n").encode("ascii", "replace"))


def servir_video(conexion, ruta):
    orden = [
        "ffmpeg", "-hide_banner", "-loglevel", "error", "-re", "-i", ruta,
        "-vf", "scale=%d:%d:force_original_aspect_ratio=decrease,pad=%d:%d:(ow-iw)/2:(oh-ih)/2"
        % (ANCHO, ALTO, ANCHO, ALTO),
        "-c:v", "mpeg1video", "-b:v", "1200k", "-r", "30",
        "-c:a", "mp2", "-b:a", "128k", "-ac", "2", "-ar", "44100",
        "-f", "mpeg", "pipe:1",
    ]
    try:
        proceso = subprocess.Popen(orden, stdout=subprocess.PIPE)
    except FileNotFoundError:
        enviar(conexion, "NO no hay ffmpeg en la antena (pkg install ffmpeg)")
        return
    enviar(conexion, "VIDEO 0 mpeg1 %dx%d" % (ANCHO, ALTO))
    enviados = 0
    try:
        while True:
            trozo = proceso.stdout.read(16384)
            if not trozo:
                break
            conexion.sendall(trozo)
            enviados += len(trozo)
    except OSError:
        pass  # BMO-X cerro la conexion: es su forma de decir PARA
    finally:
        proceso.kill()
        proceso.wait()
    print("antena: video terminado, %d bytes enviados" % enviados)


def atender(conexion, carpeta, nombre):
    conexion.settimeout(30)
    if leer_linea(conexion) != "HOLA " + VERSION:
        enviar(conexion, "NO se esperaba HOLA " + VERSION)
        return
    enviar(conexion, "HOLA %s %s" % (VERSION, limpio(nombre)))
    while True:
        linea = leer_linea(conexion)
        if linea is None:
            return
        if linea == "LISTA":
            lista = catalogo(carpeta)
            enviar(conexion, "LISTA %d" % len(lista))
            for id_, fichero in lista:
                enviar(conexion, "ENTRADA %s %s" % (id_, limpio(fichero)))
        elif linea.startswith("PIDE "):
            id_ = linea[5:]
            if not ID.match(id_):
                enviar(conexion, "NO id mal formado")
                continue
            fichero = dict(catalogo(carpeta)).get(id_)
            if fichero is None:
                enviar(conexion, "NO no hay video con ese id")
                continue
            conexion.settimeout(None)
            servir_video(conexion, os.path.join(carpeta, fichero))
            return
        else:
            enviar(conexion, "NO orden desconocida")


def main():
    ap = argparse.ArgumentParser(description="La antena del CLOUD LOCAL de BMO-X")
    ap.add_argument("--carpeta", required=True, help="donde estan los videos")
    ap.add_argument("--permitir", required=True, help="la UNICA IP que puede pedir")
    ap.add_argument("--puerto", type=int, default=PUERTO)
    ap.add_argument("--nombre", default="antena")
    args = ap.parse_args()
    if not os.path.isdir(args.carpeta):
        print("antena: no existe la carpeta")
        return 1
    servidor = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    servidor.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    servidor.bind(("0.0.0.0", args.puerto))
    servidor.listen(1)
    print("antena: escuchando en el puerto %d, %d videos en la carpeta"
          % (args.puerto, len(catalogo(args.carpeta))))
    while True:
        conexion, origen = servidor.accept()
        with conexion:
            if origen[0] != args.permitir:
                print("antena: cerrada una conexion de una IP no permitida")
                continue
            print("antena: BMO-X conectado")
            try:
                atender(conexion, args.carpeta, args.nombre)
            except (OSError, socket.timeout):
                pass
            print("antena: conexion cerrada")


if __name__ == "__main__":
    sys.exit(main())
