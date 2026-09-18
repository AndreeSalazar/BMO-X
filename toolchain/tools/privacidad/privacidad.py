"""privacidad -- ni una IP de la red de casa ni una MAC de fabricante en el repo.

== De donde sale, y con fecha ==

La regla es del dueno, del 2026-09-13: *ni MAC entera ni IP en el repo
PUBLICO*. La pantalla y CABINA recortan la MAC al fabricante desde entonces.

Y el 2026-09-18 se rompio SIN QUE NADA AVISARA: la IP de la antena del dueno
--una `192.168.0.x`-- entro en cuatro sitios en un solo dia (dos textos de uso en
`red_tcp.rs` y dos "como se sabe" en los planes), y dos de ellos llegaron a
GitHub. La regla existia y estaba bien escrita; lo que no tenia era quien la
mirara. Una regla que solo vive en un texto se incumple sin ruido.

** Y este mismo fichero la volvio a meter: esta cabecera traia la IP literal
al contar el incidente, y el guardian paso limpio mientras el fichero no
estaba en git -- solo mira lo publicado. Al hacer el commit, se cazo a si
mismo. Contar un dato sensible tambien es publicarlo.

== Que mira ==

Solo lo que git PUBLICA (`git ls-files`): lo que no esta rastreado no sale del
disco. Y en eso, dos cosas:

  IP privada   10/8, 172.16/12, 192.168/16, con sus CUATRO octetos. Tres
               octetos no son una IP: `RM 10.2.1` es una seccion del manual de
               Ada, y la primera version de la busqueda la confundio.
  MAC          solo las de FABRICANTE: unicast y globalmente administradas.
               Pasan el broadcast (`FF:..`), las multicast (`01:00:5E:..`) y
               las inventadas a proposito, que llevan el bit LOCAL (`02:..`) --
               ese bit existe justo para decir "esto no es de nadie".

Los ejemplos se escriben con un hueco (`<ip-de-la-antena>`) o con las
direcciones que el RFC 5737 reserva para documentar (192.0.2.x,
198.51.100.x, 203.0.113.x), que no son de nadie.

== Antes de juzgar, demuestra que ve ==

Lo primero que hace es pasarse sus propios casos: si su expresion no caza una IP
de prueba, se declara MUERTO en vez de decir "limpio". El exito de este guardian
es "no encontre nada", y un guardian ciego tambien encuentra nada.
"""

import argparse
import os
import re
import subprocess
import sys

RE_IPV4 = re.compile(r"(?<![\d.])(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})(?!\.?\d)")
RE_MAC = re.compile(r"(?<![0-9A-Fa-f:-])((?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2})(?![0-9A-Fa-f:-])")

# Los binarios no se miran: un .bex o una imagen trae secuencias que parecen
# numeros y no lo son.
#
# ** SOLO por extension (2026-09-18). La primera version saltaba ademas todo
# fichero con un byte cero en sus primeros 8 KB, "porque sera binario" -- y
# `inti/src/cabina/mod.rs` tenia un `'\0'` escrito como un cero CRUDO en el byte
# 7.889: el guardian no miro ese fichero de codigo ni una vez, y no lo decia.
# Un juez que decide por su cuenta que algo no le toca es un juez ciego.
BINARIOS = (".bex", ".ibx", ".bo", ".bin", ".bef", ".png", ".jpg", ".gif", ".qoi",
            ".bmp", ".wad", ".efi", ".ico", ".mus", ".wav", ".pdf", ".zip", ".mp4",
            ".mpg")


def raiz():
    return os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))


def ip_privada(texto):
    """Las IP privadas de `texto`."""
    halladas = []
    for m in RE_IPV4.finditer(texto):
        o = [int(x) for x in m.groups()]
        if any(x > 255 for x in o):
            continue
        if o[0] == 10 or (o[0] == 172 and 16 <= o[1] <= 31) or (o[0] == 192 and o[1] == 168):
            halladas.append(m.group(0))
    return halladas


def mac_de_fabricante(texto):
    """Las MAC unicast globales de `texto` (las que son de un aparato real)."""
    halladas = []
    for m in RE_MAC.finditer(texto):
        mac = m.group(1)
        primero = int(mac[0:2], 16)
        if primero & 0x01:        # multicast, broadcast incluido
            continue
        if primero & 0x02:        # administrada localmente: inventada a proposito
            continue
        if int(re.sub(r"[:-]", "", mac), 16) == 0:
            continue
        halladas.append(mac)
    return halladas


def autoprueba():
    """Si no caza lo que debe cazar, o caza lo que no, esta ciego o miente."""
    # Se arman por trozos para que este fichero no se acuse a si mismo.
    debe = [
        ("192.168" + ".0.103", ip_privada),
        ("10.1" + ".2.3", ip_privada),
        ("172.20" + ".0.5", ip_privada),
        ("2C:F0:5D" + ":12:34:56", mac_de_fabricante),
    ]
    no_debe = [
        ("RM 10.2.1", ip_privada),
        ("192.0.2" + ".7", ip_privada),
        ("version 1.10.2.3.4", ip_privada),
        ("FF:FF:FF" + ":FF:FF:FF", mac_de_fabricante),
        ("01:00:5E" + ":00:00:FB", mac_de_fabricante),
        ("02:1A:2B" + ":3C:4D:5E", mac_de_fabricante),
        ("2C-F0-5D-xx-xx-xx", mac_de_fabricante),
    ]
    fallos = [t for t, f in debe if not f(t)] + [t for t, f in no_debe if f(t)]
    return fallos


def ficheros_publicados():
    salida = subprocess.run(["git", "ls-files", "-z"], cwd=raiz(), capture_output=True, check=True).stdout
    return [p for p in salida.decode("utf-8", "replace").split("\0") if p]


def comprobar():
    fallos = autoprueba()
    if fallos:
        print("guardian MUERTO: su propia expresion falla en %s" % ", ".join(fallos))
        return 1
    mirados = saltados = 0
    quejas = []
    for rel in ficheros_publicados():
        if rel.lower().endswith(BINARIOS) or rel.endswith("Cargo.lock"):
            saltados += 1
            continue
        ruta = os.path.join(raiz(), rel)
        try:
            with open(ruta, "rb") as f:
                crudo = f.read()
        except OSError:
            continue
        mirados += 1
        for n, linea in enumerate(crudo.decode("utf-8", "replace").splitlines(), 1):
            for ip in ip_privada(linea):
                quejas.append("%s:%d  IP de una red privada: %s" % (rel, n, ip))
            for mac in mac_de_fabricante(linea):
                quejas.append("%s:%d  MAC de fabricante: %s" % (rel, n, mac))
    if mirados == 0:
        print("guardian MUERTO: no miro NI UN fichero publicado")
        return 1
    if quejas:
        for q in quejas:
            print("  [X] " + q)
        print("privacidad: %d dato(s) de la red de casa en el repo. Se escriben con un hueco "
              "(<ip-de-la-antena>) o con 192.0.2.x (RFC 5737)" % len(quejas))
        return 1
    print("clean: %d fichero(s) publicados mirados (%d binarios saltados por su extension), "
          "ni una IP de red privada ni una MAC de fabricante" % (mirados, saltados))
    return 0


def mensaje(ruta):
    """Juzga un MENSAJE DE COMMIT, antes de que el commit exista.

    ** El paso del build mira ficheros, y un mensaje no es un fichero: el
    2026-09-18 el commit que arreglaba esto CONTO el incidente con la IP
    literal en su mensaje, el build paso limpio, y alguien lo subio. Lo que hay
    en la historia publicada solo se quita reescribiendola; lo barato es no
    dejar que entre. Por eso esto va en el hook `commit-msg`.
    """
    with open(ruta, encoding="utf-8", errors="replace") as f:
        lineas = [l for l in f.read().splitlines() if not l.startswith("#")]
    quejas = []
    for n, linea in enumerate(lineas, 1):
        quejas += ["linea %d: IP de una red privada: %s" % (n, ip) for ip in ip_privada(linea)]
        quejas += ["linea %d: MAC de fabricante: %s" % (n, m) for m in mac_de_fabricante(linea)]
    if quejas:
        for q in quejas:
            print("  [X] mensaje de commit, " + q)
        print("privacidad: el mensaje publicaria un dato de la red de casa. Escribelo con un "
              "hueco (<ip-de-la-antena>); el commit NO se ha hecho.")
        return 1
    return 0


def main():
    ap = argparse.ArgumentParser(description="Ni IP de casa ni MAC de fabricante en el repo.")
    ap.add_argument("--check", action="store_true", help="lo que llama el build")
    ap.add_argument("--msg", metavar="FICHERO", help="juzga un mensaje de commit (hook commit-msg)")
    args = ap.parse_args()
    if args.msg:
        if autoprueba():
            print("guardian MUERTO: su propia expresion falla")
            return 1
        return mensaje(args.msg)
    return comprobar()


if __name__ == "__main__":
    sys.exit(main())
