#!/usr/bin/env python3
"""navegador -- la ANTENA maneja un navegador Chromium ella sola, sin manos.

`PAGINA <url>` de docs/plan/PLAN_CLOUD_LOCAL.md (seccion 11, 2026-09-16):
BMO-X pide una url; la antena la carga en un Chromium SIN CABEZA, mete
`metricaBMO()` y `lamina.js` (la misma pareja que se pega a mano en la seccion
L0 de GUIA_MOVIL.md) y se lleva la LAMINA. Es Python mandando al JavaScript:
lo que Eddi pidio -- "automatizar en Python + JavaScript" -- sin app Android.

== Como habla con el navegador ==

Por el protocolo de depuracion de Chromium (el de `chrome://inspect`), que
es un WebSocket con JSON: `Page.navigate`, `Runtime.evaluate`. Se arranca el
navegador con `--remote-debugging-port=9222` y esto se conecta a 127.0.0.1.
El cliente WebSocket esta escrito aqui, entero, con `socket`: la antena no
pide ni una dependencia (en el movil `pip` es una molestia, y una libreria
de terceros dentro de la antena es codigo que no se leyo).

== Donde corre el navegador ==

   en un PC (V2, el Arch)    chromium --headless=new --remote-debugging-port=9222
   en Windows (probar)       msedge.exe --headless=new --remote-debugging-port=9222
   en el movil               Termux no trae Chromium. Dos caminos, y los dos se
                             prueban en el movil, no aqui:
                               a) proot-distro (Debian) con chromium arm64 y el
                                  mismo puerto: la antena no cambia una linea
                               b) el Chrome del movil expone su socket de
                                  depuracion (`localabstract:chrome_devtools_remote`)
                                  SOLO a `adb`; desde Termux no se puede contar
                                  con el. Si un dia se puede, es `--navegador`
                                  con otra direccion, y nada mas

== Lo que NO hace ==

  - no descarga nada: lo que el navegador cargue es cosa de la url y de quien
    la pidio (solo la IP permitida), y la antena solo saca la lamina
  - no espera JavaScript infinito: 15 s para cargar y 5 s para la lamina, y
    despues NO con el motivo
  - no deja pestanas abiertas: cada PAGINA abre una y la cierra
"""
import base64
import json
import os
import socket
import struct
import time
import urllib.request

ANCHO_LAMINA = 640
CARGA_S = 15
LAMINA_S = 5


class SinNavegador(Exception):
    """El navegador no esta, no contesta, o la pagina no dio lamina."""


class WebSocket:
    """Un cliente WebSocket minimo: texto, mascara del cliente, ping/pong."""

    def __init__(self, url, espera):
        # ws://host:puerto/ruta
        sin = url[len("ws://"):]
        hostpuerto, _, ruta = sin.partition("/")
        host, _, puerto = hostpuerto.partition(":")
        self.s = socket.create_connection((host, int(puerto or 80)), timeout=espera)
        clave = base64.b64encode(os.urandom(16)).decode("ascii")
        peticion = (
            "GET /%s HTTP/1.1\r\nHost: %s\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
            "Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n" % (ruta, hostpuerto, clave)
        )
        self.s.sendall(peticion.encode("ascii"))
        cabecera = b""
        while b"\r\n\r\n" not in cabecera:
            trozo = self.s.recv(1024)
            if not trozo:
                raise SinNavegador("el navegador cerro el saludo del WebSocket")
            cabecera += trozo
        if not cabecera.startswith(b"HTTP/1.1 101"):
            raise SinNavegador("el navegador no acepto el WebSocket: %r" % cabecera[:60])
        self.resto = cabecera.split(b"\r\n\r\n", 1)[1]

    def _leer(self, n):
        datos = self.resto[:n]
        self.resto = self.resto[n:]
        while len(datos) < n:
            trozo = self.s.recv(min(65536, n - len(datos)))
            if not trozo:
                raise SinNavegador("el navegador cerro la conexion")
            datos += trozo
        return datos

    def enviar(self, texto):
        carga = texto.encode("utf-8")
        cabeza = bytearray([0x81])
        n = len(carga)
        if n < 126:
            cabeza.append(0x80 | n)
        elif n < 65536:
            cabeza.append(0x80 | 126)
            cabeza += struct.pack(">H", n)
        else:
            cabeza.append(0x80 | 127)
            cabeza += struct.pack(">Q", n)
        mascara = os.urandom(4)
        cabeza += mascara
        enmascarada = bytes(b ^ mascara[i % 4] for i, b in enumerate(carga))
        self.s.sendall(bytes(cabeza) + enmascarada)

    def recibir(self):
        """Un mensaje de texto entero (junta fragmentos, contesta a los ping)."""
        mensaje = b""
        while True:
            b1, b2 = self._leer(2)
            fin = b1 & 0x80
            opcode = b1 & 0x0F
            n = b2 & 0x7F
            if n == 126:
                n = struct.unpack(">H", self._leer(2))[0]
            elif n == 127:
                n = struct.unpack(">Q", self._leer(8))[0]
            if b2 & 0x80:
                mascara = self._leer(4)
                carga = bytes(b ^ mascara[i % 4] for i, b in enumerate(self._leer(n)))
            else:
                carga = self._leer(n)
            if opcode == 8:
                raise SinNavegador("el navegador cerro el WebSocket")
            if opcode == 9:  # ping -> pong
                self.s.sendall(bytes([0x8A, 0x80]) + b"\x00\x00\x00\x00")
                continue
            if opcode in (0, 1):
                mensaje += carga
                if fin:
                    return mensaje.decode("utf-8", "replace")

    def cerrar(self):
        try:
            self.s.close()
        except OSError:
            pass


class Navegador:
    """Una sesion contra un Chromium con `--remote-debugging-port`."""

    def __init__(self, puerto, lamina_js):
        self.base = "http://127.0.0.1:%d" % puerto
        self.lamina_js = lamina_js
        self.siguiente = 1

    def _http(self, metodo, ruta):
        peticion = urllib.request.Request(self.base + ruta, method=metodo)
        try:
            with urllib.request.urlopen(peticion, timeout=5) as r:
                return r.read().decode("utf-8", "replace")
        except OSError as e:
            raise SinNavegador("el navegador no contesta en %s: %s" % (self.base, e))

    def _orden(self, ws, metodo, params=None, espera=CARGA_S):
        yo = self.siguiente
        self.siguiente += 1
        ws.enviar(json.dumps({"id": yo, "method": metodo, "params": params or {}}))
        fin = time.monotonic() + espera
        while time.monotonic() < fin:
            ws.s.settimeout(max(0.1, fin - time.monotonic()))
            try:
                m = json.loads(ws.recibir())
            except socket.timeout:
                break
            if m.get("id") == yo:
                if "error" in m:
                    raise SinNavegador("%s: %s" % (metodo, m["error"].get("message", "?")))
                return m.get("result", {})
        raise SinNavegador("%s no contesto en %d s" % (metodo, espera))

    def lamina_de(self, url):
        """La LAMINA (bytes, con sus `\\n`) de esa url, o `SinNavegador`."""
        # Una pestana nueva por pagina: lo que quede en ella se cierra al final.
        try:
            nueva = json.loads(self._http("PUT", "/json/new?about:blank"))
        except ValueError:
            raise SinNavegador("el navegador no supo abrir una pestana")
        ws = WebSocket(nueva["webSocketDebuggerUrl"], CARGA_S)
        try:
            self._orden(ws, "Page.enable")
            # El ancho de la lamina es el del navegador: se fija ANTES de cargar,
            # y no se reescala nada despues.
            self._orden(ws, "Emulation.setDeviceMetricsOverride",
                        {"width": ANCHO_LAMINA, "height": 800, "deviceScaleFactor": 1, "mobile": False})
            r = self._orden(ws, "Page.navigate", {"url": url}, CARGA_S)
            if r.get("errorText"):
                raise SinNavegador("no cargo: %s" % r["errorText"])
            # Esperar a que el documento este completo, con tope.
            fin = time.monotonic() + CARGA_S
            while True:
                estado = self._orden(ws, "Runtime.evaluate",
                                     {"expression": "document.readyState", "returnByValue": True}, 5)
                if estado.get("result", {}).get("value") == "complete":
                    break
                if time.monotonic() > fin:
                    raise SinNavegador("la pagina no termino de cargar en %d s" % CARGA_S)
                time.sleep(0.2)
            # metricaBMO() primero, o el texto se sale (medido el 16-09).
            expresion = self.lamina_js + "\n;metricaBMO(); JSON.stringify(lamina({ancho: %d}));" % ANCHO_LAMINA
            r = self._orden(ws, "Runtime.evaluate",
                            {"expression": expresion, "returnByValue": True, "awaitPromise": False}, LAMINA_S)
            if "exceptionDetails" in r:
                raise SinNavegador("lamina.js fallo: %s" % r["exceptionDetails"].get("text", "?"))
            resultado = json.loads(r["result"]["value"])
            if resultado.get("truncada"):
                raise SinNavegador("la pagina no cabe en una lamina (mas de 4096 lineas)")
            # Latin-1: la lamina viaja en bytes, y lamina.js ya dejo solo
            # ASCII y 0xA0..0xFF en las tiras.
            return resultado["lamina"].encode("latin-1", "replace")
        finally:
            ws.cerrar()
            try:
                self._http("GET", "/json/close/" + nueva["id"])
            except SinNavegador:
                pass


def cargar_lamina_js(carpeta_del_programa):
    ruta = os.path.join(carpeta_del_programa, "lamina.js")
    with open(ruta, "r", encoding="ascii") as f:
        return f.read()
