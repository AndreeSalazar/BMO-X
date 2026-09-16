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
                               LAMINA <ancho> <alto> <n>, y n lineas (una PAGINA)
                               NO <motivo>

Desde el 2026-09-16 la carpeta tiene DOS cosas: videos (`v1`, `v2`...) y
PAGINAS ya maquetadas, ficheros `.lamina` (`p1`, `p2`...). Una lamina la hace
`lamina.js` en el navegador del movil (GUIA_MOVIL.md, seccion L0) y se deja en
la carpeta; la antena la JUZGA con `lamina_juez.py` antes de servirla -- una
lamina que BMO-X rechazaria no sale de aqui, sale un NO con el motivo. Una
lamina no acaba la conversacion: BMO-X pide la siguiente con el clic.

ESTRICTA (2026-09-14, Eddi: "MAS ESTRICTO ANTENA"):
  - UNA IP. Otra se cierra sin contestarle ni una palabra, y la antena espera un
    segundo antes de atender a nadie mas (un barrido no la tumba).
  - UNA conexion a la vez, y UN video por conexion.
  - el saludo en 10 segundos o se cuelga; la charla entera, 120 segundos.
  - 72 lineas como mucho por conexion (lo que pide una lista de 64 y poco mas).
  - la primera linea que no es del protocolo cierra la conexion: no hay "NO" de
    cortesia para quien no habla ANTENA/1.
  - solo ficheros NORMALES de DENTRO de la carpeta: un enlace que salga de ella
    no se sirve.
  - ffmpeg solo puede leer FICHEROS: `-protocol_whitelist file,pipe`. Un fichero
    disfrazado de video (una lista de reproduccion que apunta a Internet) no
    puede hacer que la antena descargue nada.

Lo que NO hace, a proposito:
  - no descarga nada de Internet: sirve lo que YA esta en la carpeta. De donde
    salga es decision de quien la llena, y las plataformas de video tienen sus
    condiciones de uso (seccion 1 del plan).
  - no cifra: vale en casa y con --permitir.
  - no escribe ninguna IP en ningun fichero, y no la imprime.
"""
import argparse
import os
import re
import socket
import subprocess
import sys
import time

from lamina_juez import juzgar_lamina

VERSION = "ANTENA/1"
PUERTO = 7117
LINEA_MAX = 256
LISTA_MAX = 64
LINEAS_MAX = LISTA_MAX + 8
TEXTO_MAX = 160
SALUDO_S = 10
CHARLA_S = 120
CASTIGO_S = 1.0
ANCHO, ALTO = 640, 360
EXTENSIONES = (".mp4", ".mkv", ".webm", ".mov", ".mpg", ".avi")
ID = re.compile(r"^[a-z0-9_-]{1,32}$")


class FueraDeProtocolo(Exception):
    """Una linea que no es ANTENA/1: se cuelga sin contestar."""


def limpio(texto):
    """El titulo como lo acepta BMO-X: ASCII imprimible y 160 caracteres."""
    t = "".join(c if 32 <= ord(c) < 127 else "?" for c in texto)
    return t[:TEXTO_MAX] or "?"


def dentro(carpeta, nombre):
    """La ruta real, si es un fichero normal DENTRO de la carpeta. Si no, None."""
    raiz = os.path.realpath(carpeta)
    ruta = os.path.realpath(os.path.join(carpeta, nombre))
    if os.path.commonpath([raiz, ruta]) != raiz or not os.path.isfile(ruta):
        return None
    return ruta


def catalogo(carpeta):
    """`[(id, fichero)]`. El id es `v1`, `v2`... para los videos y `p1`,
    `p2`... para las paginas: un nombre de fichero no viaja."""
    videos = sorted(
        f for f in os.listdir(carpeta)
        if f.lower().endswith(EXTENSIONES) and dentro(carpeta, f)
    )
    paginas = sorted(
        f for f in os.listdir(carpeta)
        if f.lower().endswith(".lamina") and dentro(carpeta, f)
    )
    lista = [("v%d" % (i + 1), n) for i, n in enumerate(videos)]
    lista += [("p%d" % (i + 1), n) for i, n in enumerate(paginas)]
    return lista[:LISTA_MAX]


def servir_lamina(conexion, ruta):
    """Una pagina: se juzga ENTERA antes de mandar la primera linea. Si BMO-X la
    rechazaria, aqui sale un NO con el motivo y no un fichero a medias."""
    datos = open(ruta, "rb").read()
    try:
        juzgar_lamina(ruta)
    except ValueError as e:
        enviar(conexion, "NO la lamina no vale: %s" % limpio(str(e)))
        return
    lineas = [l.rstrip(b"\r") for l in datos.split(b"\n")]
    if lineas and lineas[-1] == b"":
        lineas.pop()
    for l in lineas:
        conexion.sendall(l + b"\n")


class Linea:
    """Lee lineas con tope de largo, de cantidad y de tiempo."""

    def __init__(self, conexion):
        self.conexion = conexion
        self.leidas = 0
        self.fin_charla = time.monotonic() + CHARLA_S

    def leer(self, espera):
        self.leidas += 1
        if self.leidas > LINEAS_MAX:
            raise FueraDeProtocolo("demasiadas lineas")
        restante = self.fin_charla - time.monotonic()
        if restante <= 0:
            raise FueraDeProtocolo("la charla paso de su tiempo")
        self.conexion.settimeout(min(espera, restante))
        datos = b""
        while not datos.endswith(b"\n"):
            b = self.conexion.recv(1)
            if not b:
                return None
            datos += b
            if len(datos) > LINEA_MAX + 2:
                raise FueraDeProtocolo("linea demasiado larga")
        texto = datos.rstrip(b"\r\n")
        if any(c < 0x20 or c > 0x7E for c in texto):
            raise FueraDeProtocolo("bytes que no son texto")
        return texto.decode("ascii")


def enviar(conexion, linea):
    conexion.sendall((linea + "\n").encode("ascii", "replace"))


def servir_video(conexion, ruta):
    orden = [
        "ffmpeg", "-hide_banner", "-loglevel", "error", "-nostdin",
        "-protocol_whitelist", "file,pipe",
        "-re", "-i", ruta,
        "-vf", "scale=%d:%d:force_original_aspect_ratio=decrease,pad=%d:%d:(ow-iw)/2:(oh-ih)/2"
        % (ANCHO, ALTO, ANCHO, ALTO),
        "-c:v", "mpeg1video", "-b:v", "1200k", "-r", "30",
        "-c:a", "mp2", "-b:a", "128k", "-ac", "2", "-ar", "44100",
        "-f", "mpeg", "pipe:1",
    ]
    try:
        proceso = subprocess.Popen(orden, stdout=subprocess.PIPE, stdin=subprocess.DEVNULL)
    except FileNotFoundError:
        enviar(conexion, "NO no hay ffmpeg en la antena (pkg install ffmpeg)")
        return
    enviar(conexion, "VIDEO 0 mpeg1 %dx%d" % (ANCHO, ALTO))
    conexion.settimeout(30)
    enviados = 0
    inicio = time.monotonic()
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
    segundos = max(time.monotonic() - inicio, 0.001)
    print("antena: video terminado, %d bytes en %.0f s (%.2f Mbit/s)"
          % (enviados, segundos, enviados * 8 / 1e6 / segundos))


def atender(conexion, carpeta, nombre):
    lineas = Linea(conexion)
    if lineas.leer(SALUDO_S) != "HOLA " + VERSION:
        raise FueraDeProtocolo("no empezo con HOLA " + VERSION)
    enviar(conexion, "HOLA %s %s" % (VERSION, limpio(nombre)))
    while True:
        linea = lineas.leer(CHARLA_S)
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
                raise FueraDeProtocolo("id mal formado")
            fichero = dict(catalogo(carpeta)).get(id_)
            ruta = dentro(carpeta, fichero) if fichero else None
            if ruta is None:
                enviar(conexion, "NO no hay nada con ese id")
                continue
            if id_.startswith("p"):
                servir_lamina(conexion, ruta)
                continue
            servir_video(conexion, ruta)
            return
        else:
            raise FueraDeProtocolo("orden desconocida")


def main():
    ap = argparse.ArgumentParser(description="La antena del CLOUD LOCAL de BMO-X")
    ap.add_argument("--carpeta", required=True, help="donde estan los videos")
    ap.add_argument("--permitir", required=True, help="la UNICA IP que puede pedir")
    ap.add_argument("--escuchar", default="0.0.0.0",
                    help="la IP de la antena por la que escuchar (mejor la de la LAN)")
    ap.add_argument("--puerto", type=int, default=PUERTO)
    ap.add_argument("--nombre", default="antena")
    args = ap.parse_args()
    if not os.path.isdir(args.carpeta):
        print("antena: no existe la carpeta")
        return 1
    servidor = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    servidor.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    servidor.bind((args.escuchar, args.puerto))
    servidor.listen(1)
    lista = catalogo(args.carpeta)
    print("antena: escuchando en el puerto %d, %d videos y %d paginas en la carpeta"
          % (args.puerto, sum(1 for i, _ in lista if i[0] == "v"), sum(1 for i, _ in lista if i[0] == "p")))
    while True:
        conexion, origen = servidor.accept()
        with conexion:
            if origen[0] != args.permitir:
                print("antena: cerrada una conexion de una IP no permitida")
                time.sleep(CASTIGO_S)
                continue
            print("antena: BMO-X conectado")
            try:
                atender(conexion, args.carpeta, args.nombre)
            except FueraDeProtocolo as e:
                print("antena: colgada, fuera de protocolo: %s" % e)
            except (OSError, socket.timeout):
                print("antena: colgada por tiempo o por la red")
            print("antena: conexion cerrada")


if __name__ == "__main__":
    sys.exit(main())
